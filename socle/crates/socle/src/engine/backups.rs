//! Sauvegardes et ordonnanceur : cadence des sauvegardes et des exercices,
//! surveillance du RPO, détection de dérive d'infrastructure, nettoyage.

use socle_core::backup::{self, BackupRun, BackupStatus, RunKind};
use socle_core::incident::{IncidentSource, IncidentStatus, NewIncident, Severity};
use socle_core::operation::{Action, OpStatus, OperationKind, OperationRequest};
use socle_core::rbac::{Permission, Principal};
use socle_core::spec::Tier;

use super::Engine;
use crate::error::Result;
use crate::notify::Event;

impl Engine {
    pub(crate) async fn backup_status_for(&self, name: &str) -> Result<Option<BackupStatus>> {
        let spec = self.spec();
        let Some(policy) = spec.backups.get(name) else {
            return Ok(None);
        };
        let ops =
            self.store.finished_operations(Some(&policy.environment), &["backup", "restore_drill"], name, 500).await?;
        let runs: Vec<BackupRun> = ops
            .iter()
            .filter_map(|o| {
                Some(BackupRun {
                    kind: if o.kind() == OperationKind::Backup { RunKind::Backup } else { RunKind::Drill },
                    succeeded: o.status == OpStatus::Succeeded,
                    started_at: o.started_at?,
                    finished_at: o.finished_at?,
                })
            })
            .collect();
        Ok(Some(backup::status(name, policy, &runs, self.now())))
    }

    pub async fn backup_statuses(&self, p: &Principal) -> Result<Vec<BackupStatus>> {
        let spec = self.spec();
        self.authorize(&spec, p, Permission::Read, None, None)?;
        let mut out = Vec::new();
        for name in spec.backups.keys() {
            if let Some(s) = self.backup_status_for(name).await? {
                out.push(s);
            }
        }
        Ok(out)
    }

    async fn submit_system(&self, system: &Principal, environment: &str, action: Action, reason: &str) {
        let req = OperationRequest {
            environment: environment.to_string(),
            action,
            reason: reason.to_string(),
            change_ticket: None,
            emergency: false,
        };
        if let Err(e) = self.submit(system, req).await {
            tracing::warn!("ordonnanceur : soumission impossible ({reason}) : {e}");
        }
    }

    /// Un tour d'ordonnancement. En haute disponibilité, un seul réplica
    /// (détenteur du verrou `scheduler`) agit.
    pub async fn scheduler_tick(&self) -> Result<()> {
        let now_ms = self.now_ms();
        if !self.store.try_locks(&["scheduler".to_string()], &self.cfg.worker_id, "scheduler", 60_000, now_ms).await? {
            return Ok(());
        }
        let spec = self.spec();
        let system = Principal::system("scheduler");
        let now = self.now();

        for (name, policy) in &spec.backups {
            let Some(status) = self.backup_status_for(name).await? else {
                continue;
            };
            let env = policy.environment.as_str();
            if now >= status.next_due && !self.store.has_active(env, "backup", name).await? {
                self.submit_system(&system, env, Action::Backup { policy: name.clone() }, "Sauvegarde planifiée").await;
            }
            if let Some(d) = &status.drill
                && now >= d.next_due
                && status.last_success.is_some()
                && !self.store.has_active(env, "restore_drill", name).await?
            {
                self.submit_system(
                    &system,
                    env,
                    Action::RestoreDrill { policy: name.clone() },
                    "Exercice de restauration planifié",
                )
                .await;
            }

            // Surveillance du RPO (après la première tentative).
            let fingerprint = format!("rpo:{name}");
            if !status.rpo_compliant && status.last_attempt.is_some() {
                if self.store.find_active_incident(&fingerprint).await?.is_none() {
                    let production = spec.environments.get(env).is_some_and(|e| e.tier == Tier::Production);
                    let age = status.age_secs.map_or_else(
                        || "aucune sauvegarde réussie".to_string(),
                        |a| format!("dernière réussite il y a {} min", a / 60),
                    );
                    let incident = self
                        .open_system_incident(
                            NewIncident {
                                title: format!("RPO dépassé : sauvegarde {name}"),
                                severity: if production { Severity::Sev2 } else { Severity::Sev3 },
                                environment: Some(env.to_string()),
                                services: Vec::new(),
                                summary: format!("Objectif {}s, {age}.", status.rpo_secs),
                            },
                            IncidentSource::Backup { policy: name.clone() },
                            fingerprint,
                            None,
                        )
                        .await?;
                    let mut e = Event::new("backup.rpo_breach", format!("RPO dépassé pour la sauvegarde {name}"));
                    e.environment = Some(env.to_string());
                    e.severity = Some(incident.severity);
                    e.link = Some(format!("#/incidents/{}", incident.id));
                    self.notify(e);
                }
            } else if status.rpo_compliant
                && let Some(active) = self.store.find_active_incident(&fingerprint).await?
            {
                let _ = self
                    .transition_incident(
                        &Principal::system("scheduler"),
                        &active.value.id,
                        IncidentStatus::Resolved,
                        Some("RPO de nouveau respecté"),
                    )
                    .await;
            }
        }

        // Détection de dérive : plans d'infrastructure planifiés.
        for (name, stack) in &spec.stacks {
            let Some(every) = stack.drift_check else {
                continue;
            };
            for env in &stack.environments {
                let last = self.store.finished_operations(Some(env), &["infra_plan"], name, 1).await?;
                let due = last.first().and_then(|o| o.finished_at).is_none_or(|t| t + every.as_chrono() <= now);
                if due && !self.store.has_active(env, "infra_plan", name).await? {
                    self.submit_system(
                        &system,
                        env,
                        Action::InfraPlan { stack: name.clone() },
                        "Détection de dérive planifiée",
                    )
                    .await;
                }
            }
        }

        self.reap_expired().await?;
        self.expire_maintenance().await?;
        Ok(())
    }
}
