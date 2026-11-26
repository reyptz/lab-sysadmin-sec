//! Moteur applicatif : unique implémentation de la logique métier, partagée
//! par l'API (console, intégrations), la CLI distante et le mode local.

mod backups;
mod incidents;
mod maintenance;
mod operations;
mod views;

use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;
use socle_core::audit::AuditDraft;
use socle_core::incident::Severity;
use socle_core::rbac::{self, Permission, Principal, PrincipalKind};
use socle_core::release::{NewRelease, Release};
use socle_core::spec::Spec;
use socle_core::{CoreError, truncate_micros};
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::executor::StepRunner;
use crate::model::ApiToken;
use crate::notify::{Event, Notifier};
use crate::store::Store;

pub use incidents::IncidentPatch;
pub use operations::Prepared;
pub use views::{ComplianceReport, EnvironmentView, Overview, ServiceView};

pub type Clock = Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>;

#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Racine du dépôt de travail (GitOps, Ansible, Terraform).
    pub repo_root: PathBuf,
    /// Identifiant unique de cette instance (réservation des opérations).
    pub worker_id: String,
    /// Durée de bail d'une opération en cours (renouvelé automatiquement).
    pub lease: Duration,
    /// URL publique de la console (liens des notifications).
    pub public_url: Option<String>,
    /// Gravité minimale d'une alerte pour ouvrir un incident.
    pub incident_min_severity: Severity,
}

impl EngineConfig {
    pub fn new(repo_root: PathBuf) -> Self {
        let host = std::env::var("HOSTNAME").unwrap_or_else(|_| "socle".into());
        Self {
            repo_root,
            worker_id: format!("{host}-{}", &Uuid::new_v4().simple().to_string()[..8]),
            lease: Duration::from_secs(120),
            public_url: None,
            incident_min_severity: Severity::Sev3,
        }
    }
}

pub struct Engine {
    spec: RwLock<Arc<Spec>>,
    spec_path: Option<PathBuf>,
    store: Store,
    runner: Arc<dyn StepRunner>,
    notifier: Arc<Notifier>,
    clock: Clock,
    cfg: EngineConfig,
    http: reqwest::Client,
}

impl Engine {
    pub fn new(
        spec: Spec,
        spec_path: Option<PathBuf>,
        store: Store,
        runner: Arc<dyn StepRunner>,
        cfg: EngineConfig,
    ) -> Self {
        let notifier = Arc::new(Notifier::new(cfg.public_url.clone()));
        Self {
            spec: RwLock::new(Arc::new(spec)),
            spec_path,
            store,
            runner,
            notifier,
            clock: Arc::new(Utc::now),
            cfg,
            http: reqwest::Client::builder().timeout(Duration::from_secs(15)).build().unwrap_or_default(),
        }
    }

    /// Horloge injectable (tests des fenêtres de changement).
    #[must_use]
    pub fn with_clock(mut self, clock: Clock) -> Self {
        self.clock = clock;
        self
    }

    pub fn now(&self) -> DateTime<Utc> {
        truncate_micros((self.clock)())
    }

    fn now_ms(&self) -> i64 {
        self.now().timestamp_millis()
    }

    pub fn spec(&self) -> Arc<Spec> {
        match self.spec.read() {
            Ok(g) => Arc::clone(&g),
            Err(poisoned) => Arc::clone(&poisoned.into_inner()),
        }
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    pub fn config(&self) -> &EngineConfig {
        &self.cfg
    }

    /// Recharge `socle.yaml` (validation complète avant bascule).
    pub async fn reload_spec(&self, p: &Principal) -> Result<String> {
        let spec = self.spec();
        self.authorize(&spec, p, Permission::Admin, None, None)?;
        let path = self.spec_path.as_ref().ok_or_else(|| Error::internal("aucun fichier de spécification associé"))?;
        let text = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| Error::internal(format!("lecture de {} : {e}", path.display())))?;
        let new = Spec::from_yaml(&text)?;
        let hash = new.hash();
        let old_hash = spec.hash();
        match self.spec.write() {
            Ok(mut g) => *g = Arc::new(new),
            Err(poisoned) => *poisoned.into_inner() = Arc::new(new),
        }
        self.store
            .append_audit(self.audit(
                p,
                "spec.reloaded",
                "spec",
                "success",
                serde_json::json!({"from": old_hash, "to": hash}),
            ))
            .await?;
        Ok(hash)
    }

