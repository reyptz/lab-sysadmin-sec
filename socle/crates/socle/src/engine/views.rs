//! Vues agrégées : environnements (avec détection de dérive GitOps), tableau
//! de bord, audit et rapport de conformité (ISO 27001 / SOC 2).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use chrono::{DateTime, Utc};
use serde::Serialize;
use socle_core::audit::{AuditEvent, ChainReport, GENESIS_HASH, verify_chain};
use socle_core::backup::BackupStatus;
use socle_core::incident::{Incident, IncidentStatus, Severity};
use socle_core::operation::{ApprovalDecision, GateStatus, OpStatus, Operation, OperationKind};
use socle_core::rbac::{Permission, Principal};
use socle_core::spec::{Target, Tier};
use socle_core::{format_ts, kustomize, windows};

use super::Engine;
use crate::error::Result;
use crate::model::{ApiToken, MaintenanceWindow, OperationFilter};

#[derive(Debug, Clone, Serialize)]
pub struct ServiceView {
    pub name: String,
    pub team: String,
    pub target: String,
    pub target_kind: String,
    pub provider: String,
    pub strategy: String,
    pub url: Option<String>,
    pub release_id: Option<String>,
    pub version: Option<String>,
    pub image_ref: Option<String>,
    pub deployed_at: Option<DateTime<Utc>>,
    pub deployed_by: Option<String>,
    /// Écart entre l'état connu de socle et le dépôt GitOps.
    pub drift: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnvironmentView {
    pub name: String,
    pub tier: Tier,
    pub policy: String,
    pub description: Option<String>,
    pub promote_from: Option<String>,
    pub change_window_open: bool,
    pub next_window: Option<DateTime<Utc>>,
    pub freeze: Option<String>,
    pub maintenance: Option<MaintenanceWindow>,
    pub services: Vec<ServiceView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Overview {
    pub platform: String,
    pub spec_hash: String,
    pub environments: Vec<EnvironmentView>,
    pub awaiting_approval: Vec<Operation>,
    pub running: Vec<Operation>,
    pub recent: Vec<Operation>,
    pub open_incidents: Vec<Incident>,
    pub backups: Vec<BackupStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ControlCheck {
    pub reference: String,
    pub title: String,
    pub status: String,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComplianceReport {
    pub platform: String,
    pub generated_at: DateTime<Utc>,
    pub period_days: i64,
    pub spec_hash: String,
    pub changes_total: usize,
    pub changes_by_kind: BTreeMap<String, usize>,
    pub changes_by_status: BTreeMap<String, usize>,
    pub production_changes: usize,
    pub production_changes_approved: usize,
    pub emergency_changes: Vec<String>,
    pub gate_rejections: usize,
    pub incidents_by_severity: BTreeMap<String, usize>,
    pub mean_time_to_acknowledge_min: Option<f64>,
    pub mean_time_to_resolve_min: Option<f64>,
    pub postmortems_pending: Vec<String>,
    pub backups: Vec<BackupStatus>,
    pub audit_chain: ChainReport,
    pub access_review: Vec<serde_json::Value>,
    pub tokens: Vec<ApiToken>,
    pub controls: Vec<ControlCheck>,
}

fn average(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

impl Engine {
    pub async fn environments(&self, p: &Principal) -> Result<Vec<EnvironmentView>> {
        let spec = self.spec();
        self.authorize(&spec, p, Permission::Read, None, None)?;
        let now = self.now();
        let deployments = self.store.list_deployments().await?;
        let mut out = Vec::new();
        for (name, env) in &spec.environments {
            let mut services = Vec::new();
            for (svc_name, svc) in &spec.services {
                let Some(se) = svc.environments.get(name) else { continue };
                let Some(target) = spec.targets.get(&se.target) else { continue };
                let d = deployments.iter().find(|d| &d.environment == name && &d.service == svc_name);
                let mut drift = None;
                if let (Target::Kubernetes(_), Some(g), Some(d)) = (target, &svc.gitops, d) {
                    let file = self
                        .cfg
                        .repo_root
                        .join(&spec.platform.gitops_repo)
                        .join(se.overlay(g, name, svc_name))
                        .join("kustomization.yaml");
                    if let Ok(content) = tokio::fs::read_to_string(&file).await {
                        let image = g.image_name.as_deref().unwrap_or(svc_name);
                        let in_git = kustomize::read_image(&content, image).and_then(|i| i.digest);
                        let expected = d.image_ref.rsplit('@').next().map(str::to_string);
                        if in_git.is_some() && in_git != expected {
                            drift = Some(format!(
                                "le dépôt GitOps référence {} au lieu de {} (modification hors socle)",
                                in_git.unwrap_or_default(),
                                expected.unwrap_or_default()
                            ));
                        }
                    }
                }
                services.push(ServiceView {
                    name: svc_name.clone(),
                    team: svc.team.clone(),
                    target: se.target.clone(),
                    target_kind: target.kind_name().to_string(),
                    provider: target.provider().to_string(),
                    strategy: se.strategy.as_str().to_string(),
                    url: se.url.clone(),
                    release_id: d.map(|d| d.release_id.clone()),
                    version: d.map(|d| d.version.clone()),
                    image_ref: d.map(|d| d.image_ref.clone()),
                    deployed_at: d.map(|d| d.deployed_at),
                    deployed_by: d.map(|d| d.deployed_by.clone()),
                    drift,
                });
            }
            out.push(EnvironmentView {
                name: name.clone(),
                tier: env.tier,
                policy: env.policy.clone(),
                description: env.description.clone(),
                promote_from: env.promote_from.clone(),
                change_window_open: windows::windows_open(&env.change_windows, now),
                next_window: windows::next_opening(&env.change_windows, now),
                freeze: windows::active_freeze(&env.freezes, now).map(|f| f.reason.clone()),
                maintenance: self.store.active_maintenance(name, &now).await?,
                services,
            });
        }
        out.sort_by_key(|e| (e.tier, e.name.clone()));
        Ok(out)
    }

    pub async fn overview(&self, p: &Principal) -> Result<Overview> {
        let spec = self.spec();
        let list = |status: &str, limit: i64| OperationFilter {
            status: Some(status.to_string()),
            limit: Some(limit),
            ..OperationFilter::default()
        };
        Ok(Overview {
            platform: spec.platform.name.clone(),
            spec_hash: spec.hash(),
            environments: self.environments(p).await?,
            awaiting_approval: self.store.list_operations(&list("awaiting_approval", 50)).await?,
            running: self.store.list_operations(&list("running", 50)).await?,
            recent: self
                .store
                .list_operations(&OperationFilter { limit: Some(15), ..OperationFilter::default() })
                .await?,
            open_incidents: self.store.list_incidents(true, 50).await?,
            backups: self.backup_statuses(p).await?,
        })
    }

    // -----------------------------------------------------------------------
    // Audit
    // -----------------------------------------------------------------------

    pub async fn audit_events(&self, p: &Principal, after: Option<i64>, limit: i64) -> Result<Vec<AuditEvent>> {
        let spec = self.spec();
        self.authorize(&spec, p, Permission::Audit, None, None)?;
        match after {
            Some(a) => self.store.audit_after(a, limit.clamp(1, 5_000)).await,
            None => self.store.audit_latest(limit.clamp(1, 1_000)).await,
        }
    }

    /// Vérification intégrale de la chaîne d'audit, par lots.
    pub async fn verify_audit(&self, p: &Principal) -> Result<ChainReport> {
        let spec = self.spec();
        self.authorize(&spec, p, Permission::Audit, None, None)?;
        self.verify_audit_unchecked().await
    }

    pub(crate) async fn verify_audit_unchecked(&self) -> Result<ChainReport> {
        let mut prev = GENESIS_HASH.to_string();
        let mut next = 1;
        let mut total = 0;
        let mut head = None;
        loop {
            let batch = self.store.audit_after(next - 1, 1_000).await?;
            if batch.is_empty() {
                break;
            }
            let report = verify_chain(&batch, &prev, next);
            if !report.valid {
                return Ok(ChainReport { events_checked: total + report.events_checked, ..report });
            }
            total += report.events_checked;
            if let Some(last) = batch.last() {
                prev.clone_from(&last.hash);
                next = last.seq + 1;
                head = Some((last.seq, last.hash.clone()));
            }
        }
        Ok(ChainReport {
            valid: true,
            events_checked: total,
            head_seq: head.as_ref().map(|h| h.0),
            head_hash: head.map(|h| h.1),
            broken_at: None,
            reason: None,
        })
    }

    // -----------------------------------------------------------------------
    // Conformité
    // -----------------------------------------------------------------------

    pub async fn compliance_report(&self, p: &Principal, days: i64) -> Result<ComplianceReport> {
        let spec = self.spec();
        self.authorize(&spec, p, Permission::Audit, None, None)?;
        let days = days.clamp(1, 366);
        let now = self.now();
        let since = now - chrono::TimeDelta::days(days);
        let ops = self.store.operations_since(&since).await?;
        let incidents = self.store.incidents_since(&since).await?;
        let backups = self.backup_statuses(p).await?;
        let audit_chain = self.verify_audit_unchecked().await?;
        let tokens = self.store.list_tokens().await?;

        let is_prod =
            |o: &Operation| spec.environments.get(o.environment()).is_some_and(|e| e.tier == Tier::Production);
        let mutating = |o: &Operation| {
            !matches!(o.kind(), OperationKind::InfraPlan | OperationKind::Backup | OperationKind::RestoreDrill)
        };
        let mut by_kind = BTreeMap::new();
        let mut by_status = BTreeMap::new();
        for o in &ops {
            *by_kind.entry(o.kind().as_str().to_string()).or_insert(0) += 1;
            *by_status.entry(o.status.as_str().to_string()).or_insert(0) += 1;
        }
        let prod_changes: Vec<&Operation> =
            ops.iter().filter(|o| is_prod(o) && mutating(o) && o.status != OpStatus::Rejected).collect();
        let prod_approved = prod_changes
            .iter()
            .filter(|o| {
                o.required_approvals == 0
                    || o.approvals.iter().filter(|a| a.decision == ApprovalDecision::Approved).count() as u32
                        >= o.required_approvals
            })
            .count();
        let emergency: Vec<String> = ops
            .iter()
            .filter(|o| o.request.emergency)
            .map(|o| {
                format!(
                    "{} — {} {} ({}) : {}",
                    o.id,
                    o.kind().as_str(),
                    o.request.action.subject(),
                    o.status.as_str(),
                    o.request.reason
                )
            })
            .collect();
        let gate_rejections = ops
            .iter()
            .filter(|o| o.status == OpStatus::Rejected && o.gates.iter().any(|g| g.status == GateStatus::Failed))
            .count();

        let mut by_sev = BTreeMap::new();
        let mut mtta = Vec::new();
        let mut mttr = Vec::new();
        let mut pending_pm = Vec::new();
        for i in &incidents {
            *by_sev.entry(i.severity.as_str().to_string()).or_insert(0) += 1;
            if let Some(d) = i.time_to_acknowledge() {
                mtta.push(d.num_seconds() as f64 / 60.0);
            }
            if let Some(d) = i.time_to_resolve() {
                mttr.push(d.num_seconds() as f64 / 60.0);
            }
            if i.severity.at_least(Severity::Sev2)
                && matches!(i.status, IncidentStatus::Resolved)
                && i.root_cause.is_none()
                && i.postmortem_url.is_none()
            {
                pending_pm.push(format!("{} — {}", i.id, i.title));
            }
        }

        let access_review: Vec<serde_json::Value> = spec
            .teams
            .iter()
            .map(|(name, t)| {
                serde_json::json!({
                    "team": name,
                    "idp_groups": t.idp_groups,
                    "members": t.members,
                    "bindings": spec.access.iter().filter(|b| &b.team == name).collect::<Vec<_>>(),
                })
            })
            .collect();

        let active_tokens: Vec<&ApiToken> = tokens.iter().filter(|t| !t.revoked).collect();
        let tokens_without_expiry = active_tokens.iter().filter(|t| t.expires_at.is_none()).count();
        let backups_ok = backups.iter().filter(|b| b.compliant()).count();
        let status = |ok: bool| if ok { "conforme" } else { "à traiter" }.to_string();

        let controls = vec![
            ControlCheck {
                reference: "ISO 27001 A.8.32 / SOC 2 CC8.1".into(),
                title: "Gestion des changements".into(),
                status: status(prod_approved == prod_changes.len()),
                evidence: format!(
                    "{}/{} changements de production conformes à la politique d'approbation ; quatre yeux imposés par l'outil ; {} contournement(s) d'urgence tracé(s)",
                    prod_approved,
                    prod_changes.len(),
                    emergency.len()
                ),
            },
            ControlCheck {
                reference: "ISO 27001 A.8.15 / SOC 2 CC7.2".into(),
                title: "Journalisation et intégrité des traces".into(),
                status: status(audit_chain.valid),
                evidence: format!(
                    "chaîne d'audit {} ({} événements vérifiés)",
                    if audit_chain.valid { "intègre" } else { "ROMPUE" },
                    audit_chain.events_checked
                ),
            },
            ControlCheck {
                reference: "ISO 27001 A.8.13 / SOC 2 A1.2".into(),
                title: "Sauvegarde et restauration".into(),
                status: status(backups_ok == backups.len()),
                evidence: format!(
                    "{backups_ok}/{} politiques conformes (RPO tenu, exercices à jour et dans le RTO)",
                    backups.len()
                ),
            },
            ControlCheck {
                reference: "ISO 27001 A.5.24–A.5.27 / SOC 2 CC7.4".into(),
                title: "Gestion des incidents".into(),
                status: status(pending_pm.is_empty()),
                evidence: format!(
                    "{} incident(s) sur la période ; {} postmortem(s) Sev1/Sev2 en attente",
                    incidents.len(),
                    pending_pm.len()
                ),
            },
            ControlCheck {
                reference: "ISO 27001 A.5.15 / A.5.18 / SOC 2 CC6.1–CC6.3".into(),
                title: "Contrôle et revue des accès".into(),
                status: status(tokens_without_expiry == 0),
                evidence: format!(
                    "{} équipe(s), {} liaison(s) de rôles versionnées ; {} jeton(s) actif(s) dont {} sans expiration",
                    spec.teams.len(),
                    spec.access.len(),
                    active_tokens.len(),
                    tokens_without_expiry
                ),
            },
            ControlCheck {
                reference: "ISO 27001 A.8.9 / A.8.8 / SOC 2 CC7.1".into(),
                title: "Configuration et vulnérabilités".into(),
                status: "conforme".into(),
                evidence: format!(
                    "configuration déclarative versionnée (empreinte {}), portes de vulnérabilités et de signature appliquées aux déploiements ; {gate_rejections} refus par les portes",
                    &spec.hash()[..12]
                ),
            },
        ];

        Ok(ComplianceReport {
            platform: spec.platform.name.clone(),
            generated_at: now,
            period_days: days,
            spec_hash: spec.hash(),
            changes_total: ops.len(),
            changes_by_kind: by_kind,
            changes_by_status: by_status,
            production_changes: prod_changes.len(),
            production_changes_approved: prod_approved,
            emergency_changes: emergency,
            gate_rejections,
            incidents_by_severity: by_sev,
            mean_time_to_acknowledge_min: average(&mtta),
            mean_time_to_resolve_min: average(&mttr),
            postmortems_pending: pending_pm,
            backups,
            audit_chain,
            access_review,
            tokens,
            controls,
        })
    }
}

impl ComplianceReport {
    /// Rendu Markdown, joignable tel quel à un dossier de preuves.
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        let _ = writeln!(md, "# Rapport de conformité — {}\n", self.platform);
        let _ = writeln!(
            md,
            "Généré le {} sur {} jours · spécification `{}`\n",
            format_ts(&self.generated_at),
            self.period_days,
            &self.spec_hash[..12]
        );
        let _ = writeln!(md, "## Contrôles\n\n| Référence | Contrôle | Statut | Preuve |\n|---|---|---|---|");
        for c in &self.controls {
            let _ = writeln!(md, "| {} | {} | **{}** | {} |", c.reference, c.title, c.status, c.evidence);
        }
        let _ = writeln!(md, "\n## Changements\n\n- Total : {}", self.changes_total);
        for (k, n) in &self.changes_by_kind {
            let _ = writeln!(md, "- {k} : {n}");
        }
        let _ = writeln!(
            md,
            "- Production : {} (conformes à la politique d'approbation : {})",
            self.production_changes, self.production_changes_approved
        );
        let _ = writeln!(md, "- Refus par les portes de contrôle : {}", self.gate_rejections);
        let _ = writeln!(md, "\n### Changements d'urgence\n");
        if self.emergency_changes.is_empty() {
            let _ = writeln!(md, "Aucun.");
        }
        for e in &self.emergency_changes {
            let _ = writeln!(md, "- {e}");
        }
        let _ = writeln!(md, "\n## Incidents\n");
        for (s, n) in &self.incidents_by_severity {
            let _ = writeln!(md, "- {s} : {n}");
        }
        let fmt = |v: Option<f64>| v.map_or("—".to_string(), |x| format!("{x:.1} min"));
        let _ = writeln!(md, "- Délai moyen d'acquittement : {}", fmt(self.mean_time_to_acknowledge_min));
        let _ = writeln!(md, "- Délai moyen de résolution : {}", fmt(self.mean_time_to_resolve_min));
        for pm in &self.postmortems_pending {
            let _ = writeln!(md, "- Postmortem en attente : {pm}");
        }
        let _ = writeln!(
            md,
            "\n## Sauvegardes\n\n| Politique | Env. | RPO | Dernière réussite | Exercice | Statut |\n|---|---|---|---|---|---|"
        );
        for b in &self.backups {
            let drill = b.drill.as_ref().map_or("—".to_string(), |d| match (d.last_success, d.last_duration_secs) {
                (Some(t), Some(s)) => format!("{} ({} s)", format_ts(&t), s),
                _ => "jamais".into(),
            });
            let _ = writeln!(
                md,
                "| {} | {} | {} s | {} | {} | {} |",
                b.policy,
                b.environment,
                b.rpo_secs,
                b.last_success.map_or("jamais".into(), |t| format_ts(&t)),
                drill,
                if b.compliant() { "conforme" } else { "**à traiter**" }
            );
        }
        let _ = writeln!(
            md,
            "\n## Journal d'audit\n\nChaîne {} — {} événements vérifiés{}.",
            if self.audit_chain.valid { "intègre" } else { "**ROMPUE**" },
            self.audit_chain.events_checked,
            self.audit_chain.head_hash.as_ref().map(|h| format!(", tête `{}`", &h[..12])).unwrap_or_default()
        );
        let _ = writeln!(md, "\n## Revue des accès\n");
        for a in &self.access_review {
            let _ = writeln!(md, "- `{}` : {}", a["team"].as_str().unwrap_or("?"), a["bindings"]);
        }
        md
    }
}
