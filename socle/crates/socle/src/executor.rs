//! Exécuteur générique des étapes de plan. Une seule implémentation pour
//! tous les types d'opérations : processus (sans shell, environnement
//! filtré, délai, liste blanche de programmes), mises à jour GitOps,
//! attente Argo CD, sondes HTTP, contrôles Prometheus, garde de plan
//! Terraform, silences Alertmanager.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use socle_core::operation::{EnvValue, ExecStep, FileEdit, Step, StepAction};
use socle_core::{kustomize, names, now_micros};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;

use crate::model::LogLine;
use crate::secrets::SecretResolver;
use crate::store::Store;

pub type StepData = BTreeMap<String, Value>;

/// Programmes autorisés par défaut (surchargeable : `SOCLE_ALLOWED_PROGRAMS`).
pub const DEFAULT_PROGRAMS: &[&str] = &[
    "git",
    "kubectl",
    "kustomize",
    "argocd",
    "helm",
    "ansible",
    "ansible-playbook",
    "terraform",
    "tofu",
    "velero",
    "restic",
    "pgbackrest",
    "aws",
    "az",
    "gcloud",
    "scw",
    "ovhcloud",
    "vault",
];

/// Préfixes de variables transmises aux processus enfants. Les variables
/// `SOCLE_*` (base de données, jetons…) ne sont jamais transmises.
pub const DEFAULT_ENV_PREFIXES: &[&str] = &[
    "AWS_",
    "AZURE_",
    "ARM_",
    "GOOGLE_",
    "CLOUDSDK_",
    "OS_",
    "SCW_",
    "OVH_",
    "VAULT_",
    "TF_",
    "ANSIBLE_",
    "KUBE",
    "RESTIC_",
    "PGBACKREST_",
    "GIT_",
    "SSH_",
    "LC_",
    "XDG_",
];
const DEFAULT_ENV_NAMES: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "LANG",
    "TZ",
    "TMPDIR",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "no_proxy",
    "SYSTEMROOT",
    "SystemRoot",
    "WINDIR",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
    "PATHEXT",
    "COMSPEC",
    "TEMP",
    "TMP",
    "ProgramData",
    "ProgramFiles",
];

const MAX_LOG_LINE: usize = 4_000;
const MAX_CAPTURE: usize = 64 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Journal d'opération
// ---------------------------------------------------------------------------

/// Journal d'exécution d'une opération : masquage des secrets, troncature,
/// écriture en base par lots.
pub struct OpLogger {
    store: Store,
    operation_id: String,
    seq: AtomicI64,
    buffer: tokio::sync::Mutex<Vec<LogLine>>,
    secrets: StdMutex<Vec<String>>,
}

impl OpLogger {
    pub async fn new(store: Store, operation_id: &str) -> crate::Result<Self> {
        let last = store.last_log_seq(operation_id).await?;
        Ok(Self {
            store,
            operation_id: operation_id.to_string(),
            seq: AtomicI64::new(last),
            buffer: tokio::sync::Mutex::new(Vec::new()),
            secrets: StdMutex::new(Vec::new()),
        })
    }

    pub fn register_secret(&self, value: &str) {
        if value.len() >= 4
            && let Ok(mut s) = self.secrets.lock()
        {
            s.push(value.to_string());
        }
    }

    pub fn redact(&self, text: &str) -> String {
        let mut out = text.to_string();
        if let Ok(secrets) = self.secrets.lock() {
            for s in secrets.iter() {
                out = out.replace(s.as_str(), "***");
            }
        }
        out
    }

    pub async fn log(&self, level: &str, step: &str, message: &str) {
        let mut message = self.redact(message);
        if message.len() > MAX_LOG_LINE {
            let mut cut = MAX_LOG_LINE;
            while !message.is_char_boundary(cut) {
                cut -= 1;
            }
            message.truncate(cut);
            message.push_str(" […]");
        }
        let line = LogLine {
            seq: self.seq.fetch_add(1, Ordering::SeqCst) + 1,
            ts: now_micros(),
            level: level.to_string(),
            step: step.to_string(),
            message,
        };
        tracing::debug!(operation = %self.operation_id, level, step, message = %line.message);
        let batch = {
            let mut buf = self.buffer.lock().await;
            buf.push(line);
            if buf.len() >= 25 { std::mem::take(&mut *buf) } else { Vec::new() }
        };
        self.write(batch).await;
    }

