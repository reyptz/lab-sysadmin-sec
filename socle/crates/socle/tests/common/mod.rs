//! Outillage partagé des tests d'intégration : spécification, horloge
//! pilotable et exécuteur simulé.

#![allow(dead_code, clippy::unwrap_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use socle::engine::{Engine, EngineConfig};
use socle::executor::{StepContext, StepData, StepRunner};
use socle::store::Store;
use socle_core::operation::{Step, StepAction};
use socle_core::rbac::{Principal, PrincipalKind};
use socle_core::release::{Evidence, NewRelease, VulnSummary};
use socle_core::spec::Spec;

pub const SPEC: &str = r#"
version: 1
platform:
  name: lab-test
  owner_team: platform
  gitops_push: false
providers:
  aws-eu: { kind: aws, region: eu-west-3 }
  dc-paris: { kind: onprem }
targets:
  eks-dev: { kind: kubernetes, provider: aws-eu, context: eks-dev }
  eks-prod: { kind: kubernetes, provider: aws-eu, context: eks-prod }
  rhel-prod: { kind: vm, provider: dc-paris, inventory: inventory/prod.yml, group: app_servers,
               patching: { batch: "1" } }
environments:
  dev: { tier: development, policy: relaxed }
  prod:
    tier: production
    policy: strict
    promote_from: dev
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
    gitops: { path: "apps/erp/{env}", application: "erp-{env}" }
    health: { path: /health }
    environments:
      dev: { target: eks-dev, url: "https://erp.dev.example.org" }
      prod: { target: eks-prod, url: "https://erp.example.org", replicas: { min: 2, max: 10 } }
backups:
  erp-files:
    environment: prod
    source: { kind: files, target: rhel-prod, paths: [/etc], repository: "s3:s3.example.org/b" }
    every: 15m
    rpo: 30m
    rto: 1h
    retention: { daily: 7 }
    drill: { every: 7d }
teams:
  platform: { members: [root@example.org] }
  erp: { idp_groups: [erp-devs] }
  sre: { idp_groups: [sre] }
  audit: {}
access:
  - { team: platform, role: admin }
  - { team: sre, role: approver, environments: [prod] }
  - { team: sre, role: operator }
  - { team: erp, role: deployer, environments: [dev, prod], services: [erp-app] }
  - { team: audit, role: viewer }
"#;

/// Horloge pilotable (millisecondes UTC).
#[derive(Clone)]
pub struct TestClock(pub Arc<AtomicI64>);

impl TestClock {
    /// Mercredi 25/11/2026 10:00 à Paris : fenêtre de changement ouverte.
    pub fn new() -> Self {
        let t = Utc.with_ymd_and_hms(2026, 11, 25, 9, 0, 0).unwrap();
        Self(Arc::new(AtomicI64::new(t.timestamp_millis())))
    }
    pub fn set(&self, t: DateTime<Utc>) {
        self.0.store(t.timestamp_millis(), Ordering::SeqCst);
    }
    pub fn advance(&self, secs: i64) {
        self.0.fetch_add(secs * 1000, Ordering::SeqCst);
    }
    pub fn now(&self) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(self.0.load(Ordering::SeqCst)).unwrap()
    }
}

/// Exécuteur simulé : journalise les étapes et échoue sur demande.
#[derive(Default)]
pub struct FakeRunner {
    pub executed: Mutex<Vec<String>>,
    pub fail_on: Mutex<Option<String>>,
}

#[async_trait]
impl StepRunner for FakeRunner {
    async fn run(&self, step: &Step, ctx: &StepContext<'_>) -> Result<StepData, String> {
        self.executed.lock().unwrap().push(step.name.clone());
        ctx.logger.log("info", ctx.label, &format!("simulé : {}", step.action.describe())).await;
        if let Some(pattern) = self.fail_on.lock().unwrap().as_deref()
            && step.name.contains(pattern)
        {
            return Err(format!("échec simulé sur « {} »", step.name));
        }
        let mut data = BTreeMap::new();
        if let StepAction::TfPlanGuard { .. } = step.action {
            data.insert("tf_plan_summary".into(), serde_json::json!("f".repeat(64)));
            data.insert("tf_changes".into(), serde_json::json!(3));
        }
        Ok(data)
    }
}

pub struct Harness {
    pub engine: Arc<Engine>,
    pub runner: Arc<FakeRunner>,
    pub clock: TestClock,
    _dir: tempfile::TempDir,
}

/// Base de test : SQLite (défaut) ou PostgreSQL si `SOCLE_TEST_POSTGRES_URL`
/// est défini (CI) — chaque test reçoit alors son propre schéma isolé.
pub async fn test_store(dir: &std::path::Path) -> Store {
    match std::env::var("SOCLE_TEST_POSTGRES_URL") {
        Ok(base) if !base.is_empty() => {
            let schema = format!("t_{}", uuid::Uuid::new_v4().simple());
            sqlx::any::install_default_drivers();
            let admin = sqlx::AnyPool::connect(&base).await.unwrap();
            sqlx::query(&format!("CREATE SCHEMA {schema}")).execute(&admin).await.unwrap();
            admin.close().await;
            let sep = if base.contains('?') { '&' } else { '?' };
            Store::connect(&format!("{base}{sep}options=-c%20search_path%3D{schema}")).await.unwrap()
        }
        _ => {
            let db = dir.join("socle.db").to_string_lossy().replace('\\', "/");
            Store::connect(&format!("sqlite:{db}?mode=rwc")).await.unwrap()
        }
    }
}

pub async fn harness() -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let store = test_store(dir.path()).await;
    let runner = Arc::new(FakeRunner::default());
    let clock = TestClock::new();
    let c = clock.clone();
    let engine = Engine::new(
        Spec::from_yaml(SPEC).unwrap(),
        None,
        store,
        runner.clone(),
        EngineConfig::new(dir.path().to_path_buf()),
    )
    .with_clock(Arc::new(move || c.now()));
    Harness { engine: Arc::new(engine), runner, clock, _dir: dir }
}

pub fn principal(id: &str, teams: &[&str]) -> Principal {
    Principal {
        id: id.into(),
        display_name: id.into(),
        kind: PrincipalKind::User,
        teams: teams.iter().map(|t| (*t).to_string()).collect::<BTreeSet<_>>(),
    }
}

pub fn digest(fill: char) -> String {
    format!("sha256:{}", fill.to_string().repeat(64))
}

pub fn good_release(version: &str, fill: char) -> NewRelease {
    NewRelease {
        service: "erp-app".into(),
        version: version.into(),
        digest: digest(fill),
        image: None,
        git_sha: Some("abcdef1".into()),
        evidence: Evidence {
            tests_passed: Some(true),
            vulnerabilities: Some(VulnSummary::default()),
            signature_verified: Some(true),
            sbom_uri: Some("oci://sbom".into()),
            ..Evidence::default()
        },
    }
}