    // -----------------------------------------------------------------------
    // Utilitaires
    // -----------------------------------------------------------------------

    pub(crate) fn authorize(
        &self,
        spec: &Spec,
        p: &Principal,
        perm: Permission,
        env: Option<&str>,
        service: Option<&str>,
    ) -> Result<()> {
        if rbac::is_allowed(spec, p, perm, env, service) {
            return Ok(());
        }
        let scope = match (env, service) {
            (Some(e), Some(s)) => format!(" sur {s} en {e}"),
            (Some(e), None) => format!(" en {e}"),
            (None, Some(s)) => format!(" sur {s}"),
            (None, None) => String::new(),
        };
        Err(CoreError::forbidden(format!(
            "{} n'a pas la permission « {} »{scope}",
            p.id,
            serde_json::to_value(perm).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
        ))
        .into())
    }

    pub(crate) fn audit(
        &self,
        p: &Principal,
        action: &str,
        resource: &str,
        outcome: &str,
        details: serde_json::Value,
    ) -> AuditDraft {
        AuditDraft::new(p.id.clone(), action, resource, outcome, details)
    }

    pub(crate) fn notify(&self, event: Event) {
        self.notifier.dispatch(self.spec(), event);
    }

    // -----------------------------------------------------------------------
    // Releases
    // -----------------------------------------------------------------------

    /// Enregistre une release (idempotent : un même digest renvoie la
    /// release existante, dont les preuves sont immuables).
    pub async fn register_release(&self, p: &Principal, input: NewRelease) -> Result<Release> {
        let spec = self.spec();
        let svc = spec.service(&input.service)?;
        self.authorize(&spec, p, Permission::RegisterRelease, None, Some(&input.service))?;
        input.validate(&svc.image)?;
        if let Some(existing) = self.store.find_release(&input.service, &input.digest).await? {
            return Ok(existing);
        }
        let release = Release {
            id: Uuid::now_v7().to_string(),
            service: input.service,
            version: input.version,
            image: svc.image.clone(),
            digest: input.digest,
            git_sha: input.git_sha,
            created_by: p.id.clone(),
            created_at: self.now(),
            evidence: input.evidence,
        };
        let audit = self.audit(
            p,
            "release.registered",
            &format!("release/{}", release.id),
            "success",
            serde_json::json!({
                "service": release.service, "version": release.version,
                "digest": release.digest, "evidence": release.evidence,
            }),
        );
        match self.store.insert_release(&release, audit).await {
            Ok(()) => Ok(release),
            // Course entre deux enregistrements identiques : renvoie l'existant.
            Err(Error::Core(CoreError::Conflict(_))) => self
                .store
                .find_release(&release.service, &release.digest)
                .await?
                .ok_or_else(|| Error::internal("release introuvable après conflit")),
            Err(e) => Err(e),
        }
    }

    pub async fn resolve_release(&self, service: &str, reference: &str) -> Result<Release> {
        self.store
            .find_release(service, reference)
            .await?
            .ok_or_else(|| CoreError::not_found(format!("release « {reference} » de {service}")).into())
    }

    pub async fn list_releases(&self, p: &Principal, service: Option<&str>, limit: i64) -> Result<Vec<Release>> {
        let spec = self.spec();
        self.authorize(&spec, p, Permission::Read, None, service)?;
        self.store.list_releases(service, limit).await
    }

    pub(crate) async fn current_release(&self, env: &str, service: &str) -> Result<Option<Release>> {
        match self.store.get_deployment(env, service).await? {
            Some(d) => self.store.get_release(&d.release_id).await,
            None => Ok(None),
        }
    }

    // -----------------------------------------------------------------------
    // Jetons d'API
    // -----------------------------------------------------------------------

