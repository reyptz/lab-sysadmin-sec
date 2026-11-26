//! Interface en ligne de commande. Deux modes, même moteur :
//! * **distant** (défaut) : client de l'API (`SOCLE_URL`, `SOCLE_TOKEN`) ;
//! * **local** (`--local`) : moteur embarqué sur une base SQLite, pour les
//!   labs et la CI (les opérations soumises à approbation exigent le serveur).

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, anyhow, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use socle_core::audit::AuditEvent;
use socle_core::duration::HumanDuration;
use socle_core::incident::{Incident, IncidentStatus, NewIncident, Severity};
use socle_core::operation::{Action, ApprovalDecision, GateStatus, OpStatus, Operation, OperationRequest};
use socle_core::rbac::{Principal, PrincipalKind};
use socle_core::release::{Evidence, NewRelease, Release, VulnSummary};
use socle_core::spec::Spec;
use tokio::sync::watch;

use crate::api::{self, AppState};
use crate::auth::{Authenticator, OidcConfig};
use crate::engine::{Engine, EngineConfig, IncidentPatch};
use crate::executor::ProcessRunner;
use crate::model::{LogLine, OperationFilter};
use crate::store::Store;

// ---------------------------------------------------------------------------
// Définition des commandes
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(
    name = "socle",
    version,
    about = "Plan de contrôle du cycle de vie des déploiements et des infrastructures",
    long_about = "socle orchestre déploiements, CI/CD, supervision, scalabilité, environnements, \
                  sauvegardes, restaurations, sécurité, incidents et maintenance — multi-cloud et on-premise, \
                  Kubernetes et VMs RHEL — avec portes DevSecOps, approbations et audit infalsifiable."
)]
pub struct Cli {
    #[command(flatten)]
    global: Global,
    #[command(subcommand)]
    command: Command,
}

#[derive(Args, Clone)]
struct Global {
    /// URL du serveur socle.
    #[arg(long, env = "SOCLE_URL", global = true)]
    server: Option<String>,
    /// Jeton d'API (socle_pat_…) ou jeton OIDC.
    #[arg(long, env = "SOCLE_TOKEN", global = true, hide_env_values = true)]
    token: Option<String>,
    /// Moteur embarqué (lab, CI) au lieu du serveur.
    #[arg(long, global = true)]
    local: bool,
    /// Spécification de la plateforme.
    #[arg(long, env = "SOCLE_SPEC", default_value = "socle.yaml", global = true)]
    spec: PathBuf,
    /// Base de données (mode local et serveur).
    #[arg(
        long,
        env = "SOCLE_DATABASE_URL",
        default_value = "sqlite:.socle/socle.db?mode=rwc",
        global = true,
        hide_env_values = true
    )]
    database: String,
    /// Racine du dépôt de travail (GitOps, Ansible, Terraform).
    #[arg(long, env = "SOCLE_REPO_ROOT", default_value = ".", global = true)]
    repo_root: PathBuf,
    /// Format de sortie.
    #[arg(long, value_enum, default_value_t = Output::Text, global = true)]
    output: Output,
}

#[derive(ValueEnum, Clone, Copy, PartialEq, Eq)]
enum Output {
    Text,
    Json,
}

#[derive(Subcommand)]
enum Command {
    /// Valide socle.yaml (références croisées comprises), sans serveur.
    Validate,
    /// Démarre le serveur : API, console, workers et ordonnanceur.
    Serve(ServeArgs),
    /// Vue d'ensemble : environnements, approbations, incidents, sauvegardes.
    Status,
    /// Environnements et versions déployées (avec détection de dérive).
    Env,
    /// Releases (artefacts immuables et preuves de la CI).
    #[command(subcommand)]
    Release(ReleaseCmd),
    /// Déploie une release.
    Deploy(DeployArgs),
    /// Promeut la release en place dans l'environnement source.
    Promote(PromoteArgs),
    /// Rétablit la release précédente.
    Rollback(RollbackArgs),
    /// Ajuste les bornes d'autoscaling (Kubernetes).
    Scale(ScaleArgs),
    /// Infrastructure multi-cloud (Terraform / OpenTofu).
    #[command(subcommand)]
    Infra(InfraCmd),
    /// Sauvegardes, restaurations et exercices.
    #[command(subcommand)]
    Backup(BackupCmd),
    /// Applique les correctifs RHEL d'une cible, par lots.
    Patch(TargetArgs),
    /// Exécute un runbook.
    Runbook(TargetArgs),
    /// Opérations : liste, détail, journaux.
    #[command(subcommand)]
    Ops(OpsCmd),
    /// Approuve une opération (quatre yeux).
    Approve(DecisionArgs),
    /// Refuse une opération (motif obligatoire).
    Reject(RejectArgs),
    /// Annule une opération non démarrée.
    Cancel(CancelArgs),
    /// Incidents.
    #[command(subcommand)]
    Incident(IncidentCmd),
    /// Fenêtres de maintenance.
    #[command(subcommand)]
    Maintenance(MaintenanceCmd),
    /// Journal d'audit infalsifiable.
    #[command(subcommand)]
    Audit(AuditCmd),
    /// Rapport de conformité (ISO 27001 / SOC 2).
    Compliance(ComplianceArgs),
    /// Jetons d'API.
    #[command(subcommand)]
    Token(TokenCmd),
}

#[derive(Args)]
struct ServeArgs {
    #[arg(long, env = "SOCLE_BIND", default_value = "0.0.0.0:8080")]
    bind: String,
    /// Nombre de workers d'exécution.
    #[arg(long, env = "SOCLE_WORKERS", default_value_t = 2)]
    workers: usize,
    /// URL publique de la console (liens des notifications).
    #[arg(long, env = "SOCLE_PUBLIC_URL")]
    public_url: Option<String>,
    /// Programmes autorisés (séparés par des virgules).
    #[arg(long, env = "SOCLE_ALLOWED_PROGRAMS", value_delimiter = ',')]
    allowed_programs: Option<Vec<String>>,
    /// Préfixes de variables d'environnement transmis aux outils.
    #[arg(long, env = "SOCLE_ENV_PASSTHROUGH", value_delimiter = ',')]
    env_passthrough: Vec<String>,
    #[arg(long, env = "SOCLE_OIDC_ISSUER")]
    oidc_issuer: Option<String>,
    #[arg(long, env = "SOCLE_OIDC_AUDIENCE", default_value = "socle")]
    oidc_audience: String,
    #[arg(long, env = "SOCLE_OIDC_JWKS_URL")]
    oidc_jwks_url: Option<String>,
    #[arg(long, env = "SOCLE_OIDC_GROUPS_CLAIM", default_value = "groups")]
    oidc_groups_claim: String,
    #[arg(long, env = "SOCLE_OIDC_SUBJECT_CLAIM", default_value = "email")]
    oidc_subject_claim: String,
    /// Désactive l'ordonnanceur (sauvegardes, exercices, dérive) sur ce réplica.
    #[arg(long, env = "SOCLE_NO_SCHEDULER")]
    no_scheduler: bool,
}

#[derive(Args, Clone)]
struct Change {
    /// Environnement cible.
    #[arg(long, short = 'e')]
    env: String,
    /// Motif du changement.
    #[arg(long, default_value = "")]
    reason: String,
    /// Référence du ticket de changement (ex. CHG-1234).
    #[arg(long)]
    ticket: Option<String>,
    /// Procédure d'urgence (motif obligatoire, revue a posteriori).
    #[arg(long)]
    emergency: bool,
    /// Affiche le plan et les portes sans rien soumettre.
    #[arg(long)]
    dry_run: bool,
    /// Attend la fin de l'exécution (code de sortie ≠ 0 en cas d'échec).
    #[arg(long)]
    wait: bool,
}

