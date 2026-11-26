//! Modèle déclaratif de la plateforme (`socle.yaml`) : fournisseurs (multi-
//! cloud et on-premise), cibles d'exécution (Kubernetes, VMs RHEL), stacks
//! d'infrastructure, environnements, politiques, services, sauvegardes,
//! runbooks, équipes, droits et notifications.
//!
//! La spécification est la source de vérité unique : elle est versionnée dans
//! Git, revue en merge request et validée intégralement (références croisées
//! comprises) avant tout chargement.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::duration::HumanDuration;
use crate::incident::Severity;
use crate::names::{self, is_safe_relative_path, is_valid_name};
use crate::rbac::Role;
use crate::windows::{ChangeWindow, Freeze};
use crate::{CoreError, Result};

fn default_true() -> bool {
    true
}
fn default_dot() -> String {
    ".".into()
}
fn default_branch() -> String {
    "main".into()
}
fn default_ansible_dir() -> String {
    "infrastructure/ansible".into()
}
fn default_argocd_namespace() -> String {
    "argocd".into()
}
fn default_batch() -> String {
    "25%".into()
}
fn default_health_path() -> String {
    "/health".into()
}
fn default_status() -> u16 {
    200
}
fn default_retries() -> u32 {
    10
}
fn default_interval() -> HumanDuration {
    HumanDuration::from_secs(10)
}
fn default_timeout() -> HumanDuration {
    HumanDuration::from_secs(5)
}
fn default_verify_window() -> HumanDuration {
    HumanDuration::from_mins(5)
}
fn default_patch_playbook() -> String {
    "playbooks/patch_rhel.yml".into()
}
fn all_scope() -> Vec<String> {
    vec!["*".into()]
}

// ---------------------------------------------------------------------------
// Racine
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub version: u32,
    pub platform: PlatformMeta,
    #[serde(default)]
    pub providers: BTreeMap<String, Provider>,
    #[serde(default)]
    pub targets: BTreeMap<String, Target>,
    #[serde(default)]
    pub stacks: BTreeMap<String, Stack>,
    pub environments: BTreeMap<String, Environment>,
    #[serde(default)]
    pub policies: BTreeMap<String, Policy>,
    #[serde(default)]
    pub services: BTreeMap<String, Service>,
    #[serde(default)]
    pub backups: BTreeMap<String, BackupPolicy>,
    #[serde(default)]
    pub runbooks: BTreeMap<String, Runbook>,
    #[serde(default)]
    pub teams: BTreeMap<String, Team>,
    #[serde(default)]
    pub access: Vec<AccessBinding>,
    #[serde(default)]
    pub notifications: Notifications,
    #[serde(default)]
    pub observability: Observability,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlatformMeta {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Équipe propriétaire de la plateforme.
    pub owner_team: String,
    /// Dépôt GitOps, relatif à la racine du dépôt de travail.
    #[serde(default = "default_dot")]
    pub gitops_repo: String,
    #[serde(default = "default_branch")]
    pub gitops_branch: String,
    /// Pousser les commits GitOps (désactivable en lab ou en test).
    #[serde(default = "default_true")]
    pub gitops_push: bool,
    /// Dossier Ansible (contient `ansible.cfg`), relatif à la racine.
    #[serde(default = "default_ansible_dir")]
    pub ansible_dir: String,
}