    /// Crée un jeton. `p = None` : amorçage (premier administrateur), réservé
    /// à l'accès direct à la base.
    pub async fn create_token(
        &self,
        p: Option<&Principal>,
        name: &str,
        subject: &str,
        teams: Vec<String>,
        ttl: Option<socle_core::duration::HumanDuration>,
    ) -> Result<(ApiToken, String)> {
        let spec = self.spec();
        let actor = match p {
            Some(p) => {
                self.authorize(&spec, p, Permission::Admin, None, None)?;
                p.clone()
            }
            None => Principal::system("bootstrap"),
        };
        if name.trim().is_empty() || name.len() > 100 || subject.trim().is_empty() || subject.len() > 200 {
            return Err(CoreError::invalid("nom ou sujet de jeton invalide").into());
        }
        if teams.is_empty() {
            return Err(CoreError::invalid("un jeton doit appartenir à au moins une équipe").into());
        }
        for t in &teams {
            if !spec.teams.contains_key(t) {
                return Err(CoreError::invalid(format!("équipe inconnue « {t} »")).into());
            }
        }
        let mut raw = [0u8; 32];
        getrandom::fill(&mut raw).map_err(|e| Error::internal(format!("aléa indisponible : {e}")))?;
        let secret = format!("socle_pat_{}", hex::encode(raw));
        let now = self.now();
        let token = ApiToken {
            id: Uuid::now_v7().to_string(),
            name: name.trim().to_string(),
            subject: subject.trim().to_string(),
            teams: teams.into_iter().collect(),
            created_by: actor.id.clone(),
            created_at: now,
            expires_at: ttl.map(|d| now + d.as_chrono()),
            revoked: false,
            last_used_at: None,
        };
        let audit = self.audit(
            &actor,
            "token.created",
            &format!("token/{}", token.id),
            "success",
            serde_json::json!({"name": token.name, "subject": token.subject, "teams": token.teams, "expires_at": token.expires_at}),
        );
        self.store.insert_token(&token, &hash_token(&secret), audit).await?;
        Ok((token, secret))
    }

    pub async fn list_tokens(&self, p: &Principal) -> Result<Vec<ApiToken>> {
        let spec = self.spec();
        self.authorize(&spec, p, Permission::Admin, None, None)?;
        self.store.list_tokens().await
    }

    pub async fn revoke_token(&self, p: &Principal, id: &str) -> Result<()> {
        let spec = self.spec();
        self.authorize(&spec, p, Permission::Admin, None, None)?;
        let audit = self.audit(p, "token.revoked", &format!("token/{id}"), "success", serde_json::Value::Null);
        if self.store.revoke_token(id, audit).await? {
            Ok(())
        } else {
            Err(CoreError::not_found(format!("jeton actif {id}")).into())
        }
    }

    /// Authentifie un jeton d'API et construit le principal correspondant.
    pub async fn authenticate_token(&self, secret: &str) -> Result<Principal> {
        let token = self
            .store
            .find_token(&hash_token(secret))
            .await?
            .ok_or_else(|| Error::Unauthenticated("jeton inconnu".into()))?;
        let now = self.now();
        if token.revoked {
            return Err(Error::Unauthenticated("jeton révoqué".into()));
        }
        if token.expires_at.is_some_and(|e| e <= now) {
            return Err(Error::Unauthenticated("jeton expiré".into()));
        }
        // Mise à jour opportuniste de la dernière utilisation (au plus 1/min).
        if token.last_used_at.is_none_or(|t| now - t > chrono::TimeDelta::minutes(1)) {
            let _ = self.store.touch_token(&token.id, &now).await;
        }
        let spec = self.spec();
        let mut teams = rbac::resolve_teams(&spec, &token.subject, &[]);
        teams.extend(token.teams.iter().filter(|t| spec.teams.contains_key(*t)).cloned());
        Ok(Principal {
            id: token.subject.clone(),
            display_name: format!("{} (jeton {})", token.subject, token.name),
            kind: PrincipalKind::Token,
            teams,
        })
    }
}

pub fn hash_token(secret: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(secret.as_bytes()))
}

/// Ce que peut faire l'appelant (pour adapter la console).
#[derive(Debug, Clone, Serialize)]
pub struct WhoAmI {
    pub principal: Principal,
    pub bindings: Vec<socle_core::spec::AccessBinding>,
    pub is_admin: bool,
}

impl Engine {
    pub fn whoami(&self, p: &Principal) -> WhoAmI {
        let spec = self.spec();
        WhoAmI {
            principal: p.clone(),
            bindings: rbac::effective_bindings(&spec, p).into_iter().cloned().collect(),
            is_admin: rbac::is_allowed(&spec, p, Permission::Admin, None, None),
        }
    }
}
