//! Persistance portable SQLite / PostgreSQL (pilote `Any` de sqlx).
//!
//! * SQLite : instance unique (lab, PME), une seule connexion → écritures
//!   sérialisées, aucun verrou applicatif nécessaire.
//! * PostgreSQL : haute disponibilité (ETI) — plusieurs réplicas du serveur
//!   partagent la base ; la réservation des opérations et le chaînage de
//!   l'audit sont sérialisés côté base.
//!
//! Chaque écriture d'état s'accompagne, dans la même transaction, de son
//! événement d'audit : il est impossible de modifier l'état sans laisser de
//! trace.

use std::time::Duration;

use chrono::{DateTime, Utc};
use socle_core::audit::{AuditDraft, AuditEvent, GENESIS_HASH};
use socle_core::incident::Incident;
use socle_core::operation::Operation;
use socle_core::release::Release;
use socle_core::{CoreError, format_ts, now_micros};
use sqlx::any::{AnyPoolOptions, AnyRow};
use sqlx::{Any, AnyPool, Row, Transaction};

use crate::error::{Error, Result};
use crate::model::{ApiToken, Deployment, LogLine, MaintenanceStatus, MaintenanceWindow, OperationFilter, Versioned};

const SCHEMA_VERSION: i64 = 1;

const SCHEMA: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS socle_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
    "CREATE TABLE IF NOT EXISTS operations (
        id TEXT PRIMARY KEY,
        environment TEXT NOT NULL,
        kind TEXT NOT NULL,
        subject TEXT NOT NULL,
        status TEXT NOT NULL,
        release_id TEXT,
        requested_by TEXT NOT NULL,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        lease_owner TEXT,
        lease_until BIGINT,
        version BIGINT NOT NULL,
        data TEXT NOT NULL)",
    "CREATE INDEX IF NOT EXISTS operations_status_idx ON operations (status, created_at)",
    "CREATE INDEX IF NOT EXISTS operations_subject_idx ON operations (environment, subject, kind, created_at)",
    "CREATE TABLE IF NOT EXISTS operation_logs (
        operation_id TEXT NOT NULL,
        seq BIGINT NOT NULL,
        ts TEXT NOT NULL,
        level TEXT NOT NULL,
        step TEXT NOT NULL,
        message TEXT NOT NULL,
        PRIMARY KEY (operation_id, seq))",
    "CREATE TABLE IF NOT EXISTS releases (
        id TEXT PRIMARY KEY,
        service TEXT NOT NULL,
        digest TEXT NOT NULL,
        version TEXT NOT NULL,
        created_at TEXT NOT NULL,
        data TEXT NOT NULL,
        UNIQUE (service, digest))",
    "CREATE INDEX IF NOT EXISTS releases_service_idx ON releases (service, created_at)",
    "CREATE TABLE IF NOT EXISTS deployments (
        environment TEXT NOT NULL,
        service TEXT NOT NULL,
        release_id TEXT NOT NULL,
        operation_id TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        data TEXT NOT NULL,
        PRIMARY KEY (environment, service))",
    "CREATE TABLE IF NOT EXISTS incidents (
        id TEXT PRIMARY KEY,
        status TEXT NOT NULL,
        severity TEXT NOT NULL,
        environment TEXT,
        fingerprint TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        version BIGINT NOT NULL,
        data TEXT NOT NULL)",
    "CREATE INDEX IF NOT EXISTS incidents_status_idx ON incidents (status, created_at)",
    "CREATE INDEX IF NOT EXISTS incidents_fingerprint_idx ON incidents (fingerprint)",
    "CREATE TABLE IF NOT EXISTS maintenance_windows (
        id TEXT PRIMARY KEY,
        environment TEXT NOT NULL,
        starts_at TEXT NOT NULL,
        ends_at TEXT NOT NULL,
        status TEXT NOT NULL,
        version BIGINT NOT NULL,
        data TEXT NOT NULL)",
    "CREATE INDEX IF NOT EXISTS maintenance_env_idx ON maintenance_windows (environment, status)",
    "CREATE TABLE IF NOT EXISTS audit_events (
        seq BIGINT PRIMARY KEY,
        ts TEXT NOT NULL,
        actor TEXT NOT NULL,
        action TEXT NOT NULL,
        resource TEXT NOT NULL,
        outcome TEXT NOT NULL,
        details TEXT NOT NULL,
        prev_hash TEXT NOT NULL,
        hash TEXT NOT NULL)",
    "CREATE TABLE IF NOT EXISTS locks (
        scope TEXT PRIMARY KEY,
        holder TEXT NOT NULL,
        operation_id TEXT,
        acquired_at TEXT NOT NULL,
        expires_at BIGINT NOT NULL)",
    "CREATE TABLE IF NOT EXISTS api_tokens (
        id TEXT PRIMARY KEY,
        token_hash TEXT NOT NULL UNIQUE,
        subject TEXT NOT NULL,
        revoked BIGINT NOT NULL,
        created_at TEXT NOT NULL,
        last_used_at TEXT,
        data TEXT NOT NULL)",
];

