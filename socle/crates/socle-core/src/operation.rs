//! Opérations : l'unique point d'entrée de tout changement (déploiement,
//! retour arrière, mise à l'échelle, infrastructure, sauvegarde, restauration,
//! correctifs, runbooks). Toutes suivent le même cycle :
//! portes → (approbation) → exécution → vérification → (retour arrière).

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::rbac::Permission;
use crate::{CoreError, Result};

// ---------------------------------------------------------------------------
// Requête
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    /// Déploie une release (identifiant, digest ou version).
    Deploy {
        service: String,
        release: String,
    },
    /// Revient à la release précédente (ou à `to_release`).
    Rollback {
        service: String,
        #[serde(default)]
        to_release: Option<String>,
    },
    /// Ajuste les bornes d'autoscaling (Kubernetes).
    Scale {
        service: String,
        min: u32,
        max: u32,
    },
    /// Calcule un plan d'infrastructure (lecture seule).
    InfraPlan {
        stack: String,
    },
    /// Applique exactement le plan calculé par `plan_operation`.
    InfraApply {
        stack: String,
        plan_operation: String,
    },
    Backup {
        policy: String,
    },
    Restore {
        policy: String,
        #[serde(default)]
        snapshot: Option<String>,
        #[serde(default)]
        point_in_time: Option<DateTime<Utc>>,
        #[serde(default)]
        in_place: bool,
    },
    /// Exercice de restauration : mesure le RTO réel sans toucher la production.
    RestoreDrill {
        policy: String,
    },
    Patch {
        target: String,
    },
    Runbook {
        runbook: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    Deploy,
    Rollback,
    Scale,
    InfraPlan,
    InfraApply,
    Backup,
    Restore,
    RestoreDrill,
    Patch,
    Runbook,
}

impl OperationKind {
    pub const ALL: [OperationKind; 10] = [
        OperationKind::Deploy,
        OperationKind::Rollback,
        OperationKind::Scale,
        OperationKind::InfraPlan,
        OperationKind::InfraApply,
        OperationKind::Backup,
        OperationKind::Restore,
        OperationKind::RestoreDrill,
        OperationKind::Patch,
        OperationKind::Runbook,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            OperationKind::Deploy => "deploy",
            OperationKind::Rollback => "rollback",
            OperationKind::Scale => "scale",
            OperationKind::InfraPlan => "infra_plan",
            OperationKind::InfraApply => "infra_apply",
            OperationKind::Backup => "backup",
            OperationKind::Restore => "restore",
            OperationKind::RestoreDrill => "restore_drill",
            OperationKind::Patch => "patch",
            OperationKind::Runbook => "runbook",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }

    pub fn permission(self) -> Permission {
        match self {
            OperationKind::Deploy => Permission::Deploy,
            OperationKind::Rollback => Permission::Rollback,
            OperationKind::Scale => Permission::Scale,
            OperationKind::InfraPlan => Permission::InfraPlan,
            OperationKind::InfraApply => Permission::InfraApply,
            OperationKind::Backup | OperationKind::RestoreDrill => Permission::Backup,
            OperationKind::Restore => Permission::Restore,
            OperationKind::Patch => Permission::Patch,
            OperationKind::Runbook => Permission::Runbook,
        }
    }

    /// Libellé français pour l'affichage.
    pub fn label(self) -> &'static str {
        match self {
            OperationKind::Deploy => "Déploiement",
            OperationKind::Rollback => "Retour arrière",
            OperationKind::Scale => "Mise à l'échelle",
            OperationKind::InfraPlan => "Plan d'infrastructure",
            OperationKind::InfraApply => "Application d'infrastructure",
            OperationKind::Backup => "Sauvegarde",
            OperationKind::Restore => "Restauration",
            OperationKind::RestoreDrill => "Exercice de restauration",
            OperationKind::Patch => "Correctifs",
            OperationKind::Runbook => "Runbook",
        }
    }
}

impl Action {
    pub fn kind(&self) -> OperationKind {
        match self {
            Action::Deploy { .. } => OperationKind::Deploy,
            Action::Rollback { .. } => OperationKind::Rollback,
            Action::Scale { .. } => OperationKind::Scale,
            Action::InfraPlan { .. } => OperationKind::InfraPlan,
            Action::InfraApply { .. } => OperationKind::InfraApply,
            Action::Backup { .. } => OperationKind::Backup,
            Action::Restore { .. } => OperationKind::Restore,
            Action::RestoreDrill { .. } => OperationKind::RestoreDrill,
            Action::Patch { .. } => OperationKind::Patch,
            Action::Runbook { .. } => OperationKind::Runbook,
        }
    }