// ---------------------------------------------------------------------------
// Fournisseurs, cibles, stacks
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Aws,
    Azure,
    Gcp,
    Ovh,
    Scaleway,
    Onprem,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provider {
    pub kind: ProviderKind,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Target {
    Kubernetes(KubernetesTarget),
    Vm(VmTarget),
}

impl Target {
    pub fn provider(&self) -> &str {
        match self {
            Target::Kubernetes(t) => &t.provider,
            Target::Vm(t) => &t.provider,
        }
    }
    pub fn kind_name(&self) -> &'static str {
        match self {
            Target::Kubernetes(_) => "kubernetes",
            Target::Vm(_) => "vm",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KubernetesTarget {
    pub provider: String,
    /// Contexte kubeconfig du cluster (EKS, AKS, GKE, OVH MKS, Kapsule, on-prem…).
    pub context: String,
    #[serde(default = "default_argocd_namespace")]
    pub argocd_namespace: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VmTarget {
    pub provider: String,
    /// Inventaire Ansible, relatif à `platform.ansible_dir`.
    pub inventory: String,
    /// Groupe d'hôtes ciblé.
    pub group: String,
    #[serde(default)]
    pub patching: Option<Patching>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Patching {
    #[serde(default = "default_patch_playbook")]
    pub playbook: String,
    /// Taille de lot (`1` ou `25%`).
    #[serde(default = "default_batch")]
    pub batch: String,
    #[serde(default)]
    pub security_only: bool,
    #[serde(default = "default_true")]
    pub reboot: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IacTool {
    #[default]
    Terraform,
    Tofu,
}

impl IacTool {
    pub fn program(self) -> &'static str {
        match self {
            IacTool::Terraform => "terraform",
            IacTool::Tofu => "tofu",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stack {
    pub provider: String,
    /// Dossier Terraform/OpenTofu, relatif à la racine du dépôt.
    pub path: String,
    #[serde(default)]
    pub tool: IacTool,
    /// Environnements où cette stack est déployée.
    pub environments: Vec<String>,
    /// Fichiers de variables (relatifs à `path`, gabarit `{env}`).
    #[serde(default)]
    pub var_files: Vec<String>,
    /// Configuration partielle du backend d'état (relative à `path`, gabarit `{env}`).
    #[serde(default)]
    pub backend_config: Option<String>,
    /// Cadence de détection de dérive (plan planifié).
    #[serde(default)]
    pub drift_check: Option<HumanDuration>,
}

// ---------------------------------------------------------------------------
// Environnements et politiques
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Development,
    Staging,
    Production,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Environment {
    pub tier: Tier,
    pub policy: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Environnement source pour la promotion (« build once, deploy many »).
    #[serde(default)]
    pub promote_from: Option<String>,
    #[serde(default)]
    pub change_windows: Vec<ChangeWindow>,
    #[serde(default)]
    pub freezes: Vec<Freeze>,
    /// Libellés ciblant les alertes de l'environnement (silences de maintenance).
    #[serde(default)]
    pub alert_labels: BTreeMap<String, String>,
}

/// Politique de changement (portes DevSecOps et processus).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    #[serde(default)]
    pub approvals: u32,
    #[serde(default)]
    pub require_change_ticket: bool,
    #[serde(default)]
    pub change_ticket_pattern: Option<String>,
    #[serde(default)]
    pub require_tests: bool,
    #[serde(default)]
    pub require_signature: bool,
    #[serde(default)]
    pub require_sbom: bool,
    #[serde(default)]
    pub max_critical: Option<u32>,
    #[serde(default)]
    pub max_high: Option<u32>,
    #[serde(default)]
    pub enforce_change_windows: bool,
    #[serde(default)]
    pub enforce_promotion: bool,
    /// Bloque les déploiements tant qu'un incident au moins aussi grave est ouvert.
    #[serde(default)]
    pub block_on_incident: Option<Severity>,
    #[serde(default = "default_true")]
    pub auto_rollback: bool,
    #[serde(default)]
    pub allow_emergency: bool,
}

// ---------------------------------------------------------------------------
// Services
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strategy {
    #[default]
    Rolling,
    Canary,
    BlueGreen,
}

impl Strategy {
    pub fn as_str(self) -> &'static str {
        match self {
            Strategy::Rolling => "rolling",
            Strategy::Canary => "canary",
            Strategy::BlueGreen => "blue_green",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Service {
    pub team: String,
    /// Dépôt d'image, sans tag ni digest.
    pub image: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub gitops: Option<GitOps>,
    #[serde(default)]
    pub ansible: Option<AnsibleDeploy>,
    #[serde(default)]
    pub health: Option<HealthCheck>,
    #[serde(default)]
    pub slo: Option<Slo>,
    pub environments: BTreeMap<String, ServiceEnv>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitOps {
    /// Dossier de l'overlay Kustomize (gabarit `{env}`).
    pub path: String,
    /// Application Argo CD (gabarit `{env}`).
    pub application: String,
    /// Nom de l'image dans `images:` (défaut : nom du service).
    #[serde(default)]
    pub image_name: Option<String>,
    /// Charge de travail ciblée par l'HPA (défaut : nom du service).
    #[serde(default)]
    pub workload: Option<String>,
    /// Argo Rollout (obligatoire pour canary et blue_green).
    #[serde(default)]
    pub rollout: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnsibleDeploy {
    /// Playbook, relatif à `platform.ansible_dir`.
    pub playbook: String,
    #[serde(default)]
    pub extra_vars: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceEnv {
    pub target: String,
    #[serde(default)]
    pub strategy: Strategy,
    /// URL publique ou interne, pour les vérifications de santé.
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub replicas: Option<Replicas>,
    /// Taille de lot pour les VMs.
    #[serde(default = "default_batch")]
    pub batch: String,
    /// Attendre la convergence et vérifier la santé après le changement.
    #[serde(default = "default_true")]
    pub wait: bool,
    /// Surcharge de l'overlay GitOps pour cet environnement (ex. variante canary).
    #[serde(default)]
    pub gitops_path: Option<String>,
    /// Surcharge de l'application Argo CD pour cet environnement.
    #[serde(default)]
    pub application: Option<String>,
}

impl ServiceEnv {
    /// Overlay Kustomize effectif (surcharge d'environnement, sinon gabarit du service).
    pub fn overlay(&self, g: &GitOps, env: &str, service: &str) -> String {
        names::expand(self.gitops_path.as_deref().unwrap_or(&g.path), env, service)
    }

    /// Application Argo CD effective.
    pub fn argo_application(&self, g: &GitOps, env: &str, service: &str) -> String {
        names::expand(self.application.as_deref().unwrap_or(&g.application), env, service)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Replicas {
    pub min: u32,
    pub max: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HealthCheck {
    #[serde(default = "default_health_path")]
    pub path: String,
    #[serde(default = "default_status")]
    pub expect_status: u16,
    #[serde(default = "default_retries")]
    pub retries: u32,
    #[serde(default = "default_interval")]
    pub interval: HumanDuration,
    #[serde(default = "default_timeout")]
    pub timeout: HumanDuration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Slo {
    /// Objectif de disponibilité en pourcentage (ex. 99.9).
    pub availability: f64,
    /// Requête PromQL du taux d'erreur (gabarits `{env}`, `{service}`).
    #[serde(default)]
    pub error_rate_query: Option<String>,
    /// Seuil maximal du taux d'erreur pendant la vérification.
    #[serde(default)]
    pub max_error_rate: Option<f64>,
    #[serde(default = "default_verify_window")]
    pub verification_window: HumanDuration,
}

// ---------------------------------------------------------------------------
// Sauvegardes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupPolicy {
    pub environment: String,
    #[serde(default)]
    pub description: Option<String>,
    pub source: BackupSource,
    /// Cadence des sauvegardes planifiées.
    pub every: HumanDuration,
    /// Perte de données maximale admissible.
    pub rpo: HumanDuration,
    /// Durée maximale de restauration.
    pub rto: HumanDuration,
    pub retention: Retention,
    #[serde(default)]
    pub drill: Option<Drill>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BackupSource {
    /// Fichiers sur VMs, via restic (dépôts S3, Azure Blob, GCS, Swift…).
    Files(FilesSource),
    /// PostgreSQL autogéré, via pgBackRest (PITR par archivage WAL).
    Postgres(PostgresSource),
    /// Namespaces Kubernetes et volumes, via Velero.
    Kubernetes(KubernetesSource),
    /// Commandes génériques (bases managées, snapshots fournisseur…).
    Command(CommandSource),
}

impl BackupSource {
    pub fn kind_name(&self) -> &'static str {
        match self {
            BackupSource::Files(_) => "files",
            BackupSource::Postgres(_) => "postgres",
            BackupSource::Kubernetes(_) => "kubernetes",
            BackupSource::Command(_) => "command",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilesSource {
    pub target: String,
    pub paths: Vec<String>,
    /// Dépôt restic principal (URL non secrète).
    pub repository: String,
    /// Copies secondaires (règle 3-2-1, idéalement chez un autre fournisseur).
    #[serde(default)]
    pub copies: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PostgresSource {
    pub target: String,
    pub stanza: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KubernetesSource {
    pub target: String,
    pub namespaces: Vec<String>,
    #[serde(default)]
    pub storage_location: Option<String>,
}

/// Commandes (listes d'arguments, sans shell). Gabarits disponibles :
/// `{policy}`, `{env}`, `{operation}`, `{snapshot}`, `{point_in_time}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandSource {
    pub backup: Vec<String>,
    #[serde(default)]
    pub restore: Option<Vec<String>>,
    #[serde(default)]
    pub drill: Option<Vec<String>>,
    #[serde(default)]
    pub timeout: Option<HumanDuration>,
}

/// Rétention GFS (grand-père/père/fils), sémantique identique à restic.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Retention {
    #[serde(default)]
    pub last: u32,
    #[serde(default)]
    pub hourly: u32,
    #[serde(default)]
    pub daily: u32,
    #[serde(default)]
    pub weekly: u32,
    #[serde(default)]
    pub monthly: u32,
    #[serde(default)]
    pub yearly: u32,
}

impl Retention {
    pub fn is_empty(&self) -> bool {
        self.last + self.hourly + self.daily + self.weekly + self.monthly + self.yearly == 0
    }
    /// Horizon de conservation approximatif (pour les outils à TTL, ex. Velero).
    pub fn horizon_hours(&self) -> u64 {
        let h = [
            u64::from(self.hourly),
            u64::from(self.daily) * 24,
            u64::from(self.weekly) * 24 * 7,
            u64::from(self.monthly) * 24 * 31,
            u64::from(self.yearly) * 24 * 366,
        ];
        h.into_iter().max().unwrap_or(0).max(24)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Drill {
    pub every: HumanDuration,
    /// Cible de restauration de l'exercice (défaut : la cible source).
    #[serde(default)]
    pub target: Option<String>,
}

// ---------------------------------------------------------------------------
// Runbooks
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Runbook {
    pub description: String,
    /// Environnements autorisés (vide : tous).
    #[serde(default)]
    pub environments: Vec<String>,
    pub steps: Vec<RunbookStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RunbookStep {
    Command {
        name: String,
        program: String,
        #[serde(default)]
        args: Vec<String>,
        /// Répertoire de travail, relatif à la racine du dépôt.
        #[serde(default)]
        cwd: Option<String>,
        #[serde(default)]
        timeout: Option<HumanDuration>,
    },
    Ansible {
        name: String,
        target: String,
        playbook: String,
        #[serde(default)]
        extra_vars: BTreeMap<String, serde_json::Value>,
    },
    HttpCheck {
        name: String,
        url: String,
        #[serde(default = "default_status")]
        expect_status: u16,
    },
}

// ---------------------------------------------------------------------------
// Équipes, droits, notifications, observabilité
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Team {
    #[serde(default)]
    pub description: Option<String>,
    /// Groupes du fournisseur d'identité (Entra ID, Keycloak, Okta, Google…).
    #[serde(default)]
    pub idp_groups: Vec<String>,
    /// Membres explicites (identifiant ou e-mail).
    #[serde(default)]
    pub members: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccessBinding {
    pub team: String,
    pub role: Role,
    #[serde(default = "all_scope")]
    pub environments: Vec<String>,
    #[serde(default = "all_scope")]
    pub services: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Notifications {
    #[serde(default)]
    pub channels: BTreeMap<String, Channel>,
    #[serde(default)]
    pub routes: Vec<NotificationRoute>,
}

/// Les URL de webhook et clés de routage sont des secrets : on ne déclare
/// que leur référence (`env:`, `file:`, `vault:`, `aws-sm:`, `azure-kv:`, `gcp-sm:`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Channel {
    Webhook { url: String },
    Slack { webhook: String },
    Teams { webhook: String },
    Pagerduty { routing_key: String },
}

impl Channel {
    pub fn secret_ref(&self) -> &str {
        match self {
            Channel::Webhook { url } => url,
            Channel::Slack { webhook } | Channel::Teams { webhook } => webhook,
            Channel::Pagerduty { routing_key } => routing_key,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationRoute {
    /// Motifs d'événements (`operation.failed`, `incident.*`, `*`).
    pub events: Vec<String>,
    pub channels: Vec<String>,
    /// Restreint aux environnements listés (vide : tous).
    #[serde(default)]
    pub environments: Vec<String>,
}

impl NotificationRoute {
    pub fn matches(&self, event: &str, environment: Option<&str>) -> bool {
        let event_ok = self.events.iter().any(|p| pattern_matches(p, event));
        let env_ok =
            self.environments.is_empty() || environment.is_some_and(|e| self.environments.iter().any(|x| x == e));
        event_ok && env_ok
    }
}

/// `*` seul, suffixe `.*`, ou égalité stricte.
pub fn pattern_matches(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    match pattern.strip_suffix(".*") {
        Some(prefix) => value.starts_with(prefix) && value[prefix.len()..].starts_with('.'),
        None => pattern == value,
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observability {
    #[serde(default)]
    pub prometheus_url: Option<String>,
    #[serde(default)]
    pub alertmanager_url: Option<String>,
    #[serde(default)]
    pub grafana_url: Option<String>,
}

// ---------------------------------------------------------------------------
// Chargement, accès et validation
// ---------------------------------------------------------------------------

/// Vue résolue d'un service dans un environnement.
pub struct ServiceInEnv<'a> {
    pub name: &'a str,
    pub service: &'a Service,
    pub env: &'a ServiceEnv,
    pub target_name: &'a str,
    pub target: &'a Target,
}

impl Spec {
    /// Analyse et valide une spécification YAML.
    pub fn from_yaml(input: &str) -> Result<Self> {
        let spec: Spec =
            serde_norway::from_str(input).map_err(|e| CoreError::InvalidSpec(vec![format!("YAML : {e}")]))?;
        spec.validate()?;
        Ok(spec)
    }

    /// Empreinte de la spécification (traçabilité : chaque opération
    /// enregistre la version de spécification qui l'a planifiée).
    pub fn hash(&self) -> String {
        crate::sha256_json(self)
    }

    pub fn environment(&self, name: &str) -> Result<&Environment> {
        self.environments.get(name).ok_or_else(|| CoreError::not_found(format!("environnement « {name} »")))
    }

    pub fn policy_for(&self, env: &str) -> Result<&Policy> {
        let e = self.environment(env)?;
        self.policies.get(&e.policy).ok_or_else(|| CoreError::not_found(format!("politique « {} »", e.policy)))
    }

    pub fn service(&self, name: &str) -> Result<&Service> {
        self.services.get(name).ok_or_else(|| CoreError::not_found(format!("service « {name} »")))
    }

    pub fn target(&self, name: &str) -> Result<&Target> {
        self.targets.get(name).ok_or_else(|| CoreError::not_found(format!("cible « {name} »")))
    }

    pub fn stack(&self, name: &str) -> Result<&Stack> {
        self.stacks.get(name).ok_or_else(|| CoreError::not_found(format!("stack « {name} »")))
    }

    pub fn backup(&self, name: &str) -> Result<&BackupPolicy> {
        self.backups.get(name).ok_or_else(|| CoreError::not_found(format!("politique de sauvegarde « {name} »")))
    }

    pub fn runbook(&self, name: &str) -> Result<&Runbook> {
        self.runbooks.get(name).ok_or_else(|| CoreError::not_found(format!("runbook « {name} »")))
    }

    pub fn service_in_env<'a>(&'a self, service: &'a str, env: &str) -> Result<ServiceInEnv<'a>> {
        let (name, svc) = self
            .services
            .get_key_value(service)
            .ok_or_else(|| CoreError::not_found(format!("service « {service} »")))?;
        let senv = svc.environments.get(env).ok_or_else(|| {
            CoreError::invalid(format!("le service {service} n'est pas déclaré dans l'environnement {env}"))
        })?;
        let (target_name, target) = self
            .targets
            .get_key_value(&senv.target)
            .ok_or_else(|| CoreError::not_found(format!("cible « {} »", senv.target)))?;
        Ok(ServiceInEnv { name, service: svc, env: senv, target_name, target })
    }

    /// Validation complète. Toutes les erreurs sont collectées pour être
    /// corrigées en une seule passe.
    pub fn validate(&self) -> Result<()> {
        let mut v = Validator::default();
        self.validate_into(&mut v);
        if v.errors.is_empty() { Ok(()) } else { Err(CoreError::InvalidSpec(v.errors)) }
    }

    fn validate_into(&self, v: &mut Validator) {
        if self.version != 1 {
            v.err(format!("version {} non prise en charge (attendu : 1)", self.version));
        }
        for (kind, keys) in [
            ("fournisseur", self.providers.keys().collect::<Vec<_>>()),
            ("cible", self.targets.keys().collect()),
            ("stack", self.stacks.keys().collect()),
            ("environnement", self.environments.keys().collect()),
            ("politique", self.policies.keys().collect()),
            ("service", self.services.keys().collect()),
            ("sauvegarde", self.backups.keys().collect()),
            ("runbook", self.runbooks.keys().collect()),
            ("équipe", self.teams.keys().collect()),
            ("canal", self.notifications.channels.keys().collect()),
        ] {
            for k in keys {
                if !is_valid_name(k) {
                    v.err(format!("{kind} « {k} » : nom invalide (minuscules, chiffres, tirets ; 63 car. max.)"));
                }
            }
        }
        self.validate_platform(v);
        self.validate_targets(v);
        self.validate_stacks(v);
        self.validate_environments(v);
        self.validate_services(v);
        self.validate_backups(v);
        self.validate_runbooks(v);
        self.validate_access(v);
        self.validate_notifications(v);
    }

    fn validate_platform(&self, v: &mut Validator) {
        let p = &self.platform;
        if p.name.trim().is_empty() {
            v.err("platform.name est obligatoire");
        }
        if !self.teams.contains_key(&p.owner_team) {
            v.err(format!("platform.owner_team : équipe inconnue « {} »", p.owner_team));
        }
        for (field, path) in [("gitops_repo", &p.gitops_repo), ("ansible_dir", &p.ansible_dir)] {
            if !is_safe_relative_path(path) {
                v.err(format!("platform.{field} : chemin non sûr « {path} »"));
            }
        }
        if !names::is_valid_version(&p.gitops_branch) {
            v.err(format!("platform.gitops_branch invalide « {} »", p.gitops_branch));
        }
        for (field, url) in [
            ("prometheus_url", &self.observability.prometheus_url),
            ("alertmanager_url", &self.observability.alertmanager_url),
            ("grafana_url", &self.observability.grafana_url),
        ] {
            if let Some(u) = url
                && !names::is_http_url(u)
            {
                v.err(format!("observability.{field} : URL invalide « {u} »"));
            }
        }
    }

    fn validate_targets(&self, v: &mut Validator) {
        for (name, t) in &self.targets {
            if !self.providers.contains_key(t.provider()) {
                v.err(format!("cible {name} : fournisseur inconnu « {} »", t.provider()));
            }
            match t {
                Target::Kubernetes(k) => {
                    if !names::is_valid_kube_context(&k.context) {
                        v.err(format!("cible {name} : contexte kubeconfig invalide « {} »", k.context));
                    }
                    if !is_valid_name(&k.argocd_namespace) {
                        v.err(format!("cible {name} : namespace Argo CD invalide"));
                    }
                }
                Target::Vm(m) => {
                    if !is_safe_relative_path(&m.inventory) {
                        v.err(format!("cible {name} : inventaire non sûr « {} »", m.inventory));
                    }
                    if !names::is_valid_ansible_group(&m.group) {
                        v.err(format!("cible {name} : groupe Ansible invalide « {} »", m.group));
                    }
                    if let Some(p) = &m.patching {
                        if !is_safe_relative_path(&p.playbook) {
                            v.err(format!("cible {name} : playbook de correctifs non sûr"));
                        }
                        if !names::is_valid_batch(&p.batch) {
                            v.err(format!("cible {name} : taille de lot invalide « {} »", p.batch));
                        }
                    }
                }
            }
        }
    }

    fn validate_stacks(&self, v: &mut Validator) {
        for (name, s) in &self.stacks {
            if !self.providers.contains_key(&s.provider) {
                v.err(format!("stack {name} : fournisseur inconnu « {} »", s.provider));
            }
            if !is_safe_relative_path(&s.path) {
                v.err(format!("stack {name} : chemin non sûr « {} »", s.path));
            }
            if s.environments.is_empty() {
                v.err(format!("stack {name} : au moins un environnement est requis"));
            }
            for e in &s.environments {
                if !self.environments.contains_key(e) {
                    v.err(format!("stack {name} : environnement inconnu « {e} »"));
                }
            }
            for f in s.var_files.iter().chain(s.backend_config.iter()) {
                if !is_safe_relative_path(f) {
                    v.err(format!("stack {name} : fichier non sûr « {f} »"));
                }
            }
            if s.drift_check.is_some_and(|d| d.as_secs() < 300) {
                v.err(format!("stack {name} : drift_check doit être d'au moins 5 minutes"));
            }
        }
    }

    fn validate_environments(&self, v: &mut Validator) {
        for (name, e) in &self.environments {
            if !self.policies.contains_key(&e.policy) {
                v.err(format!("environnement {name} : politique inconnue « {} »", e.policy));
            }
            if let Some(src) = &e.promote_from {
                if src == name {
                    v.err(format!("environnement {name} : promote_from ne peut pas pointer sur lui-même"));
                } else if !self.environments.contains_key(src) {
                    v.err(format!("environnement {name} : promote_from inconnu « {src} »"));
                }
            }
            for w in &e.change_windows {
                if let Err(msg) = w.validate() {
                    v.err(format!("environnement {name} : {msg}"));
                }
            }
            for f in &e.freezes {
                if let Err(msg) = f.validate() {
                    v.err(format!("environnement {name} : {msg}"));
                }
            }
            // Détection de cycle de promotion.
            let mut seen = BTreeSet::from([name.as_str()]);
            let mut cur = e.promote_from.as_deref();
            while let Some(c) = cur {
                if !seen.insert(c) {
                    v.err(format!("environnement {name} : cycle de promotion détecté"));
                    break;
                }
                cur = self.environments.get(c).and_then(|x| x.promote_from.as_deref());
            }
        }
        for (name, p) in &self.policies {
            if let Some(pattern) = &p.change_ticket_pattern
                && regex::Regex::new(pattern).is_err()
            {
                v.err(format!("politique {name} : change_ticket_pattern n'est pas une expression régulière valide"));
            }
            if p.approvals > 5 {
                v.err(format!("politique {name} : 5 approbations au maximum"));
            }
        }
    }

    fn validate_services(&self, v: &mut Validator) {
        for (name, s) in &self.services {
            if !self.teams.contains_key(&s.team) {
                v.err(format!("service {name} : équipe inconnue « {} »", s.team));
            }
            if !names::is_valid_image_repo(&s.image) {
                v.err(format!("service {name} : dépôt d'image invalide « {} » (sans tag ni digest)", s.image));
            }
            if s.environments.is_empty() {
                v.err(format!("service {name} : aucun environnement déclaré"));
            }
            if let Some(g) = &s.gitops {
                if !is_safe_relative_path(&g.path) {
                    v.err(format!("service {name} : gitops.path non sûr « {} »", g.path));
                }
                let app_probe = names::expand(&g.application, "env", name);
                if !is_valid_name(&app_probe) {
                    v.err(format!("service {name} : nom d'application Argo CD invalide « {} »", g.application));
                }
                for (field, val) in [("image_name", &g.image_name), ("workload", &g.workload), ("rollout", &g.rollout)]
                {
                    if let Some(x) = val
                        && !is_valid_name(x)
                    {
                        v.err(format!("service {name} : gitops.{field} invalide « {x} »"));
                    }
                }
            }
            if let Some(a) = &s.ansible {
                if !is_safe_relative_path(&a.playbook) {
                    v.err(format!("service {name} : playbook non sûr « {} »", a.playbook));
                }
                if a.extra_vars.keys().any(|k| k.starts_with("socle_")) {
                    v.err(format!("service {name} : le préfixe socle_ est réservé dans extra_vars"));
                }
            }
            if let Some(h) = &s.health {
                if !h.path.starts_with('/') {
                    v.err(format!("service {name} : health.path doit commencer par /"));
                }
                if !(100..=599).contains(&h.expect_status) {
                    v.err(format!("service {name} : health.expect_status invalide"));
                }
            }
            if let Some(slo) = &s.slo {
                if !(slo.availability > 0.0 && slo.availability < 100.0) {
                    v.err(format!("service {name} : slo.availability doit être compris entre 0 et 100 (exclus)"));
                }
                if slo.error_rate_query.is_some() != slo.max_error_rate.is_some() {
                    v.err(format!("service {name} : error_rate_query et max_error_rate vont de pair"));
                }
            }
            for (env, se) in &s.environments {
                let ctx = format!("service {name}/{env}");
                if !self.environments.contains_key(env) {
                    v.err(format!("{ctx} : environnement inconnu"));
                }
                let Some(target) = self.targets.get(&se.target) else {
                    v.err(format!("{ctx} : cible inconnue « {} »", se.target));
                    continue;
                };
                match target {
                    Target::Kubernetes(_) => match &s.gitops {
                        None => v.err(format!("{ctx} : une cible Kubernetes exige la section gitops")),
                        Some(g) => {
                            if se.strategy != Strategy::Rolling && g.rollout.is_none() {
                                v.err(format!(
                                    "{ctx} : la stratégie {} exige gitops.rollout (Argo Rollouts)",
                                    se.strategy.as_str()
                                ));
                            }
                        }
                    },
                    Target::Vm(_) => {
                        if s.ansible.is_none() {
                            v.err(format!("{ctx} : une cible VM exige la section ansible"));
                        }
                        if se.strategy == Strategy::BlueGreen {
                            v.err(format!("{ctx} : blue_green n'est pas pris en charge sur VM (rolling ou canary)"));
                        }
                        if se.replicas.is_some() {
                            v.err(format!("{ctx} : replicas ne s'applique qu'à Kubernetes"));
                        }
                    }
                }
                if let Some(r) = se.replicas
                    && (r.min == 0 || r.min > r.max)
                {
                    v.err(format!("{ctx} : replicas invalides (1 ≤ min ≤ max)"));
                }
                if !names::is_valid_batch(&se.batch) {
                    v.err(format!("{ctx} : taille de lot invalide « {} »", se.batch));
                }
                if let Some(u) = &se.url
                    && !names::is_http_url(u)
                {
                    v.err(format!("{ctx} : URL invalide « {u} »"));
                }
                if let Some(p) = &se.gitops_path
                    && !is_safe_relative_path(p)
                {
                    v.err(format!("{ctx} : gitops_path non sûr « {p} »"));
                }
                if let Some(a) = &se.application
                    && !is_valid_name(&names::expand(a, env, name))
                {
                    v.err(format!("{ctx} : application Argo CD invalide « {a} »"));
                }
                if (se.gitops_path.is_some() || se.application.is_some()) && s.gitops.is_none() {
                    v.err(format!("{ctx} : gitops_path/application exigent la section gitops du service"));
                }
            }
        }
    }

    fn validate_backups(&self, v: &mut Validator) {
        for (name, b) in &self.backups {
            let ctx = format!("sauvegarde {name}");
            if name.len() > 40 {
                v.err(format!("{ctx} : nom trop long (40 caractères max.)"));
            }
            if !self.environments.contains_key(&b.environment) {
                v.err(format!("{ctx} : environnement inconnu « {} »", b.environment));
            }
            if b.every.is_zero() || b.rpo.is_zero() || b.rto.is_zero() {
                v.err(format!("{ctx} : every, rpo et rto doivent être non nuls"));
            }
            if b.every > b.rpo {
                v.err(format!(
                    "{ctx} : la cadence ({}) est plus longue que le RPO ({}) : l'objectif ne peut pas être tenu",
                    b.every, b.rpo
                ));
            }
            if b.retention.is_empty() {
                v.err(format!("{ctx} : la rétention ne conserve aucune sauvegarde"));
            }
            let expect_kind = |t: &str, kind: &str, v: &mut Validator| match self.targets.get(t) {
                None => v.err(format!("{ctx} : cible inconnue « {t} »")),
                Some(target) if target.kind_name() != kind => {
                    v.err(format!("{ctx} : la cible {t} doit être de type {kind}"));
                }
                _ => {}
            };
            match &b.source {
                BackupSource::Files(f) => {
                    expect_kind(&f.target, "vm", v);
                    if f.paths.is_empty() {
                        v.err(format!("{ctx} : aucun chemin à sauvegarder"));
                    }
                    for p in f.paths.iter().chain(&f.exclude) {
                        if !names::is_safe_absolute_path(p) {
                            v.err(format!("{ctx} : chemin absolu attendu « {p} »"));
                        }
                    }
                    for r in std::iter::once(&f.repository).chain(&f.copies) {
                        if r.trim().is_empty() || r.contains(char::is_whitespace) || r.starts_with('-') {
                            v.err(format!("{ctx} : dépôt restic invalide « {r} »"));
                        }
                    }
                }
                BackupSource::Postgres(p) => {
                    expect_kind(&p.target, "vm", v);
                    if !is_valid_name(&p.stanza) {
                        v.err(format!("{ctx} : stanza pgBackRest invalide"));
                    }
                }
                BackupSource::Kubernetes(k) => {
                    expect_kind(&k.target, "kubernetes", v);
                    if k.namespaces.is_empty() || k.namespaces.iter().any(|n| !is_valid_name(n)) {
                        v.err(format!("{ctx} : namespaces invalides"));
                    }
                }
                BackupSource::Command(c) => {
                    for argv in std::iter::once(&c.backup).chain(c.restore.iter()).chain(c.drill.iter()) {
                        if argv.first().is_none_or(|p| p.is_empty() || p.contains('/') || p.starts_with('-')) {
                            v.err(format!("{ctx} : commande invalide (programme nu attendu, ex. « aws »)"));
                        }
                    }
                }
            }
            if let Some(d) = &b.drill {
                if d.every.is_zero() {
                    v.err(format!("{ctx} : drill.every doit être non nul"));
                }
                if let Some(t) = &d.target
                    && !self.targets.contains_key(t)
                {
                    v.err(format!("{ctx} : cible d'exercice inconnue « {t} »"));
                }
                if let BackupSource::Command(c) = &b.source
                    && c.drill.is_none()
                {
                    v.err(format!("{ctx} : un exercice sur source « command » exige la commande drill"));
                }
            }
        }
    }

    fn validate_runbooks(&self, v: &mut Validator) {
        for (name, r) in &self.runbooks {
            if r.steps.is_empty() {
                v.err(format!("runbook {name} : aucune étape"));
            }
            for e in &r.environments {
                if !self.environments.contains_key(e) {
                    v.err(format!("runbook {name} : environnement inconnu « {e} »"));
                }
            }
            for step in &r.steps {
                match step {
                    RunbookStep::Command { program, cwd, .. } => {
                        if program.is_empty() || program.contains('/') || program.starts_with('-') {
                            v.err(format!("runbook {name} : programme invalide « {program} » (nom nu attendu)"));
                        }
                        if let Some(c) = cwd
                            && !is_safe_relative_path(c)
                        {
                            v.err(format!("runbook {name} : répertoire non sûr « {c} »"));
                        }
                    }
                    RunbookStep::Ansible { target, playbook, .. } => {
                        match self.targets.get(target) {
                            Some(Target::Vm(_)) => {}
                            Some(_) => v.err(format!("runbook {name} : la cible {target} n'est pas une VM")),
                            None => v.err(format!("runbook {name} : cible inconnue « {target} »")),
                        }
                        if !is_safe_relative_path(playbook) {
                            v.err(format!("runbook {name} : playbook non sûr « {playbook} »"));
                        }
                    }
                    RunbookStep::HttpCheck { url, .. } => {
                        if !names::is_http_url(url) {
                            v.err(format!("runbook {name} : URL invalide « {url} »"));
                        }
                    }
                }
            }
        }
    }

    fn validate_access(&self, v: &mut Validator) {
        if self.access.is_empty() {
            v.err("aucune liaison d'accès : au moins un administrateur est requis");
        }
        let has_admin = self.access.iter().any(|b| b.role == Role::Admin && b.environments.iter().any(|e| e == "*"));
        if !has_admin {
            v.err("aucune équipe n'a le rôle admin sur tous les environnements");
        }
        for (i, b) in self.access.iter().enumerate() {
            if !self.teams.contains_key(&b.team) {
                v.err(format!("access[{i}] : équipe inconnue « {} »", b.team));
            }
            for e in &b.environments {
                if e != "*" && !self.environments.contains_key(e) {
                    v.err(format!("access[{i}] : environnement inconnu « {e} »"));
                }
            }
            for s in &b.services {
                if s != "*" && !self.services.contains_key(s) {
                    v.err(format!("access[{i}] : service inconnu « {s} »"));
                }
            }
        }
    }

    fn validate_notifications(&self, v: &mut Validator) {
        for (name, c) in &self.notifications.channels {
            if !names::is_secret_ref(c.secret_ref()) {
                v.err(format!(
                    "canal {name} : référence de secret attendue (env:, file:, vault:, aws-sm:, azure-kv:, gcp-sm:), jamais la valeur en clair"
                ));
            }
        }
        for (i, r) in self.notifications.routes.iter().enumerate() {
            if r.events.is_empty() || r.channels.is_empty() {
                v.err(format!("notifications.routes[{i}] : events et channels sont obligatoires"));
            }
            for c in &r.channels {
                if !self.notifications.channels.contains_key(c) {
                    v.err(format!("notifications.routes[{i}] : canal inconnu « {c} »"));
                }
            }
            for e in &r.environments {
                if !self.environments.contains_key(e) {
                    v.err(format!("notifications.routes[{i}] : environnement inconnu « {e} »"));
                }
            }
        }
    }
}

#[derive(Default)]
struct Validator {
    errors: Vec<String>,
}

impl Validator {
    fn err(&mut self, msg: impl Into<String>) {
        self.errors.push(msg.into());
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Spécification minimale mais complète, partagée par les tests du crate.
    pub const SAMPLE: &str = r#"
version: 1
platform:
  name: lab
  owner_team: platform
  gitops_push: false
providers:
  aws-eu: { kind: aws, region: eu-west-3 }
  dc-paris: { kind: onprem }
targets:
  eks-dev: { kind: kubernetes, provider: aws-eu, context: eks-dev }
  eks-prod: { kind: kubernetes, provider: aws-eu, context: eks-prod }
  rhel-prod: { kind: vm, provider: dc-paris, inventory: inventory/prod.yml, group: app_servers,
               patching: { batch: "1", security_only: true } }
stacks:
  aws-core: { provider: aws-eu, path: infrastructure/terraform/aws, environments: [prod],
              var_files: ["env/{env}.tfvars"], backend_config: "backend/{env}.hcl" }
environments:
  dev: { tier: development, policy: relaxed }
  prod:
    tier: production
    policy: strict
    promote_from: dev
    alert_labels: { env: prod }
    change_windows:
      - { days: [mon, tue, wed, thu], start: "09:00", end: "16:00", timezone: Europe/Paris }
policies:
  relaxed: { approvals: 0 }
  strict:
    approvals: 1
    require_change_ticket: true
    change_ticket_pattern: "^CHG-[0-9]+$"
    require_tests: true
    require_signature: true
    max_critical: 0
    max_high: 0
    enforce_change_windows: true
    enforce_promotion: true
    block_on_incident: sev2
    allow_emergency: true
services:
  erp-app:
    team: erp
    image: ghcr.io/acme/erp-app
    gitops:
      path: "gitops/apps/erp/overlays/{env}"
      application: "erp-{env}"
      rollout: erp-app
    health: { path: /health }
    slo:
      availability: 99.9
      error_rate_query: 'sum(rate(http_requests_total{env="{env}",code=~"5.."}[5m]))'
      max_error_rate: 0.01
    environments:
      dev: { target: eks-dev, url: "https://erp.dev.example.org", wait: false }
      prod: { target: eks-prod, strategy: canary, url: "https://erp.example.org", replicas: { min: 3, max: 10 } }
  billing-worker:
    team: erp
    image: ghcr.io/acme/billing-worker
    ansible: { playbook: playbooks/deploy_service.yml }
    environments:
      prod: { target: rhel-prod, strategy: canary, url: "https://billing.example.org" }
backups:
  erp-files:
    environment: prod
    source: { kind: files, target: rhel-prod, paths: [/etc, /var/www], repository: "s3:s3.eu-west-3.amazonaws.com/acme-backups/erp",
              copies: ["swift:backup-gra:/erp"] }
    every: 15m
    rpo: 15m
    rto: 1h
    retention: { last: 4, daily: 7, weekly: 4, monthly: 12 }
    drill: { every: 7d }
  erp-k8s:
    environment: prod
    source: { kind: kubernetes, target: eks-prod, namespaces: [erp-prod] }
    every: 1h
    rpo: 1h
    rto: 1h
    retention: { daily: 7 }
runbooks:
  flush-cache:
    description: Vide le cache applicatif
    environments: [prod]
    steps:
      - { kind: ansible, name: purge, target: rhel-prod, playbook: playbooks/flush_cache.yml }
      - { kind: http_check, name: santé, url: "https://erp.example.org/health" }
teams:
  platform: { idp_groups: [socle-admins], members: [root@example.org] }
  erp: { idp_groups: [erp-devs] }
  sre: { idp_groups: [sre] }
access:
  - { team: platform, role: admin }
  - { team: sre, role: approver, environments: [prod] }
  - { team: sre, role: operator }
  - { team: erp, role: deployer, environments: [dev, prod], services: [erp-app, billing-worker] }
notifications:
  channels:
    ops: { kind: slack, webhook: "env:SOCLE_SLACK_WEBHOOK" }
  routes:
    - { events: ["operation.failed", "incident.*"], channels: [ops] }
observability:
  prometheus_url: http://prometheus:9090
  alertmanager_url: http://alertmanager:9093
"#;

    pub fn sample() -> Spec {
        Spec::from_yaml(SAMPLE).expect("spécification d'exemple valide")
    }

    #[test]
    fn sample_is_valid() {
        let s = sample();
        assert_eq!(s.services.len(), 2);
        assert!(matches!(s.targets["rhel-prod"], Target::Vm(_)));
        assert_eq!(s.hash().len(), 64);
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let bad = SAMPLE.replace("tier: development", "tier: development\n    tpyo: 1");
        assert!(Spec::from_yaml(&bad).is_err());
        let bad = SAMPLE.replace("context: eks-dev", "context: eks-dev, bogus: true");
        assert!(Spec::from_yaml(&bad).is_err(), "champ inconnu dans une cible");
    }

    #[test]
    fn cross_references_are_checked() {
        let bad = SAMPLE
            .replace("policy: relaxed", "policy: missing")
            .replace("target: eks-dev, url", "target: nowhere, url")
            .replace("webhook: \"env:SOCLE_SLACK_WEBHOOK\"", "webhook: \"https://hooks.slack.com/x\"")
            .replace("every: 15m", "every: 30m");
        let Err(CoreError::InvalidSpec(errors)) = Spec::from_yaml(&bad) else {
            panic!("la spécification aurait dû être rejetée");
        };
        let all = errors.join("\n");
        assert!(all.contains("politique inconnue"), "{all}");
        assert!(all.contains("cible inconnue"), "{all}");
        assert!(all.contains("référence de secret"), "{all}");
        assert!(all.contains("RPO"), "{all}");
    }

    #[test]
    fn strategy_requirements() {
        let bad = SAMPLE.replace("      rollout: erp-app\n", "");
        let err = Spec::from_yaml(&bad).expect_err("canary sans rollout");
        assert!(err.to_string().contains("gitops.rollout"));
    }

    #[test]
    fn route_patterns() {
        assert!(pattern_matches("*", "operation.failed"));
        assert!(pattern_matches("incident.*", "incident.opened"));
        assert!(!pattern_matches("incident.*", "incidents.opened"));
        assert!(pattern_matches("operation.failed", "operation.failed"));
    }
}