#[derive(Clone)]
pub struct Store {
    pool: AnyPool,
    postgres: bool,
}

fn parse_ts(s: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| Error::internal(format!("horodatage invalide en base ({s}) : {e}")))
}

fn is_unique_violation(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::Database(db) if db.is_unique_violation())
}

fn decode<T: serde::de::DeserializeOwned>(row: &AnyRow) -> Result<T> {
    let data: String = row.try_get("data")?;
    Ok(serde_json::from_str(&data)?)
}

fn decode_operation(row: &AnyRow) -> Result<Versioned<Operation>> {
    let mut op: Operation = decode(row)?;
    // Le statut de référence est la colonne (mise à jour atomique lors de la
    // réservation par un worker).
    let status: String = row.try_get("status")?;
    if let Some(s) = socle_core::operation::OpStatus::parse(&status) {
        op.status = s;
    }
    Ok(Versioned { value: op, version: row.try_get("version")? })
}

impl Store {
    /// Ouvre la base (`sqlite://chemin?mode=rwc`, `sqlite::memory:`,
    /// `postgres://…`) et applique le schéma.
    pub async fn connect(url: &str) -> Result<Self> {
        sqlx::any::install_default_drivers();
        let postgres = url.starts_with("postgres://") || url.starts_with("postgresql://");
        let sqlite = url.starts_with("sqlite:");
        if !postgres && !sqlite {
            return Err(Error::internal("URL de base non prise en charge (sqlite:… ou postgres://…)"));
        }
        let opts = if sqlite {
            AnyPoolOptions::new().max_connections(1).min_connections(1).idle_timeout(None).max_lifetime(None)
        } else {
            AnyPoolOptions::new().max_connections(16).acquire_timeout(Duration::from_secs(10))
        };
        let pool = opts.connect(url).await?;
        let store = Self { pool, postgres };
        if sqlite {
            sqlx::query("PRAGMA busy_timeout = 5000").execute(&store.pool).await?;
        }
        store.migrate().await?;
        Ok(store)
    }

    pub fn is_postgres(&self) -> bool {
        self.postgres
    }