    /// Objet principal de l'opération (service, stack, politique, cible…).
    pub fn subject(&self) -> &str {
        match self {
            Action::Deploy { service, .. } | Action::Rollback { service, .. } | Action::Scale { service, .. } => {
                service
            }
            Action::InfraPlan { stack } | Action::InfraApply { stack, .. } => stack,
            Action::Backup { policy } | Action::Restore { policy, .. } | Action::RestoreDrill { policy } => policy,
            Action::Patch { target } => target,
            Action::Runbook { runbook } => runbook,
        }
    }

    pub fn service(&self) -> Option<&str> {
        match self {
            Action::Deploy { service, .. } | Action::Rollback { service, .. } | Action::Scale { service, .. } => {
                Some(service)
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationRequest {
    pub environment: String,
    pub action: Action,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub change_ticket: Option<String>,
    /// Procédure d'urgence (« break-glass ») : contourne les portes de
    /// processus si la politique l'autorise, jamais les portes de sécurité.
    #[serde(default)]
    pub emergency: bool,
}

impl OperationRequest {
    pub fn validate_basic(&self) -> Result<()> {
        if self.reason.len() > 2_000 {
            return Err(CoreError::invalid("motif trop long (2000 caractères max.)"));
        }
        if let Some(t) = &self.change_ticket
            && (t.is_empty() || t.len() > 128 || t.chars().any(char::is_control))
        {
            return Err(CoreError::invalid("référence de ticket de changement invalide"));
        }
        if self.emergency && self.reason.trim().len() < 10 {
            return Err(CoreError::invalid("une procédure d'urgence exige un motif explicite (10 caractères min.)"));
        }
        if let Action::Scale { min, max, .. } = &self.action
            && (*min == 0 || min > max)
        {
            return Err(CoreError::invalid("bornes de mise à l'échelle invalides (1 ≤ min ≤ max)"));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Statut et approbations
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpStatus {
    AwaitingApproval,
    Queued,
    Running,
    Succeeded,
    Failed,
    RolledBack,
    Rejected,
    Cancelled,
}

impl OpStatus {
    pub const ALL: [OpStatus; 8] = [
        OpStatus::AwaitingApproval,
        OpStatus::Queued,
        OpStatus::Running,
        OpStatus::Succeeded,
        OpStatus::Failed,
        OpStatus::RolledBack,
        OpStatus::Rejected,
        OpStatus::Cancelled,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            OpStatus::AwaitingApproval => "awaiting_approval",
            OpStatus::Queued => "queued",
            OpStatus::Running => "running",
            OpStatus::Succeeded => "succeeded",
            OpStatus::Failed => "failed",
            OpStatus::RolledBack => "rolled_back",
            OpStatus::Rejected => "rejected",
            OpStatus::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            OpStatus::Succeeded | OpStatus::Failed | OpStatus::RolledBack | OpStatus::Rejected | OpStatus::Cancelled
        )
    }

    pub fn can_transition_to(self, next: OpStatus) -> bool {
        use OpStatus as S;
        matches!(
            (self, next),
            (S::AwaitingApproval, S::Queued | S::Rejected | S::Cancelled)
                | (S::Queued, S::Running | S::Cancelled)
                | (S::Running, S::Queued | S::Succeeded | S::Failed | S::RolledBack)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Approved,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Approval {
    pub approver: String,
    pub decision: ApprovalDecision,
    #[serde(default)]
    pub comment: String,
    pub at: DateTime<Utc>,
    /// Empreinte du plan approuvé : une approbation ne vaut que pour ce plan.
    pub plan_hash: String,
}

// ---------------------------------------------------------------------------
// Portes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateStatus {
    Passed,
    Warning,
    Failed,
    /// Échec contourné par la procédure d'urgence (tracé et revu a posteriori).
    Overridden,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateResult {
    pub gate: String,
    pub status: GateStatus,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Plan d'exécution
// ---------------------------------------------------------------------------

/// Valeur d'environnement d'une commande : littérale ou référence de secret
/// résolue au dernier moment par l'exécuteur (jamais persistée en clair).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EnvValue {
    Literal(String),
    Secret { secret: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecStep {
    pub program: String,
    pub args: Vec<String>,
    /// Répertoire de travail, relatif à la racine du dépôt.
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, EnvValue>,
    pub timeout_secs: u64,
    /// Codes de sortie considérés comme un succès (défaut : `[0]`).
    #[serde(default)]
    pub success_codes: Vec<i32>,
}

/// Modification de fichier dans le dépôt GitOps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum FileEdit {
    /// Fixe l'image d'un `kustomization.yaml` sur un digest immuable.
    SetImage { file: String, image: String, new_name: String, digest: String },
    /// Écrit un fichier entièrement géré par socle.
    Write { file: String, content: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StepAction {
    Exec(ExecStep),
    /// Modifie le dépôt GitOps puis commit/push (idempotent, avec reprise
    /// sur conflit de push).
    GitopsUpdate {
        repo: String,
        branch: String,
        edits: Vec<FileEdit>,
        message: String,
        push: bool,
    },
    /// Attend qu'une application Argo CD soit synchronisée et saine, et
    /// qu'elle exécute l'image attendue.
    WaitArgo {
        context: String,
        namespace: String,
        application: String,
        #[serde(default)]
        image_ref: Option<String>,
        timeout_secs: u64,
    },
    HttpCheck {
        url: String,
        expect_status: u16,
        retries: u32,
        interval_secs: u64,
        timeout_secs: u64,
    },
    /// Vérifie un indicateur Prometheus (taux d'erreur, latence…).
    PromCheck {
        prometheus_url: String,
        query: String,
        max: f64,
        samples: u32,
        interval_secs: u64,
    },
    /// Calcule l'empreinte d'un plan Terraform/OpenTofu ; si `expected` est
    /// fourni, échoue quand le plan diffère de celui qui a été approuvé.
    TfPlanGuard {
        program: String,
        chdir: String,
        planfile: String,
        #[serde(default)]
        expected: Option<String>,
    },
    /// Crée un silence Alertmanager pendant une intervention.
    Silence {
        alertmanager_url: String,
        matchers: BTreeMap<String, String>,
        duration_secs: u64,
        comment: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub name: String,
    pub action: StepAction,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    pub summary: String,
    pub steps: Vec<Step>,
    /// Vérifications post-changement (santé, SLO).
    #[serde(default)]
    pub verify: Vec<Step>,
    /// Étapes de compensation exécutées si l'exécution ou la vérification échoue.
    #[serde(default)]
    pub rollback: Vec<Step>,
    /// Verrous à détenir pendant l'exécution.
    #[serde(default)]
    pub locks: Vec<String>,
    /// Fichiers temporaires à supprimer quoi qu'il arrive (plans Terraform…).
    #[serde(default)]
    pub cleanup: Vec<String>,
    /// Sorties connues dès la planification (nom d'instantané…), recopiées
    /// dans le résultat de l'opération en cas de succès.
    #[serde(default)]
    pub outputs: BTreeMap<String, String>,
}

impl Plan {
    pub fn hash(&self) -> String {
        crate::sha256_json(self)
    }

    /// Description lisible de toutes les étapes (CLI, console, approbations).
    pub fn describe(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for (title, steps) in
            [("exécution", &self.steps), ("vérification", &self.verify), ("retour arrière", &self.rollback)]
        {
            for (i, step) in steps.iter().enumerate() {
                lines.push(format!("[{title} {}] {} : {}", i + 1, step.name, step.action.describe()));
            }
        }
        lines
    }
}

/// Mise entre guillemets pour l'affichage uniquement (les commandes ne
/// passent jamais par un shell).
fn display_arg(arg: &str) -> String {
    if !arg.is_empty() && arg.chars().all(|c| c.is_ascii_alphanumeric() || "-_./=:@%+,".contains(c)) {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', "'\\''"))
    }
}

impl StepAction {
    pub fn describe(&self) -> String {
        match self {
            StepAction::Exec(e) => {
                let mut s = String::new();
                for (k, v) in &e.env {
                    match v {
                        EnvValue::Literal(val) => s.push_str(&format!("{k}={} ", display_arg(val))),
                        EnvValue::Secret { secret } => s.push_str(&format!("{k}=<secret:{secret}> ")),
                    }
                }
                s.push_str(&display_arg(&e.program));
                for a in &e.args {
                    s.push(' ');
                    s.push_str(&display_arg(a));
                }
                if let Some(cwd) = &e.cwd {
                    s.push_str(&format!("  (dans {cwd})"));
                }
                s
            }
            StepAction::GitopsUpdate { edits, branch, push, .. } => {
                let what: Vec<String> = edits
                    .iter()
                    .map(|e| match e {
                        FileEdit::SetImage { file, image, new_name, digest } => {
                            format!("{file} : {image} → {new_name}@{digest}")
                        }
                        FileEdit::Write { file, .. } => format!("{file} (réécrit)"),
                    })
                    .collect();
                format!(
                    "{} ; commit{}",
                    what.join(", "),
                    if *push { format!(" + push sur {branch}") } else { String::new() }
                )
            }
            StepAction::WaitArgo { application, image_ref, timeout_secs, context, .. } => format!(
                "attendre Argo CD « {application} » ({context}) : Synced + Healthy{} — délai {timeout_secs}s",
                image_ref.as_ref().map(|i| format!(", image {i}")).unwrap_or_default()
            ),
            StepAction::HttpCheck { url, expect_status, retries, interval_secs, .. } => {
                format!("GET {url} → {expect_status} ({retries} essais, toutes les {interval_secs}s)")
            }
            StepAction::PromCheck { query, max, samples, interval_secs, .. } => {
                format!("PromQL `{query}` ≤ {max} ({samples} mesures, toutes les {interval_secs}s)")
            }
            StepAction::TfPlanGuard { chdir, expected, .. } => match expected {
                Some(h) => format!(
                    "vérifier que le plan de {chdir} est identique au plan approuvé ({})",
                    &h[..h.len().min(12)]
                ),
                None => format!("calculer l'empreinte du plan de {chdir}"),
            },
            StepAction::Silence { matchers, duration_secs, .. } => {
                let m: Vec<String> = matchers.iter().map(|(k, v)| format!("{k}={v}")).collect();
                format!("silence Alertmanager {{{}}} pendant {}s", m.join(","), duration_secs)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Opération
// ---------------------------------------------------------------------------

/// Contexte résolu au moment de la planification.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolved {
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub release_id: Option<String>,
    #[serde(default)]
    pub image_ref: Option<String>,
    #[serde(default)]
    pub previous_release_id: Option<String>,
    #[serde(default)]
    pub previous_image_ref: Option<String>,
    #[serde(default)]
    pub snapshot: Option<String>,
    /// Empreinte de la spécification qui a servi à planifier.
    pub spec_hash: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Outcome {
    pub message: String,
    #[serde(default)]
    pub rolled_back: bool,
    #[serde(default)]
    pub incident_id: Option<String>,
    #[serde(default)]
    pub data: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Operation {
    pub id: String,
    pub request: OperationRequest,
    pub status: OpStatus,
    pub requested_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub finished_at: Option<DateTime<Utc>>,
    pub plan: Plan,
    pub plan_hash: String,
    pub gates: Vec<GateResult>,
    pub required_approvals: u32,
    #[serde(default)]
    pub approvals: Vec<Approval>,
    pub resolved: Resolved,
    #[serde(default)]
    pub outcome: Option<Outcome>,
}

impl Operation {
    pub fn kind(&self) -> OperationKind {
        self.request.action.kind()
    }

    pub fn environment(&self) -> &str {
        &self.request.environment
    }

    /// Nombre d'approbations valides (sur le plan courant).
    pub fn approvals_granted(&self) -> u32 {
        let n = self
            .approvals
            .iter()
            .filter(|a| a.decision == ApprovalDecision::Approved && a.plan_hash == self.plan_hash)
            .count();
        u32::try_from(n).unwrap_or(u32::MAX)
    }

    pub fn transition(&mut self, next: OpStatus, now: DateTime<Utc>) -> Result<()> {
        if !self.status.can_transition_to(next) {
            return Err(CoreError::InvalidTransition(format!(
                "opération {} : {} → {}",
                self.id,
                self.status.as_str(),
                next.as_str()
            )));
        }
        self.status = next;
        self.updated_at = now;
        match next {
            OpStatus::Running => self.started_at = Some(now),
            s if s.is_terminal() => self.finished_at = Some(now),
            _ => {}
        }
        Ok(())
    }

    /// Enregistre une décision d'approbation (principe des quatre yeux :
    /// le demandeur ne peut pas approuver sa propre opération).
    /// Renvoie `true` si l'opération devient exécutable.
    pub fn decide(
        &mut self,
        approver: &str,
        decision: ApprovalDecision,
        comment: &str,
        now: DateTime<Utc>,
    ) -> Result<bool> {
        if self.status != OpStatus::AwaitingApproval {
            return Err(CoreError::InvalidTransition(format!(
                "l'opération {} n'attend pas d'approbation (statut : {})",
                self.id,
                self.status.as_str()
            )));
        }
        if approver == self.requested_by {
            return Err(CoreError::forbidden(
                "principe des quatre yeux : le demandeur ne peut pas approuver sa propre opération",
            ));
        }
        if self.approvals.iter().any(|a| a.approver == approver && a.plan_hash == self.plan_hash) {
            return Err(CoreError::Conflict(format!("{approver} s'est déjà prononcé sur cette opération")));
        }
        if decision == ApprovalDecision::Rejected && comment.trim().is_empty() {
            return Err(CoreError::invalid("un refus doit être motivé"));
        }
        self.approvals.push(Approval {
            approver: approver.to_string(),
            decision,
            comment: comment.trim().to_string(),
            at: now,
            plan_hash: self.plan_hash.clone(),
        });
        match decision {
            ApprovalDecision::Rejected => {
                self.transition(OpStatus::Rejected, now)?;
                self.outcome = Some(Outcome {
                    message: format!("Refusée par {approver} : {}", comment.trim()),
                    ..Outcome::default()
                });
                Ok(false)
            }
            ApprovalDecision::Approved => {
                if self.approvals_granted() >= self.required_approvals {
                    self.transition(OpStatus::Queued, now)?;
                    Ok(true)
                } else {
                    self.updated_at = now;
                    Ok(false)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(required: u32) -> Operation {
        let plan = Plan { summary: "test".into(), ..Plan::default() };
        Operation {
            id: "op-1".into(),
            request: OperationRequest {
                environment: "prod".into(),
                action: Action::Deploy { service: "erp-app".into(), release: "1.0.0".into() },
                reason: String::new(),
                change_ticket: None,
                emergency: false,
            },
            status: OpStatus::AwaitingApproval,
            requested_by: "alice".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            started_at: None,
            finished_at: None,
            plan_hash: plan.hash(),
            plan,
            gates: vec![],
            required_approvals: required,
            approvals: vec![],
            resolved: Resolved::default(),
            outcome: None,
        }
    }

    #[test]
    fn four_eyes_and_quorum() {
        let mut o = op(2);
        assert!(o.decide("alice", ApprovalDecision::Approved, "", Utc::now()).is_err());
        assert!(!o.decide("bob", ApprovalDecision::Approved, "ok", Utc::now()).expect("bob"));
        assert!(o.decide("bob", ApprovalDecision::Approved, "encore", Utc::now()).is_err());
        assert!(o.decide("carol", ApprovalDecision::Approved, "ok", Utc::now()).expect("carol"));
        assert_eq!(o.status, OpStatus::Queued);
    }

    #[test]
    fn rejection_requires_reason() {
        let mut o = op(1);
        assert!(o.decide("bob", ApprovalDecision::Rejected, " ", Utc::now()).is_err());
        o.decide("bob", ApprovalDecision::Rejected, "risque trop élevé", Utc::now()).expect("refus");
        assert_eq!(o.status, OpStatus::Rejected);
        assert!(o.finished_at.is_some());
    }

    #[test]
    fn status_machine() {
        assert!(OpStatus::Queued.can_transition_to(OpStatus::Running));
        assert!(!OpStatus::Succeeded.can_transition_to(OpStatus::Running));
        assert!(!OpStatus::AwaitingApproval.can_transition_to(OpStatus::Running));
        for s in OpStatus::ALL {
            assert_eq!(OpStatus::parse(s.as_str()), Some(s));
        }
        for k in OperationKind::ALL {
            assert_eq!(OperationKind::parse(k.as_str()), Some(k));
        }
    }

    #[test]
    fn action_serde_is_tagged() {
        let a: Action =
            serde_json::from_value(serde_json::json!({"kind": "scale", "service": "erp-app", "min": 2, "max": 6}))
                .expect("scale");
        assert_eq!(a.kind(), OperationKind::Scale);
        assert!(
            serde_json::from_value::<Action>(
                serde_json::json!({"kind": "deploy", "service": "x", "release": "1", "bogus": 1})
            )
            .is_err()
        );
    }
}