#[derive(Args)]
struct DeployArgs {
    service: String,
    /// Release : identifiant, digest (sha256:…) ou version.
    #[arg(long)]
    release: String,
    #[command(flatten)]
    change: Change,
}

#[derive(Args)]
struct PromoteArgs {
    service: String,
    /// Environnement source (défaut : promote_from de l'environnement cible).
    #[arg(long)]
    from: Option<String>,
    #[command(flatten)]
    change: Change,
}

#[derive(Args)]
struct RollbackArgs {
    service: String,
    /// Release à rétablir (défaut : la précédente).
    #[arg(long)]
    to: Option<String>,
    #[command(flatten)]
    change: Change,
}

#[derive(Args)]
struct ScaleArgs {
    service: String,
    #[arg(long)]
    min: u32,
    #[arg(long)]
    max: u32,
    #[command(flatten)]
    change: Change,
}

#[derive(Args)]
struct TargetArgs {
    /// Cible (patch) ou nom du runbook.
    name: String,
    #[command(flatten)]
    change: Change,
}

#[derive(Subcommand)]
enum ReleaseCmd {
    /// Enregistre une release et ses preuves (appelé par la CI).
    Register(RegisterArgs),
    /// Liste les releases.
    List {
        #[arg(long)]
        service: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: i64,
    },
}

#[derive(Args)]
struct RegisterArgs {
    #[arg(long)]
    service: String,
    #[arg(long)]
    version: String,
    #[arg(long)]
    digest: String,
    #[arg(long)]
    git_sha: Option<String>,
    #[arg(long)]
    image: Option<String>,
    #[arg(long)]
    tests_passed: Option<bool>,
    /// Rapport Trivy JSON (comptage automatique des vulnérabilités).
    #[arg(long)]
    trivy_report: Option<PathBuf>,
    #[arg(long)]
    critical: Option<u32>,
    #[arg(long)]
    high: Option<u32>,
    #[arg(long)]
    signed: bool,
    #[arg(long)]
    sbom: Option<String>,
    #[arg(long)]
    provenance: Option<String>,
    #[arg(long)]
    iac_scan_passed: Option<bool>,
    #[arg(long)]
    secret_scan_passed: Option<bool>,
}

#[derive(Subcommand)]
enum InfraCmd {
    /// Calcule et empreinte le plan d'une stack.
    Plan {
        stack: String,
        #[command(flatten)]
        change: Change,
    },
    /// Applique exactement un plan calculé (et approuvé).
    Apply {
        stack: String,
        /// Identifiant de l'opération `infra plan`.
        #[arg(long)]
        plan_op: String,
        #[command(flatten)]
        change: Change,
    },
}

#[derive(Subcommand)]
enum BackupCmd {
    /// Conformité RPO/RTO de toutes les politiques.
    Status,
    /// Lance une sauvegarde.
    Run {
        policy: String,
        #[command(flatten)]
        change: Change,
    },
    /// Lance un exercice de restauration (mesure du RTO).
    Drill {
        policy: String,
        #[command(flatten)]
        change: Change,
    },
    /// Restaure (destructif : confirmation explicite exigée).
    Restore {
        policy: String,
        #[arg(long)]
        snapshot: Option<String>,
        /// Instant cible RFC 3339 (PostgreSQL / commandes fournisseur).
        #[arg(long)]
        at: Option<String>,
        /// Restauration en place (écrase l'existant).
        #[arg(long)]
        in_place: bool,
        /// Retapez le nom de l'environnement pour confirmer.
        #[arg(long)]
        confirm: String,
        #[command(flatten)]
        change: Change,
    },
}

#[derive(Subcommand)]
enum OpsCmd {
    List {
        #[arg(long, short = 'e')]
        env: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        kind: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: i64,
    },
    Show {
        id: String,
    },
    Logs {
        id: String,
        /// Suit le journal jusqu'à la fin de l'opération.
        #[arg(long, short = 'f')]
        follow: bool,
    },
}

#[derive(Args)]
struct DecisionArgs {
    id: String,
    #[arg(long, default_value = "")]
    comment: String,
}

#[derive(Args)]
struct RejectArgs {
    id: String,
    #[arg(long)]
    comment: String,
}

#[derive(Args)]
struct CancelArgs {
    id: String,
    #[arg(long, default_value = "")]
    reason: String,
}

#[derive(Subcommand)]
enum IncidentCmd {
    List {
        /// Inclut les incidents résolus et clos.
        #[arg(long)]
        all: bool,
    },
    Open {
        #[arg(long)]
        title: String,
        #[arg(long, default_value = "sev3")]
        severity: String,
        #[arg(long, short = 'e')]
        env: Option<String>,
        #[arg(long = "service")]
        services: Vec<String>,
        #[arg(long, default_value = "")]
        summary: String,
    },
    Show {
        id: String,
    },
    Ack(IncidentNote),
    Mitigate(IncidentNote),
    Resolve(IncidentNote),
    Close(IncidentNote),
    Reopen(IncidentNote),
    Note {
        id: String,
        text: String,
    },
    Update {
        id: String,
        #[arg(long)]
        severity: Option<String>,
        #[arg(long)]
        root_cause: Option<String>,
        #[arg(long)]
        postmortem_url: Option<String>,
        #[arg(long)]
        commander: Option<String>,
    },
    /// Postmortem Markdown.
    Postmortem {
        id: String,
    },
}

#[derive(Args)]
struct IncidentNote {
    id: String,
    #[arg(long)]
    note: Option<String>,
}

#[derive(Subcommand)]
enum MaintenanceCmd {
    List {
        #[arg(long)]
        all: bool,
    },
    Start {
        #[arg(long, short = 'e')]
        env: String,
        #[arg(long)]
        duration: HumanDuration,
        #[arg(long)]
        reason: String,
    },
    End {
        id: String,
    },
}

#[derive(Subcommand)]
enum AuditCmd {
    List {
        #[arg(long, default_value_t = 30)]
        limit: i64,
    },
    /// Vérifie l'intégrité de toute la chaîne.
    Verify,
    /// Exporte le journal (JSON Lines).
    Export {
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[derive(Args)]
struct ComplianceArgs {
    #[arg(long, default_value_t = 90)]
    days: i64,
    /// Rendu Markdown (dossier de preuves).
    #[arg(long)]
    markdown: bool,
}

#[derive(Subcommand)]
enum TokenCmd {
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        subject: String,
        #[arg(long = "team", required = true)]
        teams: Vec<String>,
        #[arg(long)]
        ttl: Option<HumanDuration>,
        /// Amorçage du premier administrateur (accès direct à la base).
        #[arg(long)]
        bootstrap: bool,
    },
    List,
    Revoke {
        id: String,
    },
}

// ---------------------------------------------------------------------------
// Point d'entrée
// ---------------------------------------------------------------------------

pub fn main() -> ExitCode {
    let cli = Cli::parse();
    let serve = matches!(cli.command, Command::Serve(_));
    init_tracing(serve);
    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("erreur : runtime indisponible : {e}");
            return ExitCode::from(1);
        }
    };
    match runtime.block_on(run(cli)) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("erreur : {e:#}");
            ExitCode::from(1)
        }
    }
}

fn init_tracing(serve: bool) {
    use tracing_subscriber::EnvFilter;
    let filter =
        EnvFilter::try_from_env("SOCLE_LOG").unwrap_or_else(|_| EnvFilter::new(if serve { "info" } else { "warn" }));
    let json = std::env::var("SOCLE_LOG_FORMAT").is_ok_and(|v| v == "json");
    let builder = tracing_subscriber::fmt().with_env_filter(filter).with_writer(std::io::stderr);
    if json {
        builder.json().init();
    } else {
        builder.init();
    }
}