    async fn migrate(&self) -> Result<()> {
        for stmt in SCHEMA {
            sqlx::query(stmt).execute(&self.pool).await?;
        }
        sqlx::query(
            "INSERT INTO socle_meta (key, value) VALUES ('schema_version', $1)
             ON CONFLICT (key) DO UPDATE SET value = $1",
        )
        .bind(SCHEMA_VERSION.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn ping(&self) -> Result<()> {
        sqlx::query("SELECT 1 AS ok").fetch_one(&self.pool).await?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Audit
    // -----------------------------------------------------------------------

    async fn append_audit_tx(&self, tx: &mut Transaction<'_, Any>, draft: AuditDraft) -> Result<AuditEvent> {
        if self.postgres {
            // Sérialise le chaînage entre réplicas (verrou de transaction).
            sqlx::query("SELECT 1 AS locked FROM (SELECT pg_advisory_xact_lock(7469656)) AS l")
                .fetch_one(&mut **tx)
                .await?;
        }
        let head = sqlx::query("SELECT seq, hash FROM audit_events ORDER BY seq DESC LIMIT 1")
            .fetch_optional(&mut **tx)
            .await?;
        let (seq, prev) = match head {
            Some(row) => (row.try_get::<i64, _>("seq")? + 1, row.try_get::<String, _>("hash")?),
            None => (1, GENESIS_HASH.to_string()),
        };
        let event = draft.seal(seq, now_micros(), &prev);
        sqlx::query(
            "INSERT INTO audit_events (seq, ts, actor, action, resource, outcome, details, prev_hash, hash)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(event.seq)
        .bind(format_ts(&event.ts))
        .bind(event.actor.clone())
        .bind(event.action.clone())
        .bind(event.resource.clone())
        .bind(event.outcome.clone())
        .bind(serde_json::to_string(&event.details)?)
        .bind(event.prev_hash.clone())
        .bind(event.hash.clone())
        .execute(&mut **tx)
        .await?;
        Ok(event)
    }

    pub async fn append_audit(&self, draft: AuditDraft) -> Result<AuditEvent> {
        let mut tx = self.pool.begin().await?;
        let ev = self.append_audit_tx(&mut tx, draft).await?;
        tx.commit().await?;
        Ok(ev)
    }

    fn audit_from_row(row: &AnyRow) -> Result<AuditEvent> {
        let ts: String = row.try_get("ts")?;
        let details: String = row.try_get("details")?;
        Ok(AuditEvent {
            seq: row.try_get("seq")?,
            ts: parse_ts(&ts)?,
            actor: row.try_get("actor")?,
            action: row.try_get("action")?,
            resource: row.try_get("resource")?,
            outcome: row.try_get("outcome")?,
            details: serde_json::from_str(&details)?,
            prev_hash: row.try_get("prev_hash")?,
            hash: row.try_get("hash")?,
        })
    }

    /// Événements de séquence strictement supérieure à `after`, ordre croissant.
    pub async fn audit_after(&self, after: i64, limit: i64) -> Result<Vec<AuditEvent>> {
        let rows = sqlx::query(
            "SELECT seq, ts, actor, action, resource, outcome, details, prev_hash, hash
             FROM audit_events WHERE seq > $1 ORDER BY seq ASC LIMIT $2",
        )
        .bind(after)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(Self::audit_from_row).collect()
    }

    /// Derniers événements, du plus récent au plus ancien.
    pub async fn audit_latest(&self, limit: i64) -> Result<Vec<AuditEvent>> {
        let rows = sqlx::query(
            "SELECT seq, ts, actor, action, resource, outcome, details, prev_hash, hash
             FROM audit_events ORDER BY seq DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(Self::audit_from_row).collect()
    }

    // -----------------------------------------------------------------------
    // Opérations
    // -----------------------------------------------------------------------

    pub async fn insert_operation(&self, op: &Operation, audit: AuditDraft) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO operations (id, environment, kind, subject, status, release_id, requested_by,
                                     created_at, updated_at, lease_owner, lease_until, version, data)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NULL, NULL, 1, $10)",
        )
        .bind(op.id.clone())
        .bind(op.environment().to_string())
        .bind(op.kind().as_str())
        .bind(op.request.action.subject().to_string())
        .bind(op.status.as_str())
        .bind(op.resolved.release_id.clone())
        .bind(op.requested_by.clone())
        .bind(format_ts(&op.created_at))
        .bind(format_ts(&op.updated_at))
        .bind(serde_json::to_string(op)?)
        .execute(&mut *tx)
        .await?;
        self.append_audit_tx(&mut tx, audit).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn get_operation(&self, id: &str) -> Result<Option<Versioned<Operation>>> {
        let row = sqlx::query("SELECT status, version, data FROM operations WHERE id = $1")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(decode_operation).transpose()
    }

    /// Met à jour une opération (verrouillage optimiste), avec ses événements
    /// d'audit et, le cas échéant, l'enregistrement du déploiement — le tout
    /// atomiquement. Libère la réservation du worker si l'état est terminal.
    pub async fn update_operation(
        &self,
        op: &Operation,
        expected_version: i64,
        audits: Vec<AuditDraft>,
        deployment: Option<&Deployment>,
    ) -> Result<i64> {
        let mut tx = self.pool.begin().await?;
        let terminal = op.status.is_terminal();
        let res = sqlx::query(
            "UPDATE operations
             SET status = $1, release_id = $2, updated_at = $3, data = $4, version = version + 1,
                 lease_owner = CASE WHEN $5 = 1 THEN NULL ELSE lease_owner END,
                 lease_until = CASE WHEN $5 = 1 THEN NULL ELSE lease_until END
             WHERE id = $6 AND version = $7",
        )
        .bind(op.status.as_str())
        .bind(op.resolved.release_id.clone())
        .bind(format_ts(&op.updated_at))
        .bind(serde_json::to_string(op)?)
        .bind(i64::from(terminal))
        .bind(op.id.clone())
        .bind(expected_version)
        .execute(&mut *tx)
        .await?;
        if res.rows_affected() != 1 {
            return Err(Error::Stale(format!("opération {}", op.id)));
        }
        if let Some(d) = deployment {
            sqlx::query(
                "INSERT INTO deployments (environment, service, release_id, operation_id, updated_at, data)
                 VALUES ($1, $2, $3, $4, $5, $6)
                 ON CONFLICT (environment, service) DO UPDATE
                 SET release_id = $3, operation_id = $4, updated_at = $5, data = $6",
            )
            .bind(d.environment.clone())
            .bind(d.service.clone())
            .bind(d.release_id.clone())
            .bind(d.operation_id.clone())
            .bind(format_ts(&d.deployed_at))
            .bind(serde_json::to_string(d)?)
            .execute(&mut *tx)
            .await?;
        }
        for a in audits {
            self.append_audit_tx(&mut tx, a).await?;
        }
        tx.commit().await?;
        Ok(expected_version + 1)
    }

    pub async fn list_operations(&self, f: &OperationFilter) -> Result<Vec<Operation>> {
        let limit = f.limit.unwrap_or(50).clamp(1, 500);
        // Filtres optionnels exprimés en SQL portable : ($n IS NULL OR col = $n).
        let rows = sqlx::query(
            "SELECT status, version, data FROM operations
             WHERE ($1 IS NULL OR environment = $1)
               AND ($2 IS NULL OR status = $2)
               AND ($3 IS NULL OR kind = $3)
               AND ($4 IS NULL OR subject = $4)
             ORDER BY created_at DESC, id DESC LIMIT $5",
        )
        .bind(f.environment.clone())
        .bind(f.status.clone())
        .bind(f.kind.clone())
        .bind(f.subject.clone())
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(|r| decode_operation(r).map(|v| v.value)).collect()
    }

    /// Réserve la plus ancienne opération prête (ou `only`), pour `worker`.
    pub async fn claim_operation(
        &self,
        worker: &str,
        lease_until_ms: i64,
        now_ms: i64,
        only: Option<&str>,
    ) -> Result<Option<String>> {
        let skip_locked = if self.postgres { " FOR UPDATE SKIP LOCKED" } else { "" };
        let sql = format!(
            "UPDATE operations
             SET status = 'running', lease_owner = $1, lease_until = $2, version = version + 1
             WHERE id = (SELECT id FROM operations
                         WHERE status = 'queued' AND (lease_until IS NULL OR lease_until < $3)
                           AND ($4 IS NULL OR id = $4)
                         ORDER BY created_at, id LIMIT 1{skip_locked})
               AND status = 'queued'
             RETURNING id"
        );
        let row = sqlx::query(&sql)
            .bind(worker.to_string())
            .bind(lease_until_ms)
            .bind(now_ms)
            .bind(only.map(str::to_string))
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| r.try_get::<String, _>("id")).transpose()?)
    }

    /// Remet une opération réservée dans la file (verrou occupé), avec délai.
    pub async fn requeue_operation(&self, id: &str, not_before_ms: i64) -> Result<()> {
        sqlx::query(
            "UPDATE operations SET status = 'queued', lease_owner = NULL, lease_until = $1, version = version + 1
             WHERE id = $2 AND status = 'running'",
        )
        .bind(not_before_ms)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn renew_lease(&self, id: &str, worker: &str, until_ms: i64) -> Result<bool> {
        let res = sqlx::query(
            "UPDATE operations SET lease_until = $1 WHERE id = $2 AND lease_owner = $3 AND status = 'running'",
        )
        .bind(until_ms)
        .bind(id.to_string())
        .bind(worker.to_string())
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() == 1)
    }

    pub async fn expired_leases(&self, now_ms: i64) -> Result<Vec<String>> {
        let rows = sqlx::query(
            "SELECT id FROM operations WHERE status = 'running' AND lease_until IS NOT NULL AND lease_until < $1",
        )
        .bind(now_ms)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(|r| Ok(r.try_get::<String, _>("id")?)).collect()
    }

    /// La release a-t-elle été déployée avec succès dans cet environnement ?
    pub async fn succeeded_in_env(&self, env: &str, service: &str, release_id: &str) -> Result<bool> {
        let row = sqlx::query(
            "SELECT COUNT(*) AS n FROM operations
             WHERE environment = $1 AND subject = $2 AND release_id = $3
               AND kind IN ('deploy', 'rollback') AND status = 'succeeded'",
        )
        .bind(env.to_string())
        .bind(service.to_string())
        .bind(release_id.to_string())
        .fetch_one(&self.pool)
        .await?;
        Ok(row.try_get::<i64, _>("n")? > 0)
    }

    /// Opérations terminées d'un type donné pour un sujet, plus récentes d'abord.
    pub async fn finished_operations(
        &self,
        env: Option<&str>,
        kinds: &[&str],
        subject: &str,
        limit: i64,
    ) -> Result<Vec<Operation>> {
        let kinds_list = kinds.iter().map(|k| format!("'{}'", k.replace('\'', ""))).collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT status, version, data FROM operations
             WHERE ($1 IS NULL OR environment = $1) AND subject = $2 AND kind IN ({kinds_list})
               AND status IN ('succeeded', 'failed', 'rolled_back')
             ORDER BY created_at DESC, id DESC LIMIT $3"
        );
        let rows = sqlx::query(&sql)
            .bind(env.map(str::to_string))
            .bind(subject.to_string())
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(|r| decode_operation(r).map(|v| v.value)).collect()
    }