    pub async fn flush(&self) {
        let batch = std::mem::take(&mut *self.buffer.lock().await);
        self.write(batch).await;
    }

    async fn write(&self, mut batch: Vec<LogLine>) {
        if batch.is_empty() {
            return;
        }
        batch.sort_by_key(|l| l.seq);
        if let Err(e) = self.store.append_logs(&self.operation_id, &batch).await {
            tracing::warn!(operation = %self.operation_id, "écriture du journal impossible : {e}");
        }
    }
}

// ---------------------------------------------------------------------------
// Contrat d'exécution
// ---------------------------------------------------------------------------

pub struct StepContext<'a> {
    pub operation_id: &'a str,
    pub label: &'a str,
    pub logger: &'a OpLogger,
}

#[async_trait]
pub trait StepRunner: Send + Sync {
    async fn run(&self, step: &Step, ctx: &StepContext<'_>) -> Result<StepData, String>;
}

// ---------------------------------------------------------------------------
// Exécuteur réel
// ---------------------------------------------------------------------------

pub struct ProcessRunner {
    repo_root: PathBuf,
    allowed_programs: BTreeSet<String>,
    env_prefixes: Vec<String>,
    http: reqwest::Client,
    secrets: SecretResolver,
    git_lock: tokio::sync::Mutex<()>,
    poll_interval: Duration,
}

impl ProcessRunner {
    pub fn new(repo_root: PathBuf, allowed_programs: Option<Vec<String>>, extra_env_prefixes: Vec<String>) -> Self {
        let allowed = allowed_programs
            .unwrap_or_else(|| DEFAULT_PROGRAMS.iter().map(|s| (*s).to_string()).collect())
            .into_iter()
            .collect();
        let mut env_prefixes: Vec<String> = DEFAULT_ENV_PREFIXES.iter().map(|s| (*s).to_string()).collect();
        env_prefixes.extend(extra_env_prefixes);
        let http = reqwest::Client::builder()
            .user_agent(concat!("socle/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        Self {
            repo_root,
            allowed_programs: allowed,
            env_prefixes,
            http,
            secrets: SecretResolver,
            git_lock: tokio::sync::Mutex::new(()),
            poll_interval: Duration::from_secs(10),
        }
    }

    fn passes_env(&self, key: &str) -> bool {
        !key.starts_with("SOCLE_")
            && (DEFAULT_ENV_NAMES.contains(&key) || self.env_prefixes.iter().any(|p| key.starts_with(p.as_str())))
    }

    /// Chemin relatif sûr, résolu sous `base` (refuse toute évasion, liens
    /// symboliques compris).
    fn safe_path(&self, base: &Path, rel: &str) -> Result<PathBuf, String> {
        if !names::is_safe_relative_path(rel) {
            return Err(format!("chemin refusé : {rel}"));
        }
        let path = base.join(rel);
        let root = self.repo_root.canonicalize().map_err(|e| format!("racine du dépôt inaccessible : {e}"))?;
        let probe = if path.exists() { path.clone() } else { path.parent().map(Path::to_path_buf).unwrap_or_default() };
        if probe.exists() {
            let canon = probe.canonicalize().map_err(|e| format!("{rel} : {e}"))?;
            if !canon.starts_with(&root) {
                return Err(format!("chemin hors du dépôt refusé : {rel}"));
            }
        }
        Ok(path)
    }

    fn command(&self, program: &str, args: &[String], cwd: &Path) -> Result<Command, String> {
        if !self.allowed_programs.contains(program) {
            return Err(format!("programme non autorisé : {program} (voir SOCLE_ALLOWED_PROGRAMS)"));
        }
        let mut cmd = Command::new(program);
        cmd.args(args)
            .current_dir(cwd)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        for (k, v) in std::env::vars() {
            if self.passes_env(&k) {
                cmd.env(k, v);
            }
        }
        Ok(cmd)
    }

    /// Exécute une commande en capturant sa sortie standard (non journalisée).
    async fn capture(
        &self,
        program: &str,
        args: &[String],
        cwd: &Path,
        timeout: Duration,
    ) -> Result<(i32, String, String), String> {
        let mut cmd = self.command(program, args, cwd)?;
        let mut child = cmd.spawn().map_err(|e| format!("lancement de {program} impossible : {e}"))?;
        let mut out = child.stdout.take();
        let mut err = child.stderr.take();
        let fut = async {
            let mut o = Vec::new();
            let mut e = Vec::new();
            let read_o = async {
                if let Some(s) = out.as_mut() {
                    let _ = s.take(MAX_CAPTURE as u64).read_to_end(&mut o).await;
                }
            };
            let read_e = async {
                if let Some(s) = err.as_mut() {
                    let _ = s.take(1024 * 1024).read_to_end(&mut e).await;
                }
            };
            let (_, _, status) = tokio::join!(read_o, read_e, child.wait());
            (status, o, e)
        };
        match tokio::time::timeout(timeout, fut).await {
            Ok((Ok(status), o, e)) => Ok((
                status.code().unwrap_or(-1),
                String::from_utf8_lossy(&o).into_owned(),
                String::from_utf8_lossy(&e).into_owned(),
            )),
            Ok((Err(e), _, _)) => Err(format!("{program} : {e}")),
            Err(_) => {
                let _ = child.kill().await;
                Err(format!("{program} : délai dépassé ({}s)", timeout.as_secs()))
            }
        }
    }

    async fn run_exec(&self, e: &ExecStep, ctx: &StepContext<'_>) -> Result<StepData, String> {
        let cwd = match &e.cwd {
            Some(c) => self.safe_path(&self.repo_root, c)?,
            None => self.repo_root.clone(),
        };
        let mut cmd = self.command(&e.program, &e.args, &cwd)?;
        cmd.env("SOCLE_OPERATION", ctx.operation_id);
        for (k, v) in &e.env {
            let value = match v {
                EnvValue::Literal(s) => s.clone(),
                EnvValue::Secret { secret } => {
                    let s = self.secrets.resolve(secret).await?;
                    ctx.logger.register_secret(&s);
                    s
                }
            };
            cmd.env(k, value);
        }
        let mut child = cmd.spawn().map_err(|err| format!("lancement de {} impossible : {err}", e.program))?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let pump = |stream: Option<Box<dyn tokio::io::AsyncRead + Unpin + Send>>, level: &'static str| async move {
            if let Some(s) = stream {
                let mut lines = BufReader::new(s).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    ctx.logger.log(level, ctx.label, &line).await;
                }
            }
        };
        let out_stream: Option<Box<dyn tokio::io::AsyncRead + Unpin + Send>> =
            stdout.map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>);
        let err_stream: Option<Box<dyn tokio::io::AsyncRead + Unpin + Send>> =
            stderr.map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>);
        let status = {
            let fut = async { tokio::join!(pump(out_stream, "out"), pump(err_stream, "err"), child.wait()).2 };
            tokio::time::timeout(Duration::from_secs(e.timeout_secs.max(1)), fut).await
        };
        let status = match status {
            Ok(Ok(s)) => s,
            Ok(Err(err)) => return Err(format!("{} : {err}", e.program)),
            Err(_) => {
                let _ = child.kill().await;
                return Err(format!("{} : délai dépassé ({}s), processus arrêté", e.program, e.timeout_secs));
            }
        };
        let ok_codes = if e.success_codes.is_empty() { vec![0] } else { e.success_codes.clone() };
        match status.code() {
            Some(c) if ok_codes.contains(&c) => Ok(BTreeMap::from([("exit_code".into(), json!(c))])),
            Some(c) => Err(format!("{} a échoué (code de sortie {c})", e.program)),
            None => Err(format!("{} interrompu par un signal", e.program)),
        }
    }

    async fn git(&self, repo: &Path, args: &[&str], ctx: &StepContext<'_>) -> Result<String, String> {
        let args: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();
        let (code, out, err) = self.capture("git", &args, repo, Duration::from_secs(300)).await?;
        for line in err.lines().filter(|l| !l.trim().is_empty()) {
            ctx.logger.log("git", ctx.label, line).await;
        }
        if code != 0 {
            return Err(format!("git {} a échoué (code {code})", args.first().map_or("", String::as_str)));
        }
        Ok(out)
    }

    async fn run_gitops(
        &self,
        repo: &str,
        branch: &str,
        edits: &[FileEdit],
        message: &str,
        push: bool,
        ctx: &StepContext<'_>,
    ) -> Result<StepData, String> {
        let _guard = self.git_lock.lock().await;
        let repo_dir = self.safe_path(&self.repo_root, repo)?;
        let files: Vec<&str> = edits
            .iter()
            .map(|e| match e {
                FileEdit::SetImage { file, .. } | FileEdit::Write { file, .. } => file.as_str(),
            })
            .collect();

        let mut status_args = vec!["status", "--porcelain", "--"];
        status_args.extend(files.iter().copied());
        let dirty = self.git(&repo_dir, &status_args, ctx).await?;
        if !dirty.trim().is_empty() {
            return Err(format!("modifications locales non commitées sur {} : refus de les écraser", files.join(", ")));
        }
        if push {
            self.git(&repo_dir, &["pull", "--ff-only", "origin", branch], ctx).await?;
        }

        let mut changed = false;
        for edit in edits {
            match edit {
                FileEdit::SetImage { file, image, new_name, digest } => {
                    let path = self.safe_path(&repo_dir, file)?;
                    let content = tokio::fs::read_to_string(&path)
                        .await
                        .map_err(|e| format!("lecture de {file} impossible : {e}"))?;
                    let updated = kustomize::set_image(&content, image, new_name, digest)?;
                    if updated != content {
                        tokio::fs::write(&path, updated).await.map_err(|e| format!("écriture de {file} : {e}"))?;
                        changed = true;
                    }
                }
                FileEdit::Write { file, content } => {
                    let path = self.safe_path(&repo_dir, file)?;
                    let dir = path.parent().ok_or("chemin sans dossier parent")?;
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
                    let kustomization =
                        tokio::fs::read_to_string(dir.join("kustomization.yaml")).await.unwrap_or_default();
                    if !kustomization.contains(name) {
                        return Err(format!(
                            "{file} n'est référencé par aucun kustomization.yaml du dossier : ajoutez-le aux patches de l'overlay"
                        ));
                    }
                    let old = tokio::fs::read_to_string(&path).await.unwrap_or_default();
                    if &old != content {
                        tokio::fs::write(&path, content).await.map_err(|e| format!("écriture de {file} : {e}"))?;
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            ctx.logger.log("info", ctx.label, "dépôt GitOps déjà à jour : aucun commit nécessaire").await;
            return Ok(BTreeMap::from([("gitops_changed".into(), json!(false))]));
        }

        let mut add = vec!["add", "--"];
        add.extend(files.iter().copied());
        self.git(&repo_dir, &add, ctx).await?;
        let mut commit = vec![
            "-c",
            "user.name=socle",
            "-c",
            "user.email=socle@socle.invalid",
            "commit",
            "--quiet",
            "-m",
            message,
            "--",
        ];
        commit.extend(files.iter().copied());
        self.git(&repo_dir, &commit, ctx).await?;
        if push {
            let refspec = format!("HEAD:{branch}");
            let mut attempt = 0;
            loop {
                attempt += 1;
                match self.git(&repo_dir, &["push", "origin", &refspec], ctx).await {
                    Ok(_) => break,
                    Err(e) if attempt < 3 => {
                        ctx.logger
                            .log("warn", ctx.label, &format!("push refusé ({e}) : rebase puis nouvel essai"))
                            .await;
                        self.git(&repo_dir, &["pull", "--rebase", "origin", branch], ctx).await?;
                    }
                    Err(e) => return Err(e),
                }
            }
        }
        let sha = self.git(&repo_dir, &["rev-parse", "HEAD"], ctx).await?.trim().to_string();
        ctx.logger.log("info", ctx.label, &format!("commit GitOps {sha}")).await;
        Ok(BTreeMap::from([("gitops_changed".into(), json!(true)), ("gitops_commit".into(), json!(sha))]))
    }

    async fn wait_argo(
        &self,
        context: &str,
        namespace: &str,
        application: &str,
        image_ref: Option<&str>,
        timeout_secs: u64,
        ctx: &StepContext<'_>,
    ) -> Result<StepData, String> {
        let base = vec![format!("--context={context}"), format!("--namespace={namespace}")];
        let resource = format!("applications.argoproj.io/{application}");
        let mut refresh = base.clone();
        refresh.extend([
            "annotate".into(),
            resource.clone(),
            "argocd.argoproj.io/refresh=normal".into(),
            "--overwrite".into(),
        ]);
        let _ = self.capture("kubectl", &refresh, &self.repo_root, Duration::from_secs(60)).await;

        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        let mut last_state = String::new();
        let mut degraded = 0;
        let mut get = base;
        get.extend(["get".into(), resource, "--output=json".into()]);
        loop {
            match self.capture("kubectl", &get, &self.repo_root, Duration::from_secs(60)).await {
                Ok((0, out, _)) => {
                    let v: Value = serde_json::from_str(&out).unwrap_or(Value::Null);
                    let sync = v.pointer("/status/sync/status").and_then(Value::as_str).unwrap_or("?");
                    let health = v.pointer("/status/health/status").and_then(Value::as_str).unwrap_or("?");
                    let images: Vec<&str> = v
                        .pointer("/status/summary/images")
                        .and_then(Value::as_array)
                        .map(|a| a.iter().filter_map(Value::as_str).collect())
                        .unwrap_or_default();
                    let image_ok = image_ref.is_none_or(|r| images.contains(&r));
                    let state = format!(
                        "sync={sync} santé={health}{}",
                        if image_ref.is_some() {
                            if image_ok { " image=à jour" } else { " image=en attente" }
                        } else {
                            ""
                        }
                    );
                    if state != last_state {
                        ctx.logger.log("info", ctx.label, &state).await;
                        last_state = state;
                    }
                    if sync == "Synced" && health == "Healthy" && image_ok {
                        let revision = v.pointer("/status/sync/revision").cloned().unwrap_or(Value::Null);
                        return Ok(BTreeMap::from([("argo_revision".into(), revision)]));
                    }
                    if image_ok && health == "Degraded" {
                        degraded += 1;
                        if degraded >= 3 {
                            return Err(format!("application {application} dégradée selon Argo CD"));
                        }
                    } else {
                        degraded = 0;
                    }
                }
                Ok((code, _, err)) => {
                    let msg = format!("kubectl get (code {code}) : {}", err.lines().next().unwrap_or(""));
                    ctx.logger.log("warn", ctx.label, &msg).await;
                }
                Err(e) => ctx.logger.log("warn", ctx.label, &e).await,
            }
            if Instant::now() >= deadline {
                return Err(format!("délai de convergence dépassé ({timeout_secs}s), dernier état : {last_state}"));
            }
            tokio::time::sleep(self.poll_interval).await;
        }
    }

    async fn http_check(
        &self,
        url: &str,
        expect: u16,
        retries: u32,
        interval: u64,
        timeout: u64,
        ctx: &StepContext<'_>,
    ) -> Result<StepData, String> {
        let retries = retries.max(1);
        for attempt in 1..=retries {
            match self.http.get(url).timeout(Duration::from_secs(timeout.max(1))).send().await {
                Ok(resp) if resp.status().as_u16() == expect => {
                    ctx.logger.log("info", ctx.label, &format!("{url} → {expect} (essai {attempt})")).await;
                    return Ok(BTreeMap::from([("http_status".into(), json!(expect))]));
                }
                Ok(resp) => {
                    let msg =
                        format!("{url} → {} (attendu {expect}, essai {attempt}/{retries})", resp.status().as_u16());
                    ctx.logger.log("warn", ctx.label, &msg).await;
                }
                Err(e) => {
                    let msg = format!("{url} injoignable (essai {attempt}/{retries}) : {e}");
                    ctx.logger.log("warn", ctx.label, &msg).await;
                }
            }
            if attempt < retries {
                tokio::time::sleep(Duration::from_secs(interval)).await;
            }
        }
        Err(format!("{url} n'a pas répondu {expect} après {retries} essais"))
    }

    async fn prom_value(&self, base: &str, query: &str) -> Result<f64, String> {
        let url = format!("{}/api/v1/query", base.trim_end_matches('/'));
        let v: Value = self
            .http
            .get(&url)
            .query(&[("query", query)])
            .send()
            .await
            .map_err(|e| format!("Prometheus injoignable : {e}"))?
            .json()
            .await
            .map_err(|e| format!("réponse Prometheus illisible : {e}"))?;
        if v.get("status").and_then(Value::as_str) != Some("success") {
            return Err(format!("requête PromQL refusée : {}", v.get("error").and_then(Value::as_str).unwrap_or("?")));
        }
        let parse = |x: &Value| x.get(1).and_then(Value::as_str).and_then(|s| s.parse::<f64>().ok());
        let data = v.get("data").cloned().unwrap_or(Value::Null);
        let value = match data.get("resultType").and_then(Value::as_str) {
            Some("scalar") => data.get("result").and_then(parse).unwrap_or(0.0),
            _ => data
                .get("result")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|r| r.get("value").and_then(parse)).fold(0.0_f64, f64::max))
                .unwrap_or(0.0),
        };
        Ok(if value.is_nan() { 0.0 } else { value })
    }

    async fn tf_plan_guard(
        &self,
        program: &str,
        chdir: &str,
        planfile: &str,
        expected: Option<&str>,
        ctx: &StepContext<'_>,
    ) -> Result<StepData, String> {
        if !names::is_safe_relative_path(chdir) || !names::is_safe_relative_path(planfile) {
            return Err("chemin de plan refusé".into());
        }
        let args =
            vec![format!("-chdir={chdir}"), "show".into(), "-json".into(), "-no-color".into(), planfile.to_string()];
        let (code, out, err) = self.capture(program, &args, &self.repo_root, Duration::from_secs(600)).await?;
        if code != 0 {
            return Err(format!("{program} show a échoué : {}", err.lines().last().unwrap_or("")));
        }
        let v: Value = serde_json::from_str(&out).map_err(|e| format!("plan JSON illisible : {e}"))?;
        let (digest, lines) = plan_digest(&v);
        for l in lines.iter().take(200) {
            ctx.logger.log("plan", ctx.label, l).await;
        }
        ctx.logger.log("info", ctx.label, &format!("{} changement(s), empreinte {}", lines.len(), &digest[..12])).await;
        if let Some(exp) = expected
            && exp != digest
        {
            return Err(format!(
                "le plan actuel diffère du plan approuvé ({} ≠ {}) : relancez `socle infra plan` et faites approuver le nouveau plan",
                &digest[..12],
                &exp[..exp.len().min(12)]
            ));
        }
        Ok(BTreeMap::from([
            ("tf_plan_summary".into(), json!(digest)),
            ("tf_changes".into(), json!(lines.len())),
            ("tf_changes_list".into(), json!(lines.iter().take(200).collect::<Vec<_>>())),
        ]))
    }

    async fn silence(
        &self,
        url: &str,
        matchers: &BTreeMap<String, String>,
        duration_secs: u64,
        comment: &str,
        ctx: &StepContext<'_>,
    ) -> Result<StepData, String> {
        let id = create_silence(&self.http, url, matchers, duration_secs, comment).await?;
        ctx.logger.log("info", ctx.label, &format!("silence Alertmanager {id}")).await;
        Ok(BTreeMap::from([("silence_id".into(), json!(id))]))
    }
}