fn load_spec(path: &PathBuf) -> anyhow::Result<Spec> {
    let text = std::fs::read_to_string(path).with_context(|| format!("lecture de {}", path.display()))?;
    Ok(Spec::from_yaml(&text)?)
}

async fn local_engine(g: &Global) -> anyhow::Result<Arc<Engine>> {
    let spec = load_spec(&g.spec)?;
    if let Some(path) = g
        .database
        .strip_prefix("sqlite://")
        .or_else(|| g.database.strip_prefix("sqlite:"))
        .and_then(|p| p.split('?').next())
        && let Some(parent) = std::path::Path::new(path).parent().filter(|p| !p.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).with_context(|| format!("création de {}", parent.display()))?;
    }
    let store = Store::connect(&g.database).await?;
    let runner = Arc::new(ProcessRunner::new(g.repo_root.clone(), None, Vec::new()));
    Ok(Arc::new(Engine::new(spec, Some(g.spec.clone()), store, runner, EngineConfig::new(g.repo_root.clone()))))
}

fn local_principal(spec: &Spec) -> Principal {
    let id = std::env::var("SOCLE_ACTOR").ok().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| {
        let user = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_else(|_| "operateur".into());
        format!("local:{user}")
    });
    Principal {
        display_name: id.clone(),
        id,
        kind: PrincipalKind::Local,
        teams: BTreeSet::from([spec.platform.owner_team.clone()]),
    }
}

async fn backend(g: &Global) -> anyhow::Result<Backend> {
    if g.local {
        let engine = local_engine(g).await?;
        let principal = local_principal(&engine.spec());
        return Ok(Backend::Local { engine, principal });
    }
    let base = g
        .server
        .clone()
        .ok_or_else(|| anyhow!("serveur non défini : --server / SOCLE_URL, ou --local pour le moteur embarqué"))?;
    Ok(Backend::Remote(Client::new(base, g.token.clone())?))
}

async fn run(cli: Cli) -> anyhow::Result<ExitCode> {
    let g = cli.global;
    let out = g.output;
    match cli.command {
        Command::Validate => {
            let spec = load_spec(&g.spec)?;
            if out == Output::Json {
                print_json(&json!({"valid": true, "hash": spec.hash()}));
            } else {
                println!(
                    "✓ {} valide — {} environnement(s), {} service(s), {} cible(s), {} stack(s), {} politique(s) de sauvegarde",
                    g.spec.display(),
                    spec.environments.len(),
                    spec.services.len(),
                    spec.targets.len(),
                    spec.stacks.len(),
                    spec.backups.len()
                );
                println!("  empreinte {}", spec.hash());
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Serve(a) => serve(&g, a).await.map(|()| ExitCode::SUCCESS),
        Command::Token(TokenCmd::Create { name, subject, teams, ttl, bootstrap }) if bootstrap => {
            let engine = local_engine(&g).await?;
            let (token, secret) = engine.create_token(None, &name, &subject, teams, ttl).await?;
            print_token(out, &serde_json::to_value(&token)?, &secret);
            Ok(ExitCode::SUCCESS)
        }
        command => {
            let b = backend(&g).await?;
            dispatch(&b, command, out).await
        }
    }
}

// ---------------------------------------------------------------------------
// Serveur
// ---------------------------------------------------------------------------

async fn serve(g: &Global, a: ServeArgs) -> anyhow::Result<()> {
    let spec = load_spec(&g.spec)?;
    if let Some(path) = g
        .database
        .strip_prefix("sqlite://")
        .or_else(|| g.database.strip_prefix("sqlite:"))
        .and_then(|p| p.split('?').next())
        && let Some(parent) = std::path::Path::new(path).parent().filter(|p| !p.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    let store = Store::connect(&g.database).await?;
    if !store.is_postgres() {
        tracing::warn!("base SQLite : instance unique. Utilisez PostgreSQL pour la haute disponibilité.");
    }
    let runner =
        Arc::new(ProcessRunner::new(g.repo_root.clone(), a.allowed_programs.clone(), a.env_passthrough.clone()));
    let mut cfg = EngineConfig::new(g.repo_root.clone());
    cfg.public_url.clone_from(&a.public_url);
    let engine = Arc::new(Engine::new(spec, Some(g.spec.clone()), store, runner, cfg));
    let oidc = a.oidc_issuer.clone().map(|issuer| OidcConfig {
        issuer,
        audience: a.oidc_audience.clone(),
        jwks_url: a.oidc_jwks_url.clone(),
        groups_claim: a.oidc_groups_claim.clone(),
        subject_claim: a.oidc_subject_claim.clone(),
    });
    let auth = Arc::new(Authenticator::new(Arc::clone(&engine), oidc));

    let (stop_tx, stop_rx) = watch::channel(false);
    let mut tasks = Vec::new();
    for _ in 0..a.workers.max(1) {
        let (engine, mut stop) = (Arc::clone(&engine), stop_rx.clone());
        tasks.push(tokio::spawn(async move {
            while !*stop.borrow() {
                match engine.run_next().await {
                    Ok(Some(_)) => continue,
                    Ok(None) => {}
                    Err(e) => tracing::error!("worker : {e}"),
                }
                tokio::select! {
                    () = tokio::time::sleep(Duration::from_secs(2)) => {}
                    _ = stop.changed() => {}
                }
            }
        }));
    }
    if !a.no_scheduler {
        let (engine, mut stop) = (Arc::clone(&engine), stop_rx.clone());
        tasks.push(tokio::spawn(async move {
            while !*stop.borrow() {
                if let Err(e) = engine.scheduler_tick().await {
                    tracing::error!("ordonnanceur : {e}");
                }
                tokio::select! {
                    () = tokio::time::sleep(Duration::from_secs(30)) => {}
                    _ = stop.changed() => {}
                }
            }
        }));
    }

    let app = api::router(AppState { engine: Arc::clone(&engine), auth });
    let listener = tokio::net::TcpListener::bind(&a.bind).await.with_context(|| format!("écoute sur {}", a.bind))?;
    tracing::info!(
        adresse = %a.bind,
        workers = a.workers,
        instance = %engine.config().worker_id,
        "socle {} démarré",
        env!("CARGO_PKG_VERSION")
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            tracing::info!("arrêt demandé : fin des opérations en cours…");
            let _ = stop_tx.send(true);
        })
        .await?;
    let _ = tokio::time::timeout(Duration::from_secs(300), futures_join(tasks)).await;
    Ok(())
}

async fn futures_join(tasks: Vec<tokio::task::JoinHandle<()>>) {
    for t in tasks {
        let _ = t.await;
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {}
        () = term => {}
    }
}

// ---------------------------------------------------------------------------
// Accès : client HTTP ou moteur embarqué
// ---------------------------------------------------------------------------

struct Client {
    http: reqwest::Client,
    base: String,
    token: Option<String>,
}

impl Client {
    fn new(base: String, token: Option<String>) -> anyhow::Result<Self> {
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .user_agent(concat!("socle-cli/", env!("CARGO_PKG_VERSION")))
                .build()?,
            base: base.trim_end_matches('/').to_string(),
            token,
        })
    }

    async fn raw(&self, method: Method, path: &str, body: Option<&Value>) -> anyhow::Result<reqwest::Response> {
        let mut req = self.http.request(method, format!("{}/api/v1{path}", self.base));
        if let Some(t) = &self.token {
            req = req.bearer_auth(t);
        }
        if let Some(b) = body {
            req = req.json(b);
        }
        let res = req.send().await.with_context(|| format!("serveur {} injoignable", self.base))?;
        if res.status().is_success() {
            return Ok(res);
        }
        let status = res.status();
        let v: Value = res.json().await.unwrap_or(Value::Null);
        let msg = v.pointer("/error/message").and_then(Value::as_str).unwrap_or("erreur inconnue");
        bail!("{msg} (HTTP {})", status.as_u16())
    }

    async fn call<T: DeserializeOwned>(&self, method: Method, path: &str, body: Option<&Value>) -> anyhow::Result<T> {
        Ok(self.raw(method, path, body).await?.json().await?)
    }

    async fn text(&self, path: &str) -> anyhow::Result<String> {
        Ok(self.raw(Method::GET, path, None).await?.text().await?)
    }
}