    /// Une opération de ce type est-elle déjà en attente ou en cours ?
    pub async fn has_active(&self, env: &str, kind: &str, subject: &str) -> Result<bool> {
        let row = sqlx::query(
            "SELECT COUNT(*) AS n FROM operations
             WHERE environment = $1 AND kind = $2 AND subject = $3
               AND status IN ('awaiting_approval', 'queued', 'running')",
        )
        .bind(env.to_string())
        .bind(kind.to_string())
        .bind(subject.to_string())
        .fetch_one(&self.pool)
        .await?;
        Ok(row.try_get::<i64, _>("n")? > 0)
    }

    pub async fn count_operations_by_status(&self) -> Result<Vec<(String, i64)>> {
        let rows =
            sqlx::query("SELECT status, COUNT(*) AS n FROM operations GROUP BY status").fetch_all(&self.pool).await?;
        rows.iter().map(|r| Ok((r.try_get::<String, _>("status")?, r.try_get::<i64, _>("n")?))).collect()
    }

    pub async fn operations_since(&self, since: &DateTime<Utc>) -> Result<Vec<Operation>> {
        let rows = sqlx::query(
            "SELECT status, version, data FROM operations WHERE created_at >= $1 ORDER BY created_at ASC, id ASC",
        )
        .bind(format_ts(since))
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(|r| decode_operation(r).map(|v| v.value)).collect()
    }