/// Empreinte canonique d'un plan Terraform/OpenTofu (`show -json`) : seules
/// les ressources réellement modifiées sont prises en compte.
pub fn plan_digest(plan: &Value) -> (String, Vec<String>) {
    let mut changes: Vec<(String, Value)> = plan
        .get("resource_changes")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|rc| {
                    let change = rc.get("change")?;
                    let actions = change.get("actions")?.clone();
                    if actions == json!(["no-op"]) || actions == json!(["read"]) {
                        return None;
                    }
                    let address = rc.get("address")?.as_str()?.to_string();
                    Some((
                        address,
                        json!({
                            "actions": actions,
                            "before": change.get("before"),
                            "after": change.get("after"),
                            "after_unknown": change.get("after_unknown"),
                        }),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    changes.sort_by(|a, b| a.0.cmp(&b.0));
    let digest = hex::encode(Sha256::digest(serde_json::to_vec(&changes).unwrap_or_default()));
    let lines = changes
        .iter()
        .map(|(addr, c)| {
            let actions: Vec<&str> =
                c["actions"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
            let symbol = match actions.as_slice() {
                ["create"] => "+",
                ["delete"] => "-",
                ["update"] => "~",
                _ => "±",
            };
            format!("{symbol} {addr} ({})", actions.join("/"))
        })
        .collect();
    (digest, lines)
}

pub async fn create_silence(
    http: &reqwest::Client,
    url: &str,
    matchers: &BTreeMap<String, String>,
    duration_secs: u64,
    comment: &str,
) -> Result<String, String> {
    let now = chrono::Utc::now();
    let end = now + chrono::TimeDelta::seconds(i64::try_from(duration_secs).unwrap_or(7_200));
    let body = json!({
        "matchers": matchers.iter().map(|(k, v)| json!({"name": k, "value": v, "isRegex": false, "isEqual": true})).collect::<Vec<_>>(),
        "startsAt": now.to_rfc3339(),
        "endsAt": end.to_rfc3339(),
        "createdBy": "socle",
        "comment": comment,
    });
    let v: Value = http
        .post(format!("{}/api/v2/silences", url.trim_end_matches('/')))
        .json(&body)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| format!("création du silence impossible : {e}"))?
        .json()
        .await
        .map_err(|e| format!("réponse Alertmanager illisible : {e}"))?;
    v.get("silenceID")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "Alertmanager n'a pas renvoyé d'identifiant de silence".into())
}

pub async fn expire_silence(http: &reqwest::Client, url: &str, id: &str) -> Result<(), String> {
    http.delete(format!("{}/api/v2/silence/{id}", url.trim_end_matches('/')))
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map(|_| ())
        .map_err(|e| format!("expiration du silence {id} impossible : {e}"))
}

#[async_trait]
impl StepRunner for ProcessRunner {
    async fn run(&self, step: &Step, ctx: &StepContext<'_>) -> Result<StepData, String> {
        match &step.action {
            StepAction::Exec(e) => self.run_exec(e, ctx).await,
            StepAction::GitopsUpdate { repo, branch, edits, message, push } => {
                self.run_gitops(repo, branch, edits, message, *push, ctx).await
            }
            StepAction::WaitArgo { context, namespace, application, image_ref, timeout_secs } => {
                self.wait_argo(context, namespace, application, image_ref.as_deref(), *timeout_secs, ctx).await
            }
            StepAction::HttpCheck { url, expect_status, retries, interval_secs, timeout_secs } => {
                self.http_check(url, *expect_status, *retries, *interval_secs, *timeout_secs, ctx).await
            }
            StepAction::PromCheck { prometheus_url, query, max, samples, interval_secs } => {
                let samples = (*samples).max(1);
                let mut last = 0.0;
                for i in 0..samples {
                    last = self.prom_value(prometheus_url, query).await?;
                    ctx.logger
                        .log("info", ctx.label, &format!("mesure {}/{samples} : {last} (seuil {max})", i + 1))
                        .await;
                    if last > *max {
                        return Err(format!("indicateur au-dessus du seuil : {last} > {max}"));
                    }
                    if i + 1 < samples {
                        tokio::time::sleep(Duration::from_secs(*interval_secs)).await;
                    }
                }
                Ok(BTreeMap::from([("prom_last".into(), json!(last))]))
            }
            StepAction::TfPlanGuard { program, chdir, planfile, expected } => {
                self.tf_plan_guard(program, chdir, planfile, expected.as_deref(), ctx).await
            }
            StepAction::Silence { alertmanager_url, matchers, duration_secs, comment } => {
                self.silence(alertmanager_url, matchers, *duration_secs, comment, ctx).await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_digest_ignores_noop_and_is_order_independent() {
        let a = json!({"resource_changes": [
            {"address": "aws_s3_bucket.b", "change": {"actions": ["update"], "before": {"x": 1}, "after": {"x": 2}}},
            {"address": "aws_vpc.main", "change": {"actions": ["no-op"], "before": {}, "after": {}}},
            {"address": "aws_instance.a", "change": {"actions": ["create"], "before": null, "after": {"t": "m5"}}}
        ]});
        let b = json!({"resource_changes": [
            {"address": "aws_instance.a", "change": {"actions": ["create"], "before": null, "after": {"t": "m5"}}},
            {"address": "aws_s3_bucket.b", "change": {"actions": ["update"], "before": {"x": 1}, "after": {"x": 2}}}
        ]});
        let (da, la) = plan_digest(&a);
        let (db, _) = plan_digest(&b);
        assert_eq!(da, db);
        assert_eq!(la, vec!["+ aws_instance.a (create)", "~ aws_s3_bucket.b (update)"]);
        let c = json!({"resource_changes": [
            {"address": "aws_instance.a", "change": {"actions": ["create"], "before": null, "after": {"t": "m6"}}},
            {"address": "aws_s3_bucket.b", "change": {"actions": ["update"], "before": {"x": 1}, "after": {"x": 2}}}
        ]});
        assert_ne!(plan_digest(&c).0, da, "une valeur différente change l'empreinte");
    }

    #[test]
    fn environment_filter_never_leaks_socle_variables() {
        let r = ProcessRunner::new(PathBuf::from("."), None, vec![]);
        assert!(r.passes_env("PATH"));
        assert!(r.passes_env("AWS_REGION"));
        assert!(r.passes_env("KUBECONFIG"));
        assert!(!r.passes_env("SOCLE_DATABASE_URL"));
        assert!(!r.passes_env("DATABASE_PASSWORD"));
    }
}