enum Backend {
    Remote(Client),
    Local { engine: Arc<Engine>, principal: Principal },
}

fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

impl Backend {
    async fn submit(&self, req: &OperationRequest) -> anyhow::Result<Operation> {
        match self {
            Backend::Remote(c) => c.call(Method::POST, "/operations", Some(&serde_json::to_value(req)?)).await,
            Backend::Local { engine, principal } => Ok(engine.submit(principal, req.clone()).await?),
        }
    }

    async fn preview(&self, req: &OperationRequest) -> anyhow::Result<Value> {
        match self {
            Backend::Remote(c) => c.call(Method::POST, "/operations/preview", Some(&serde_json::to_value(req)?)).await,
            Backend::Local { engine, principal } => Ok(serde_json::to_value(engine.preview(principal, req).await?)?),
        }
    }

    async fn operation(&self, id: &str) -> anyhow::Result<Operation> {
        match self {
            Backend::Remote(c) => c.call(Method::GET, &format!("/operations/{}", enc(id)), None).await,
            Backend::Local { engine, principal } => Ok(engine.get_operation(principal, id).await?),
        }
    }

    async fn operations(&self, f: &OperationFilter) -> anyhow::Result<Vec<Operation>> {
        match self {
            Backend::Remote(c) => {
                let mut q = vec![format!("limit={}", f.limit.unwrap_or(20))];
                for (k, v) in
                    [("environment", &f.environment), ("status", &f.status), ("kind", &f.kind), ("subject", &f.subject)]
                {
                    if let Some(v) = v {
                        q.push(format!("{k}={}", enc(v)));
                    }
                }
                c.call(Method::GET, &format!("/operations?{}", q.join("&")), None).await
            }
            Backend::Local { engine, principal } => Ok(engine.list_operations(principal, f).await?),
        }
    }

    async fn logs(&self, id: &str, after: i64) -> anyhow::Result<Vec<LogLine>> {
        match self {
            Backend::Remote(c) => {
                c.call(Method::GET, &format!("/operations/{}/logs?after={after}", enc(id)), None).await
            }
            Backend::Local { engine, principal } => Ok(engine.operation_logs(principal, id, after, 5_000).await?),
        }
    }

    async fn decide(&self, id: &str, d: ApprovalDecision, comment: &str) -> anyhow::Result<Operation> {
        match self {
            Backend::Remote(c) => {
                let path = match d {
                    ApprovalDecision::Approved => "approve",
                    ApprovalDecision::Rejected => "reject",
                };
                c.call(Method::POST, &format!("/operations/{}/{path}", enc(id)), Some(&json!({"comment": comment})))
                    .await
            }
            Backend::Local { engine, principal } => Ok(engine.decide(principal, id, d, comment).await?),
        }
    }

    async fn cancel(&self, id: &str, reason: &str) -> anyhow::Result<Operation> {
        match self {
            Backend::Remote(c) => {
                c.call(Method::POST, &format!("/operations/{}/cancel", enc(id)), Some(&json!({"reason": reason}))).await
            }
            Backend::Local { engine, principal } => Ok(engine.cancel(principal, id, reason).await?),
        }
    }

    async fn register_release(&self, r: &NewRelease) -> anyhow::Result<Release> {
        match self {
            Backend::Remote(c) => c.call(Method::POST, "/releases", Some(&serde_json::to_value(r)?)).await,
            Backend::Local { engine, principal } => Ok(engine.register_release(principal, r.clone()).await?),
        }
    }

    async fn releases(&self, service: Option<&str>, limit: i64) -> anyhow::Result<Vec<Release>> {
        match self {
            Backend::Remote(c) => {
                let q = service.map(|s| format!("&service={}", enc(s))).unwrap_or_default();
                c.call(Method::GET, &format!("/releases?limit={limit}{q}"), None).await
            }
            Backend::Local { engine, principal } => Ok(engine.list_releases(principal, service, limit).await?),
        }
    }

    async fn view(&self, path: &str) -> anyhow::Result<Value> {
        match self {
            Backend::Remote(c) => c.call(Method::GET, path, None).await,
            Backend::Local { engine, principal } => Ok(match path {
                "/overview" => serde_json::to_value(engine.overview(principal).await?)?,
                "/environments" => serde_json::to_value(engine.environments(principal).await?)?,
                "/backups" => serde_json::to_value(engine.backup_statuses(principal).await?)?,
                "/audit/verify" => serde_json::to_value(engine.verify_audit(principal).await?)?,
                "/maintenance?active=true" => serde_json::to_value(engine.list_maintenance(principal, true).await?)?,
                "/maintenance" => serde_json::to_value(engine.list_maintenance(principal, false).await?)?,
                "/tokens" => serde_json::to_value(engine.list_tokens(principal).await?)?,
                other => bail!("vue {other} indisponible en mode local"),
            }),
        }
    }

    async fn incidents(&self, active: bool) -> anyhow::Result<Vec<Incident>> {
        match self {
            Backend::Remote(c) => c.call(Method::GET, &format!("/incidents?active={active}"), None).await,
            Backend::Local { engine, principal } => Ok(engine.list_incidents(principal, active, 100).await?),
        }
    }

    async fn incident(&self, id: &str) -> anyhow::Result<Incident> {
        match self {
            Backend::Remote(c) => c.call(Method::GET, &format!("/incidents/{}", enc(id)), None).await,
            Backend::Local { engine, principal } => Ok(engine.get_incident(principal, id).await?),
        }
    }

    async fn open_incident(&self, n: &NewIncident) -> anyhow::Result<Incident> {
        match self {
            Backend::Remote(c) => c.call(Method::POST, "/incidents", Some(&serde_json::to_value(n)?)).await,
            Backend::Local { engine, principal } => Ok(engine.open_incident(principal, n.clone()).await?),
        }
    }

    async fn transition(&self, id: &str, to: IncidentStatus, note: Option<&str>) -> anyhow::Result<Incident> {
        match self {
            Backend::Remote(c) => {
                c.call(
                    Method::POST,
                    &format!("/incidents/{}/transition", enc(id)),
                    Some(&json!({"status": to.as_str(), "note": note})),
                )
                .await
            }
            Backend::Local { engine, principal } => Ok(engine.transition_incident(principal, id, to, note).await?),
        }
    }

    async fn note(&self, id: &str, note: &str) -> anyhow::Result<Incident> {
        match self {
            Backend::Remote(c) => {
                c.call(Method::POST, &format!("/incidents/{}/notes", enc(id)), Some(&json!({"note": note}))).await
            }
            Backend::Local { engine, principal } => Ok(engine.add_incident_note(principal, id, note).await?),
        }
    }