    // -----------------------------------------------------------------------
    // Journaux d'exécution
    // -----------------------------------------------------------------------

    pub async fn append_logs(&self, operation_id: &str, lines: &[LogLine]) -> Result<()> {
        if lines.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        for l in lines {
            sqlx::query(
                "INSERT INTO operation_logs (operation_id, seq, ts, level, step, message)
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(operation_id.to_string())
            .bind(l.seq)
            .bind(format_ts(&l.ts))
            .bind(l.level.clone())
            .bind(l.step.clone())
            .bind(l.message.clone())
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn last_log_seq(&self, operation_id: &str) -> Result<i64> {
        let row = sqlx::query("SELECT MAX(seq) AS s FROM operation_logs WHERE operation_id = $1")
            .bind(operation_id.to_string())
            .fetch_one(&self.pool)
            .await?;
        Ok(row.try_get::<Option<i64>, _>("s")?.unwrap_or(0))
    }

    pub async fn logs(&self, operation_id: &str, after_seq: i64, limit: i64) -> Result<Vec<LogLine>> {
        let rows = sqlx::query(
            "SELECT seq, ts, level, step, message FROM operation_logs
             WHERE operation_id = $1 AND seq > $2 ORDER BY seq ASC LIMIT $3",
        )
        .bind(operation_id.to_string())
        .bind(after_seq)
        .bind(limit.clamp(1, 5_000))
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|r| {
                let ts: String = r.try_get("ts")?;
                Ok(LogLine {
                    seq: r.try_get("seq")?,
                    ts: parse_ts(&ts)?,
                    level: r.try_get("level")?,
                    step: r.try_get("step")?,
                    message: r.try_get("message")?,
                })
            })
            .collect()
    }

    // -----------------------------------------------------------------------
    // Releases et déploiements
    // -----------------------------------------------------------------------

    pub async fn insert_release(&self, r: &Release, audit: AuditDraft) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let res = sqlx::query(
            "INSERT INTO releases (id, service, digest, version, created_at, data) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(r.id.clone())
        .bind(r.service.clone())
        .bind(r.digest.clone())
        .bind(r.version.clone())
        .bind(format_ts(&r.created_at))
        .bind(serde_json::to_string(r)?)
        .execute(&mut *tx)
        .await;
        if let Err(e) = res {
            return Err(if is_unique_violation(&e) {
                CoreError::Conflict(format!("le digest {} est déjà enregistré pour {}", r.digest, r.service)).into()
            } else {
                e.into()
            });
        }
        self.append_audit_tx(&mut tx, audit).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn get_release(&self, id: &str) -> Result<Option<Release>> {
        let row = sqlx::query("SELECT data FROM releases WHERE id = $1")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(decode).transpose()
    }

    /// Recherche par identifiant, digest ou version (la plus récente).
    pub async fn find_release(&self, service: &str, reference: &str) -> Result<Option<Release>> {
        let row = sqlx::query(
            "SELECT data FROM releases
             WHERE service = $1 AND (id = $2 OR digest = $2 OR version = $2)
             ORDER BY created_at DESC, id DESC LIMIT 1",
        )
        .bind(service.to_string())
        .bind(reference.to_string())
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(decode).transpose()
    }

    pub async fn list_releases(&self, service: Option<&str>, limit: i64) -> Result<Vec<Release>> {
        let rows = sqlx::query(
            "SELECT data FROM releases WHERE ($1 IS NULL OR service = $1) ORDER BY created_at DESC, id DESC LIMIT $2",
        )
        .bind(service.map(str::to_string))
        .bind(limit.clamp(1, 500))
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(decode).collect()
    }

    pub async fn get_deployment(&self, env: &str, service: &str) -> Result<Option<Deployment>> {
        let row = sqlx::query("SELECT data FROM deployments WHERE environment = $1 AND service = $2")
            .bind(env.to_string())
            .bind(service.to_string())
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(decode).transpose()
    }

    pub async fn list_deployments(&self) -> Result<Vec<Deployment>> {
        let rows =
            sqlx::query("SELECT data FROM deployments ORDER BY environment, service").fetch_all(&self.pool).await?;
        rows.iter().map(decode).collect()
    }

    // -----------------------------------------------------------------------
    // Incidents
    // -----------------------------------------------------------------------

    pub async fn insert_incident(&self, i: &Incident, audit: AuditDraft) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO incidents (id, status, severity, environment, fingerprint, created_at, updated_at, version, data)
             VALUES ($1, $2, $3, $4, $5, $6, $7, 1, $8)",
        )
        .bind(i.id.clone())
        .bind(i.status.as_str())
        .bind(i.severity.as_str())
        .bind(i.environment.clone())
        .bind(i.fingerprint.clone())
        .bind(format_ts(&i.created_at))
        .bind(format_ts(&i.updated_at))
        .bind(serde_json::to_string(i)?)
        .execute(&mut *tx)
        .await?;
        self.append_audit_tx(&mut tx, audit).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn get_incident(&self, id: &str) -> Result<Option<Versioned<Incident>>> {
        let row = sqlx::query("SELECT version, data FROM incidents WHERE id = $1")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        row.map(|r| Ok(Versioned { value: decode(&r)?, version: r.try_get("version")? })).transpose()
    }

    pub async fn update_incident(&self, i: &Incident, expected_version: i64, audit: AuditDraft) -> Result<i64> {
        let mut tx = self.pool.begin().await?;
        let res = sqlx::query(
            "UPDATE incidents SET status = $1, severity = $2, updated_at = $3, data = $4, version = version + 1
             WHERE id = $5 AND version = $6",
        )
        .bind(i.status.as_str())
        .bind(i.severity.as_str())
        .bind(format_ts(&i.updated_at))
        .bind(serde_json::to_string(i)?)
        .bind(i.id.clone())
        .bind(expected_version)
        .execute(&mut *tx)
        .await?;
        if res.rows_affected() != 1 {
            return Err(Error::Stale(format!("incident {}", i.id)));
        }
        self.append_audit_tx(&mut tx, audit).await?;
        tx.commit().await?;
        Ok(expected_version + 1)
    }

    pub async fn list_incidents(&self, active_only: bool, limit: i64) -> Result<Vec<Incident>> {
        let rows = sqlx::query(
            "SELECT data FROM incidents
             WHERE ($1 = 0 OR status IN ('open', 'acknowledged', 'mitigated'))
             ORDER BY created_at DESC, id DESC LIMIT $2",
        )
        .bind(i64::from(active_only))
        .bind(limit.clamp(1, 500))
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(decode).collect()
    }

    pub async fn incidents_since(&self, since: &DateTime<Utc>) -> Result<Vec<Incident>> {
        let rows = sqlx::query("SELECT data FROM incidents WHERE created_at >= $1 ORDER BY created_at ASC, id ASC")
            .bind(format_ts(since))
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(decode).collect()
    }

    pub async fn find_active_incident(&self, fingerprint: &str) -> Result<Option<Versioned<Incident>>> {
        let row = sqlx::query(
            "SELECT version, data FROM incidents
             WHERE fingerprint = $1 AND status IN ('open', 'acknowledged', 'mitigated')
             ORDER BY created_at DESC, id DESC LIMIT 1",
        )
        .bind(fingerprint.to_string())
        .fetch_optional(&self.pool)
        .await?;
        row.map(|r| Ok(Versioned { value: decode(&r)?, version: r.try_get("version")? })).transpose()
    }

    // -----------------------------------------------------------------------
    // Maintenance
    // -----------------------------------------------------------------------

    pub async fn insert_maintenance(&self, m: &MaintenanceWindow, audit: AuditDraft) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO maintenance_windows (id, environment, starts_at, ends_at, status, version, data)
             VALUES ($1, $2, $3, $4, $5, 1, $6)",
        )
        .bind(m.id.clone())
        .bind(m.environment.clone())
        .bind(format_ts(&m.starts_at))
        .bind(format_ts(&m.ends_at))
        .bind(m.status.as_str())
        .bind(serde_json::to_string(m)?)
        .execute(&mut *tx)
        .await?;
        self.append_audit_tx(&mut tx, audit).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn get_maintenance(&self, id: &str) -> Result<Option<Versioned<MaintenanceWindow>>> {
        let row = sqlx::query("SELECT version, data FROM maintenance_windows WHERE id = $1")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        row.map(|r| Ok(Versioned { value: decode(&r)?, version: r.try_get("version")? })).transpose()
    }

    pub async fn update_maintenance(
        &self,
        m: &MaintenanceWindow,
        expected_version: i64,
        audit: AuditDraft,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let res = sqlx::query(
            "UPDATE maintenance_windows SET status = $1, ends_at = $2, data = $3, version = version + 1
             WHERE id = $4 AND version = $5",
        )
        .bind(m.status.as_str())
        .bind(format_ts(&m.ends_at))
        .bind(serde_json::to_string(m)?)
        .bind(m.id.clone())
        .bind(expected_version)
        .execute(&mut *tx)
        .await?;
        if res.rows_affected() != 1 {
            return Err(Error::Stale(format!("maintenance {}", m.id)));
        }
        self.append_audit_tx(&mut tx, audit).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn list_maintenance(&self, active_only: bool, limit: i64) -> Result<Vec<MaintenanceWindow>> {
        let rows = sqlx::query(
            "SELECT data FROM maintenance_windows WHERE ($1 = 0 OR status = 'active')
             ORDER BY starts_at DESC LIMIT $2",
        )
        .bind(i64::from(active_only))
        .bind(limit.clamp(1, 500))
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(decode).collect()
    }

    pub async fn active_maintenance(&self, env: &str, now: &DateTime<Utc>) -> Result<Option<MaintenanceWindow>> {
        let now = format_ts(now);
        let row = sqlx::query(
            "SELECT data FROM maintenance_windows
             WHERE environment = $1 AND status = 'active' AND starts_at <= $2 AND ends_at > $2
             ORDER BY starts_at DESC LIMIT 1",
        )
        .bind(env.to_string())
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(decode).transpose()
    }

    pub async fn expired_maintenance(&self, now: &DateTime<Utc>) -> Result<Vec<String>> {
        let rows = sqlx::query("SELECT id FROM maintenance_windows WHERE status = $1 AND ends_at <= $2")
            .bind(MaintenanceStatus::Active.as_str())
            .bind(format_ts(now))
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(|r| Ok(r.try_get::<String, _>("id")?)).collect()
    }

    // -----------------------------------------------------------------------
    // Verrous (baux à durée limitée, non bloquants)
    // -----------------------------------------------------------------------

    /// Acquiert tous les verrous ou aucun. Réentrant pour un même détenteur.
    pub async fn try_locks(
        &self,
        scopes: &[String],
        holder: &str,
        operation_id: &str,
        ttl_ms: i64,
        now_ms: i64,
    ) -> Result<bool> {
        let mut acquired: Vec<&String> = Vec::new();
        for scope in scopes {
            sqlx::query("DELETE FROM locks WHERE scope = $1 AND expires_at < $2")
                .bind(scope.clone())
                .bind(now_ms)
                .execute(&self.pool)
                .await?;
            sqlx::query(
                "INSERT INTO locks (scope, holder, operation_id, acquired_at, expires_at)
                 VALUES ($1, $2, $3, $4, $5) ON CONFLICT (scope) DO NOTHING",
            )
            .bind(scope.clone())
            .bind(holder.to_string())
            .bind(operation_id.to_string())
            .bind(format_ts(&now_micros()))
            .bind(now_ms + ttl_ms)
            .execute(&self.pool)
            .await?;
            let row = sqlx::query("SELECT holder FROM locks WHERE scope = $1")
                .bind(scope.clone())
                .fetch_optional(&self.pool)
                .await?;
            let owner: Option<String> = row.map(|r| r.try_get("holder")).transpose()?;
            if owner.as_deref() == Some(holder) {
                acquired.push(scope);
            } else {
                for s in acquired {
                    self.release_lock(s, holder).await?;
                }
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub async fn renew_locks(&self, holder: &str, until_ms: i64) -> Result<()> {
        sqlx::query("UPDATE locks SET expires_at = $1 WHERE holder = $2")
            .bind(until_ms)
            .bind(holder.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn release_lock(&self, scope: &str, holder: &str) -> Result<()> {
        sqlx::query("DELETE FROM locks WHERE scope = $1 AND holder = $2")
            .bind(scope.to_string())
            .bind(holder.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn release_locks(&self, holder: &str) -> Result<()> {
        sqlx::query("DELETE FROM locks WHERE holder = $1").bind(holder.to_string()).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn release_locks_for_operation(&self, operation_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM locks WHERE operation_id = $1")
            .bind(operation_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_locks(&self) -> Result<Vec<serde_json::Value>> {
        let rows = sqlx::query("SELECT scope, holder, operation_id, acquired_at, expires_at FROM locks ORDER BY scope")
            .fetch_all(&self.pool)
            .await?;
        rows.iter()
            .map(|r| {
                Ok(serde_json::json!({
                    "scope": r.try_get::<String, _>("scope")?,
                    "holder": r.try_get::<String, _>("holder")?,
                    "operation_id": r.try_get::<Option<String>, _>("operation_id")?,
                    "acquired_at": r.try_get::<String, _>("acquired_at")?,
                    "expires_at_ms": r.try_get::<i64, _>("expires_at")?,
                }))
            })
            .collect()
    }

    // -----------------------------------------------------------------------
    // Jetons d'API
    // -----------------------------------------------------------------------

    pub async fn insert_token(&self, t: &ApiToken, token_hash: &str, audit: AuditDraft) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO api_tokens (id, token_hash, subject, revoked, created_at, last_used_at, data)
             VALUES ($1, $2, $3, 0, $4, NULL, $5)",
        )
        .bind(t.id.clone())
        .bind(token_hash.to_string())
        .bind(t.subject.clone())
        .bind(format_ts(&t.created_at))
        .bind(serde_json::to_string(t)?)
        .execute(&mut *tx)
        .await?;
        self.append_audit_tx(&mut tx, audit).await?;
        tx.commit().await?;
        Ok(())
    }

    fn token_from_row(r: &AnyRow) -> Result<ApiToken> {
        let mut t: ApiToken = decode(r)?;
        t.revoked = r.try_get::<i64, _>("revoked")? != 0;
        t.last_used_at = r.try_get::<Option<String>, _>("last_used_at")?.as_deref().map(parse_ts).transpose()?;
        Ok(t)
    }

    pub async fn find_token(&self, token_hash: &str) -> Result<Option<ApiToken>> {
        let row = sqlx::query("SELECT revoked, last_used_at, data FROM api_tokens WHERE token_hash = $1")
            .bind(token_hash.to_string())
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(Self::token_from_row).transpose()
    }

    pub async fn touch_token(&self, id: &str, now: &DateTime<Utc>) -> Result<()> {
        sqlx::query("UPDATE api_tokens SET last_used_at = $1 WHERE id = $2")
            .bind(format_ts(now))
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_tokens(&self) -> Result<Vec<ApiToken>> {
        let rows = sqlx::query("SELECT revoked, last_used_at, data FROM api_tokens ORDER BY created_at DESC")
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(Self::token_from_row).collect()
    }

    pub async fn revoke_token(&self, id: &str, audit: AuditDraft) -> Result<bool> {
        let mut tx = self.pool.begin().await?;
        let res = sqlx::query("UPDATE api_tokens SET revoked = 1 WHERE id = $1 AND revoked = 0")
            .bind(id.to_string())
            .execute(&mut *tx)
            .await?;
        if res.rows_affected() == 1 {
            self.append_audit_tx(&mut tx, audit).await?;
        }
        tx.commit().await?;
        Ok(res.rows_affected() == 1)
    }
}