    async fn update_incident(&self, id: &str, patch: &IncidentPatch) -> anyhow::Result<Incident> {
        match self {
            Backend::Remote(c) => {
                c.call(Method::PATCH, &format!("/incidents/{}", enc(id)), Some(&serde_json::to_value(patch)?)).await
            }
            Backend::Local { engine, principal } => Ok(engine.update_incident(principal, id, patch.clone()).await?),
        }
    }

    async fn start_maintenance(&self, env: &str, duration: HumanDuration, reason: &str) -> anyhow::Result<Value> {
        match self {
            Backend::Remote(c) => {
                c.call(
                    Method::POST,
                    "/maintenance",
                    Some(&json!({"environment": env, "duration": duration, "reason": reason})),
                )
                .await
            }
            Backend::Local { engine, principal } => {
                Ok(serde_json::to_value(engine.start_maintenance(principal, env, duration, reason).await?)?)
            }
        }
    }

    async fn end_maintenance(&self, id: &str) -> anyhow::Result<Value> {
        match self {
            Backend::Remote(c) => c.call(Method::POST, &format!("/maintenance/{}/end", enc(id)), None).await,
            Backend::Local { engine, principal } => {
                Ok(serde_json::to_value(engine.end_maintenance(principal, id).await?)?)
            }
        }
    }

    async fn audit_page(&self, after: Option<i64>, limit: i64) -> anyhow::Result<Vec<AuditEvent>> {
        match self {
            Backend::Remote(c) => {
                let q = after.map(|a| format!("&after={a}")).unwrap_or_default();
                c.call(Method::GET, &format!("/audit?limit={limit}{q}"), None).await
            }
            Backend::Local { engine, principal } => Ok(engine.audit_events(principal, after, limit).await?),
        }
    }

    async fn compliance(&self, days: i64, markdown: bool) -> anyhow::Result<String> {
        match self {
            Backend::Remote(c) => {
                if markdown {
                    c.text(&format!("/compliance?days={days}&format=md")).await
                } else {
                    let v: Value = c.call(Method::GET, &format!("/compliance?days={days}"), None).await?;
                    Ok(serde_json::to_string_pretty(&v)?)
                }
            }
            Backend::Local { engine, principal } => {
                let r = engine.compliance_report(principal, days).await?;
                Ok(if markdown { r.to_markdown() } else { serde_json::to_string_pretty(&r)? })
            }
        }
    }

    async fn create_token(
        &self,
        name: &str,
        subject: &str,
        teams: &[String],
        ttl: Option<HumanDuration>,
    ) -> anyhow::Result<(Value, String)> {
        match self {
            Backend::Remote(c) => {
                let v: Value = c
                    .call(
                        Method::POST,
                        "/tokens",
                        Some(&json!({"name": name, "subject": subject, "teams": teams, "ttl": ttl})),
                    )
                    .await?;
                let secret = v["secret"].as_str().unwrap_or_default().to_string();
                Ok((v["token"].clone(), secret))
            }
            Backend::Local { engine, principal } => {
                let (t, s) = engine.create_token(Some(principal), name, subject, teams.to_vec(), ttl).await?;
                Ok((serde_json::to_value(t)?, s))
            }
        }
    }

    async fn revoke_token(&self, id: &str) -> anyhow::Result<()> {
        match self {
            Backend::Remote(c) => c.raw(Method::DELETE, &format!("/tokens/{}", enc(id)), None).await.map(|_| ()),
            Backend::Local { engine, principal } => Ok(engine.revoke_token(principal, id).await?),
        }
    }
}

// ---------------------------------------------------------------------------
// Exécution des commandes
// ---------------------------------------------------------------------------

fn request(change: &Change, action: Action) -> OperationRequest {
    OperationRequest {
        environment: change.env.clone(),
        action,
        reason: change.reason.clone(),
        change_ticket: change.ticket.clone(),
        emergency: change.emergency,
    }
}

async fn dispatch(b: &Backend, command: Command, out: Output) -> anyhow::Result<ExitCode> {
    match command {
        Command::Validate | Command::Serve(_) => unreachable!("traité en amont"),
        Command::Status => {
            let v = b.view("/overview").await?;
            if out == Output::Json {
                print_json(&v);
            } else {
                print_overview(&v);
            }
        }
        Command::Env => {
            let v = b.view("/environments").await?;
            if out == Output::Json {
                print_json(&v);
            } else {
                print_environments(&v);
            }
        }
        Command::Release(ReleaseCmd::Register(a)) => {
            let r = b.register_release(&new_release(a)?).await?;
            if out == Output::Json {
                print_json(&serde_json::to_value(&r)?);
            } else {
                println!("✓ release {} {} enregistrée ({})", r.service, r.version, r.id);
                println!("  {}", r.image_ref());
            }
        }
        Command::Release(ReleaseCmd::List { service, limit }) => {
            let list = b.releases(service.as_deref(), limit).await?;
            if out == Output::Json {
                print_json(&serde_json::to_value(&list)?);
            } else {
                let rows = list
                    .iter()
                    .map(|r| {
                        let ev = &r.evidence;
                        let vulns = ev
                            .vulnerabilities
                            .as_ref()
                            .map_or("—".into(), |v| format!("C{} H{}", v.critical, v.high));
                        vec![
                            r.service.clone(),
                            r.version.clone(),
                            r.short_digest().to_string(),
                            yes_no(ev.tests_passed),
                            vulns,
                            yes_no(ev.signature_verified),
                            socle_core::format_ts(&r.created_at)[..16].to_string(),
                        ]
                    })
                    .collect();
                table(&["SERVICE", "VERSION", "DIGEST", "TESTS", "VULNS", "SIGNÉE", "CRÉÉE"], rows);
            }
        }
        Command::Deploy(a) => {
            let req = request(&a.change, Action::Deploy { service: a.service, release: a.release });
            return change(b, req, &a.change, out).await;
        }
        Command::Promote(a) => {
            let envs = b.view("/environments").await?;
            let target = envs
                .as_array()
                .and_then(|l| l.iter().find(|e| e["name"] == a.change.env.as_str()))
                .ok_or_else(|| anyhow!("environnement {} inconnu", a.change.env))?;
            let source = a
                .from
                .clone()
                .or_else(|| target["promote_from"].as_str().map(str::to_string))
                .ok_or_else(|| anyhow!("aucun environnement source : précisez --from"))?;
            let release = envs
                .as_array()
                .and_then(|l| l.iter().find(|e| e["name"] == source.as_str()))
                .and_then(|e| e["services"].as_array())
                .and_then(|s| s.iter().find(|x| x["name"] == a.service.as_str()))
                .and_then(|s| s["release_id"].as_str())
                .ok_or_else(|| anyhow!("{} n'est pas déployé en {source}", a.service))?
                .to_string();
            if out == Output::Text {
                println!("Promotion de {} : {source} → {}", a.service, a.change.env);
            }
            let req = request(&a.change, Action::Deploy { service: a.service, release });
            return change(b, req, &a.change, out).await;
        }
        Command::Rollback(a) => {
            let req = request(&a.change, Action::Rollback { service: a.service, to_release: a.to });
            return change(b, req, &a.change, out).await;
        }
        Command::Scale(a) => {
            let req = request(&a.change, Action::Scale { service: a.service, min: a.min, max: a.max });
            return change(b, req, &a.change, out).await;
        }
        Command::Infra(InfraCmd::Plan { stack, change: c }) => {
            return change(b, request(&c, Action::InfraPlan { stack }), &c, out).await;
        }
        Command::Infra(InfraCmd::Apply { stack, plan_op, change: c }) => {
            return change(b, request(&c, Action::InfraApply { stack, plan_operation: plan_op }), &c, out).await;
        }
        Command::Backup(BackupCmd::Status) => {
            let v = b.view("/backups").await?;
            if out == Output::Json {
                print_json(&v);
            } else {
                print_backups(&v);
            }
        }
        Command::Backup(BackupCmd::Run { policy, change: c }) => {
            return change(b, request(&c, Action::Backup { policy }), &c, out).await;
        }
        Command::Backup(BackupCmd::Drill { policy, change: c }) => {
            return change(b, request(&c, Action::RestoreDrill { policy }), &c, out).await;
        }
        Command::Backup(BackupCmd::Restore { policy, snapshot, at, in_place, confirm, change: c }) => {
            if confirm != c.env {
                bail!("confirmation refusée : retapez exactement le nom de l'environnement (--confirm {})", c.env);
            }
            let point_in_time = at
                .map(|s| chrono::DateTime::parse_from_rfc3339(&s).map(|d| d.with_timezone(&chrono::Utc)))
                .transpose()
                .context("--at : horodatage RFC 3339 attendu")?;
            let req = request(&c, Action::Restore { policy, snapshot, point_in_time, in_place });
            return change(b, req, &c, out).await;
        }
        Command::Patch(a) => {
            return change(b, request(&a.change, Action::Patch { target: a.name }), &a.change, out).await;
        }
        Command::Runbook(a) => {
            return change(b, request(&a.change, Action::Runbook { runbook: a.name }), &a.change, out).await;
        }
        Command::Ops(OpsCmd::List { env, status, kind, limit }) => {
            let ops = b
                .operations(&OperationFilter { environment: env, status, kind, subject: None, limit: Some(limit) })
                .await?;
            if out == Output::Json {
                print_json(&serde_json::to_value(&ops)?);
            } else {
                print_operations(&ops);
            }
        }
        Command::Ops(OpsCmd::Show { id }) => {
            let op = b.operation(&id).await?;
            if out == Output::Json {
                print_json(&serde_json::to_value(&op)?);
            } else {
                print_operation(&op);
            }
        }
        Command::Ops(OpsCmd::Logs { id, follow }) => {
            let op = if follow {
                follow_operation(b, &id).await?
            } else {
                print_logs(b, &id, 0).await.map(|_| ())?;
                b.operation(&id).await?
            };
            if follow {
                return Ok(exit_for(&op));
            }
        }
        Command::Approve(a) => {
            let op = b.decide(&a.id, ApprovalDecision::Approved, &a.comment).await?;
            report_decision(&op, out)?;
        }
        Command::Reject(a) => {
            let op = b.decide(&a.id, ApprovalDecision::Rejected, &a.comment).await?;
            report_decision(&op, out)?;
        }
        Command::Cancel(a) => {
            let op = b.cancel(&a.id, &a.reason).await?;
            report_decision(&op, out)?;
        }
        Command::Incident(cmd) => incident(b, cmd, out).await?,
        Command::Maintenance(MaintenanceCmd::List { all }) => {
            let v = b.view(if all { "/maintenance" } else { "/maintenance?active=true" }).await?;
            if out == Output::Json {
                print_json(&v);
            } else {
                let rows = v
                    .as_array()
                    .map(|l| {
                        l.iter()
                            .map(|m| {
                                vec![
                                    s(&m["id"]),
                                    s(&m["environment"]),
                                    s(&m["status"]),
                                    s(&m["ends_at"]),
                                    s(&m["reason"]),
                                ]
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                table(&["ID", "ENV", "STATUT", "FIN", "MOTIF"], rows);
            }
        }
        Command::Maintenance(MaintenanceCmd::Start { env, duration, reason }) => {
            let v = b.start_maintenance(&env, duration, &reason).await?;
            if out == Output::Json {
                print_json(&v);
            } else {
                println!("✓ maintenance {} en {env} jusqu'à {}", s(&v["id"]), s(&v["ends_at"]));
            }
        }
        Command::Maintenance(MaintenanceCmd::End { id }) => {
            let v = b.end_maintenance(&id).await?;
            if out == Output::Json {
                print_json(&v);
            } else {
                println!("✓ maintenance {id} terminée");
            }
        }
        Command::Audit(AuditCmd::List { limit }) => {
            let events = b.audit_page(None, limit).await?;
            if out == Output::Json {
                print_json(&serde_json::to_value(&events)?);
            } else {
                let rows = events
                    .iter()
                    .map(|e| {
                        vec![
                            e.seq.to_string(),
                            socle_core::format_ts(&e.ts)[..19].to_string(),
                            e.actor.clone(),
                            e.action.clone(),
                            e.resource.clone(),
                            e.outcome.clone(),
                        ]
                    })
                    .collect();
                table(&["SEQ", "HORODATAGE", "ACTEUR", "ACTION", "RESSOURCE", "RÉSULTAT"], rows);
            }
        }
        Command::Audit(AuditCmd::Verify) => {
            let v = b.view("/audit/verify").await?;
            if out == Output::Json {
                print_json(&v);
            } else if v["valid"].as_bool() == Some(true) {
                println!(
                    "✓ chaîne d'audit intègre — {} événement(s), tête {}",
                    v["events_checked"],
                    s(&v["head_hash"])
                );
            } else {
                println!("✗ chaîne d'audit ROMPUE à la séquence {} : {}", v["broken_at"], s(&v["reason"]));
                return Ok(ExitCode::from(2));
            }
        }
        Command::Audit(AuditCmd::Export { out: file }) => {
            let mut lines = String::new();
            let mut after = 0;
            loop {
                let page = b.audit_page(Some(after), 5_000).await?;
                let Some(last) = page.last() else { break };
                after = last.seq;
                for e in &page {
                    lines.push_str(&serde_json::to_string(e)?);
                    lines.push('\n');
                }
            }
            match file {
                Some(path) => {
                    std::fs::write(&path, lines)?;
                    eprintln!("✓ journal d'audit exporté dans {}", path.display());
                }
                None => print!("{lines}"),
            }
        }
        Command::Compliance(a) => println!("{}", b.compliance(a.days, a.markdown).await?),
        Command::Token(TokenCmd::Create { name, subject, teams, ttl, .. }) => {
            let (token, secret) = b.create_token(&name, &subject, &teams, ttl).await?;
            print_token(out, &token, &secret);
        }
        Command::Token(TokenCmd::List) => {
            let v = b.view("/tokens").await?;
            if out == Output::Json {
                print_json(&v);
            } else {
                let rows = v
                    .as_array()
                    .map(|l| {
                        l.iter()
                            .map(|t| {
                                vec![
                                    s(&t["id"]),
                                    s(&t["name"]),
                                    s(&t["subject"]),
                                    t["teams"]
                                        .as_array()
                                        .map(|a| a.iter().map(s).collect::<Vec<_>>().join(","))
                                        .unwrap_or_default(),
                                    if t["revoked"].as_bool() == Some(true) {
                                        "révoqué".into()
                                    } else {
                                        "actif".into()
                                    },
                                    s(&t["expires_at"]),
                                ]
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                table(&["ID", "NOM", "SUJET", "ÉQUIPES", "ÉTAT", "EXPIRATION"], rows);
            }
        }
        Command::Token(TokenCmd::Revoke { id }) => {
            b.revoke_token(&id).await?;
            println!("✓ jeton {id} révoqué");
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// Soumission d'un changement (ou aperçu), puis exécution/attente éventuelle.
async fn change(b: &Backend, req: OperationRequest, c: &Change, out: Output) -> anyhow::Result<ExitCode> {
    if c.dry_run {
        let v = b.preview(&req).await?;
        if out == Output::Json {
            print_json(&v);
        } else {
            print_preview(&v);
        }
        return Ok(ExitCode::SUCCESS);
    }
    let op = b.submit(&req).await?;
    if out == Output::Text {
        print_operation(&op);
    }
    let final_op = match (b, op.status) {
        // Mode local : pas de worker, on exécute immédiatement.
        (Backend::Local { engine, .. }, OpStatus::Queued) => {
            let done = engine.run_now(&op.id).await?;
            if out == Output::Text {
                print_logs(b, &op.id, 0).await?;
                println!("\n{}", outcome_line(&done));
            }
            done
        }
        (Backend::Remote(_), OpStatus::Queued) if c.wait => follow_operation(b, &op.id).await?,
        _ => op,
    };
    if out == Output::Json {
        print_json(&serde_json::to_value(&final_op)?);
    }
    if final_op.status == OpStatus::AwaitingApproval && out == Output::Text {
        println!(
            "\n⏳ {} approbation(s) requise(s) : socle approve {}{}",
            final_op.required_approvals,
            final_op.id,
            if matches!(b, Backend::Local { .. }) { "  (approbations : utilisez le serveur)" } else { "" }
        );
    }
    let waited = c.wait || matches!(b, Backend::Local { .. });
    Ok(if waited || final_op.status == OpStatus::Rejected { exit_for(&final_op) } else { ExitCode::SUCCESS })
}

fn exit_for(op: &Operation) -> ExitCode {
    match op.status {
        OpStatus::Succeeded | OpStatus::AwaitingApproval | OpStatus::Queued => ExitCode::SUCCESS,
        _ => ExitCode::from(2),
    }
}

async fn print_logs(b: &Backend, id: &str, after: i64) -> anyhow::Result<i64> {
    let mut last = after;
    for l in b.logs(id, after).await? {
        last = l.seq;
        let ts = socle_core::format_ts(&l.ts);
        println!("{} {:<5} {} │ {}", &ts[11..19], l.level, l.step, l.message);
    }
    Ok(last)
}

async fn follow_operation(b: &Backend, id: &str) -> anyhow::Result<Operation> {
    let mut last = 0;
    loop {
        last = print_logs(b, id, last).await?;
        let op = b.operation(id).await?;
        if op.status.is_terminal() || op.status == OpStatus::AwaitingApproval {
            last = print_logs(b, id, last).await?;
            let _ = last;
            println!("\n{}", outcome_line(&op));
            return Ok(op);
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

async fn incident(b: &Backend, cmd: IncidentCmd, out: Output) -> anyhow::Result<()> {
    let one = |i: &Incident| -> anyhow::Result<()> {
        if out == Output::Json {
            print_json(&serde_json::to_value(i)?);
        } else {
            print_incident(i);
        }
        Ok(())
    };
    match cmd {
        IncidentCmd::List { all } => {
            let list = b.incidents(!all).await?;
            if out == Output::Json {
                print_json(&serde_json::to_value(&list)?);
            } else {
                let rows = list
                    .iter()
                    .map(|i| {
                        vec![
                            i.id.clone(),
                            i.severity.as_str().to_uppercase(),
                            i.status.as_str().into(),
                            i.environment.clone().unwrap_or_else(|| "—".into()),
                            i.title.clone(),
                            socle_core::format_ts(&i.created_at)[..16].to_string(),
                        ]
                    })
                    .collect();
                table(&["ID", "GRAVITÉ", "STATUT", "ENV", "TITRE", "OUVERT"], rows);
            }
        }
        IncidentCmd::Open { title, severity, env, services, summary } => {
            let severity = Severity::parse(&severity).ok_or_else(|| anyhow!("gravité inconnue (sev1 à sev4)"))?;
            one(&b.open_incident(&NewIncident { title, severity, environment: env, services, summary }).await?)?;
        }
        IncidentCmd::Show { id } => one(&b.incident(&id).await?)?,
        IncidentCmd::Ack(n) => one(&b.transition(&n.id, IncidentStatus::Acknowledged, n.note.as_deref()).await?)?,
        IncidentCmd::Mitigate(n) => one(&b.transition(&n.id, IncidentStatus::Mitigated, n.note.as_deref()).await?)?,
        IncidentCmd::Resolve(n) => one(&b.transition(&n.id, IncidentStatus::Resolved, n.note.as_deref()).await?)?,
        IncidentCmd::Close(n) => one(&b.transition(&n.id, IncidentStatus::Closed, n.note.as_deref()).await?)?,
        IncidentCmd::Reopen(n) => one(&b.transition(&n.id, IncidentStatus::Open, n.note.as_deref()).await?)?,
        IncidentCmd::Note { id, text } => one(&b.note(&id, &text).await?)?,
        IncidentCmd::Update { id, severity, root_cause, postmortem_url, commander } => {
            let severity = severity
                .map(|s| Severity::parse(&s).ok_or_else(|| anyhow!("gravité inconnue (sev1 à sev4)")))
                .transpose()?;
            let patch = IncidentPatch { severity, commander, root_cause, postmortem_url, ..IncidentPatch::default() };
            one(&b.update_incident(&id, &patch).await?)?;
        }
        IncidentCmd::Postmortem { id } => println!("{}", b.incident(&id).await?.render_postmortem()),
    }
    Ok(())
}

fn new_release(a: RegisterArgs) -> anyhow::Result<NewRelease> {
    let mut vulns = None;
    if let Some(path) = &a.trivy_report {
        let text = std::fs::read_to_string(path).with_context(|| format!("lecture de {}", path.display()))?;
        let v: Value = serde_json::from_str(&text).context("rapport Trivy JSON invalide")?;
        let mut summary = VulnSummary::default();
        for r in v["Results"].as_array().into_iter().flatten() {
            for vuln in r["Vulnerabilities"].as_array().into_iter().flatten() {
                match vuln["Severity"].as_str() {
                    Some("CRITICAL") => summary.critical += 1,
                    Some("HIGH") => summary.high += 1,
                    Some("MEDIUM") => summary.medium += 1,
                    Some("LOW") => summary.low += 1,
                    _ => {}
                }
            }
        }
        vulns = Some(summary);
    }
    if a.critical.is_some() || a.high.is_some() {
        let mut v = vulns.unwrap_or_default();
        if let Some(c) = a.critical {
            v.critical = c;
        }
        if let Some(h) = a.high {
            v.high = h;
        }
        vulns = Some(v);
    }
    Ok(NewRelease {
        service: a.service,
        version: a.version,
        digest: a.digest,
        image: a.image,
        git_sha: a.git_sha,
        evidence: Evidence {
            tests_passed: a.tests_passed,
            vulnerabilities: vulns,
            signature_verified: a.signed.then_some(true),
            sbom_uri: a.sbom,
            provenance_uri: a.provenance,
            iac_scan_passed: a.iac_scan_passed,
            secret_scan_passed: a.secret_scan_passed,
        },
    })
}

// ---------------------------------------------------------------------------
// Affichage
// ---------------------------------------------------------------------------

fn print_json(v: &Value) {
    println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
}

fn s(v: &Value) -> String {
    match v {
        Value::Null => "—".into(),
        Value::String(x) => x.clone(),
        other => other.to_string(),
    }
}

fn yes_no(v: Option<bool>) -> String {
    match v {
        Some(true) => "oui".into(),
        Some(false) => "NON".into(),
        None => "—".into(),
    }
}

fn table(headers: &[&str], rows: Vec<Vec<String>>) {
    if rows.is_empty() {
        println!("(aucun élément)");
        return;
    }
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for r in &rows {
        for (i, c) in r.iter().enumerate() {
            if let Some(w) = widths.get_mut(i) {
                *w = (*w).max(c.chars().count()).min(60);
            }
        }
    }
    let line = |cells: Vec<String>| {
        cells
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let w = widths.get(i).copied().unwrap_or(0);
                let c: String = c.chars().take(w).collect();
                format!("{c:<w$}")
            })
            .collect::<Vec<_>>()
            .join("  ")
    };
    println!("{}", line(headers.iter().map(|h| (*h).to_string()).collect()));
    for r in rows {
        println!("{}", line(r));
    }
}

fn status_icon(s: OpStatus) -> &'static str {
    match s {
        OpStatus::Succeeded => "✓",
        OpStatus::Failed => "✗",
        OpStatus::RolledBack => "↺",
        OpStatus::Rejected => "⛔",
        OpStatus::Cancelled => "∅",
        OpStatus::AwaitingApproval => "⏳",
        OpStatus::Queued => "…",
        OpStatus::Running => "▶",
    }
}

fn outcome_line(op: &Operation) -> String {
    let msg = op.outcome.as_ref().map(|o| o.message.clone()).unwrap_or_default();
    format!("{} {} — {}", status_icon(op.status), op.status.as_str(), msg)
}

fn print_operation(op: &Operation) {
    println!("{} opération {}", status_icon(op.status), op.id);
    println!("  {}", op.plan.summary);
    println!(
        "  statut : {} · demandée par {} · approbations {}/{}",
        op.status.as_str(),
        op.requested_by,
        op.approvals_granted(),
        op.required_approvals
    );
    if !op.gates.is_empty() {
        println!("  portes :");
        for g in &op.gates {
            let icon = match g.status {
                GateStatus::Passed => "✓",
                GateStatus::Warning => "!",
                GateStatus::Failed => "✗",
                GateStatus::Overridden => "⚠",
            };
            println!("    {icon} {:<15} {}", g.gate, g.message);
        }
    }
    println!("  plan ({}) :", &op.plan_hash[..12]);
    for l in op.plan.describe() {
        println!("    {l}");
    }
    if let Some(o) = &op.outcome {
        println!("  résultat : {}", o.message);
        if let Some(i) = &o.incident_id {
            println!("  incident ouvert : {i}");
        }
    }
}

fn print_preview(v: &Value) {
    println!("Aperçu — rien n'est soumis ni exécuté\n");
    println!("  {}", s(&v["plan"]["summary"]));
    println!(
        "  décision : {} · approbations requises : {}",
        s(&v["report"]["decision"]),
        v["report"]["required_approvals"]
    );
    for g in v["report"]["results"].as_array().into_iter().flatten() {
        let icon = match g["status"].as_str() {
            Some("passed") => "✓",
            Some("warning") => "!",
            Some("overridden") => "⚠",
            _ => "✗",
        };
        println!("    {icon} {:<15} {}", s(&g["gate"]), s(&g["message"]));
    }
    println!("  étapes :");
    for l in v["description"].as_array().into_iter().flatten() {
        println!("    {}", s(l));
    }
}

fn print_operations(ops: &[Operation]) {
    let rows = ops
        .iter()
        .map(|o| {
            vec![
                o.id.clone(),
                format!("{} {}", status_icon(o.status), o.status.as_str()),
                o.kind().as_str().into(),
                o.environment().into(),
                o.request.action.subject().into(),
                o.requested_by.clone(),
                socle_core::format_ts(&o.created_at)[..16].to_string(),
            ]
        })
        .collect();
    table(&["ID", "STATUT", "TYPE", "ENV", "OBJET", "DEMANDEUR", "CRÉÉE"], rows);
}

fn report_decision(op: &Operation, out: Output) -> anyhow::Result<()> {
    if out == Output::Json {
        print_json(&serde_json::to_value(op)?);
    } else {
        println!(
            "{} {} — approbations {}/{}",
            status_icon(op.status),
            op.status.as_str(),
            op.approvals_granted(),
            op.required_approvals
        );
    }
    Ok(())
}

fn print_incident(i: &Incident) {
    println!("[{}] {} — {}", i.severity.as_str().to_uppercase(), i.title, i.status.as_str());
    println!(
        "  id {} · env {} · responsable {}",
        i.id,
        i.environment.as_deref().unwrap_or("—"),
        i.commander.as_deref().unwrap_or("—")
    );
    for e in &i.timeline {
        println!("  {} {} : {}", &socle_core::format_ts(&e.at)[..16], e.author, e.message);
    }
}

fn print_token(out: Output, token: &Value, secret: &str) {
    if out == Output::Json {
        print_json(&json!({"token": token, "secret": secret}));
    } else {
        println!("✓ jeton {} créé pour {}", s(&token["name"]), s(&token["subject"]));
        println!("  secret (affiché une seule fois) : {secret}");
    }
}

fn print_environments(v: &Value) {
    for env in v.as_array().into_iter().flatten() {
        let mut flags = Vec::new();
        if env["change_window_open"].as_bool() == Some(false) {
            flags.push("hors fenêtre".to_string());
        }
        if !env["freeze"].is_null() {
            flags.push(format!("gel : {}", s(&env["freeze"])));
        }
        if !env["maintenance"].is_null() {
            flags.push("maintenance".to_string());
        }
        println!(
            "■ {} ({}, politique {}){}",
            s(&env["name"]),
            s(&env["tier"]),
            s(&env["policy"]),
            if flags.is_empty() { String::new() } else { format!(" — {}", flags.join(", ")) }
        );
        let rows = env["services"]
            .as_array()
            .map(|l| {
                l.iter()
                    .map(|x| {
                        vec![
                            s(&x["name"]),
                            s(&x["version"]),
                            format!("{}/{}", s(&x["target_kind"]), s(&x["target"])),
                            s(&x["provider"]),
                            s(&x["strategy"]),
                            if x["drift"].is_null() { "—".into() } else { "DÉRIVE".into() },
                        ]
                    })
                    .collect()
            })
            .unwrap_or_default();
        table(&["  SERVICE", "VERSION", "CIBLE", "FOURNISSEUR", "STRATÉGIE", "DÉRIVE"], rows);
        println!();
    }
}

fn print_backups(v: &Value) {
    let rows = v
        .as_array()
        .map(|l| {
            l.iter()
                .map(|b| {
                    let age = b["age_secs"].as_i64().map_or("jamais".into(), |a| format!("{} min", a / 60));
                    vec![
                        s(&b["policy"]),
                        s(&b["environment"]),
                        s(&b["source"]),
                        format!("{} min", b["rpo_secs"].as_u64().unwrap_or(0) / 60),
                        age,
                        if b["rpo_compliant"].as_bool() == Some(true) { "✓".into() } else { "✗".into() },
                        b["drill"]["last_duration_secs"].as_i64().map_or("—".into(), |d| format!("{d} s")),
                    ]
                })
                .collect()
        })
        .unwrap_or_default();
    table(&["POLITIQUE", "ENV", "SOURCE", "RPO", "DERNIÈRE", "RPO OK", "EXERCICE"], rows);
}

fn print_overview(v: &Value) {
    println!(
        "socle — {} (spécification {})\n",
        s(&v["platform"]),
        s(&v["spec_hash"]).chars().take(12).collect::<String>()
    );
    print_environments(&v["environments"]);
    let pending = v["awaiting_approval"].as_array().map_or(0, Vec::len);
    let running = v["running"].as_array().map_or(0, Vec::len);
    let incidents = v["open_incidents"].as_array().map_or(0, Vec::len);
    println!(
        "Approbations en attente : {pending} · opérations en cours : {running} · incidents actifs : {incidents}\n"
    );
    print_backups(&v["backups"]);
}
