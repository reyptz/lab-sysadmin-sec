use chrono::Utc;

use super::*;
use crate::operation::{FileEdit, StepAction};
use crate::release::{Evidence, Release};
use crate::spec::tests::sample;

fn release(service: &str, version: &str, fill: char) -> Release {
    Release {
        id: format!("rel-{version}"),
        service: service.into(),
        version: version.into(),
        image: format!("ghcr.io/acme/{service}"),
        digest: format!("sha256:{}", fill.to_string().repeat(64)),
        git_sha: None,
        created_by: "ci".into(),
        created_at: Utc::now(),
        evidence: Evidence::default(),
    }
}

fn request(env: &str, action: Action) -> OperationRequest {
    OperationRequest {
        environment: env.into(),
        action,
        reason: String::new(),
        change_ticket: Some("CHG-1".into()),
        emergency: false,
    }
}

fn ctx<'a>(
    spec: &'a Spec,
    req: &'a OperationRequest,
    release: Option<&'a Release>,
    current: Option<&'a Release>,
) -> PlanContext<'a> {
    PlanContext {
        spec,
        request: req,
        operation_id: "0193a0b4-7c1e-7d2a-9f00-1234567890ab",
        actor: "alice",
        now: Utc::now(),
        release,
        current,
        infra_plan_summary: None,
        snapshot: None,
    }
}

#[test]
fn kubernetes_deploy_with_verification_and_rollback() {
    let spec = sample();
    let new = release("erp-app", "1.3.0", 'c');
    let cur = release("erp-app", "1.2.0", 'b');
    let req = request("prod", Action::Deploy { service: "erp-app".into(), release: "1.3.0".into() });
    let p = plan(&ctx(&spec, &req, Some(&new), Some(&cur))).expect("plan");

    assert_eq!(p.steps.len(), 2, "{:#?}", p.steps);
    let StepAction::GitopsUpdate { edits, message, push, .. } = &p.steps[0].action else {
        panic!("étape GitOps attendue");
    };
    assert!(!push, "gitops_push: false dans l'exemple");
    assert!(message.contains("socle-operation: 0193a0b4"));
    assert_eq!(
        edits[0],
        FileEdit::SetImage {
            file: "gitops/apps/erp/overlays/prod/kustomization.yaml".into(),
            image: "erp-app".into(),
            new_name: "ghcr.io/acme/erp-app".into(),
            digest: new.digest.clone(),
        }
    );
    let StepAction::WaitArgo { application, timeout_secs, image_ref, .. } = &p.steps[1].action else {
        panic!("attente Argo CD attendue");
    };
    assert_eq!(application, "erp-prod");
    assert_eq!(*timeout_secs, 2_700, "canary : délai long");
    assert_eq!(image_ref.as_deref(), Some(new.image_ref().as_str()));

    assert_eq!(p.verify.len(), 2, "sonde HTTP + SLO");
    assert!(
        matches!(&p.verify[0].action, StepAction::HttpCheck { url, .. } if url == "https://erp.example.org/health")
    );
    assert!(matches!(&p.verify[1].action, StepAction::PromCheck { query, .. } if query.contains(r#"env="prod""#)));
    assert_eq!(p.rollback.len(), 2, "rétablissement de 1.2.0 + attente");
    assert_eq!(p.locks, vec!["env:prod:service:erp-app".to_string()]);
    // Déterminisme : même entrée, même empreinte.
    let again = plan(&ctx(&spec, &req, Some(&new), Some(&cur))).expect("plan");
    assert_eq!(p.hash(), again.hash());
}

#[test]
fn dev_deploy_without_wait_has_no_verification() {
    let spec = sample();
    let new = release("erp-app", "1.3.0", 'c');
    let req = request("dev", Action::Deploy { service: "erp-app".into(), release: "1.3.0".into() });
    let p = plan(&ctx(&spec, &req, Some(&new), None)).expect("plan");
    assert_eq!(p.steps.len(), 1);
    assert!(p.verify.is_empty());
    assert!(p.rollback.is_empty(), "premier déploiement : rien à rétablir");
}

#[test]
fn vm_canary_uses_ansible_batches() {
    let spec = sample();
    let new = release("billing-worker", "2.0.0", 'd');
    let req = request("prod", Action::Deploy { service: "billing-worker".into(), release: "2.0.0".into() });
    let p = plan(&ctx(&spec, &req, Some(&new), None)).expect("plan");
    let StepAction::Exec(e) = &p.steps[0].action else { panic!("exec attendu") };
    assert_eq!(e.program, "ansible-playbook");
    assert_eq!(e.cwd.as_deref(), Some("infrastructure/ansible"));
    assert!(e.args.contains(&"--limit=app_servers".to_string()));
    let vars = e.args.iter().find_map(|a| a.strip_prefix("--extra-vars=")).expect("vars");
    let vars: serde_json::Value = serde_json::from_str(vars).expect("json");
    assert_eq!(vars["socle_serial"], serde_json::json!([1, "25%", "100%"]));
    assert_eq!(vars["socle_image"], serde_json::json!(new.image_ref()));
    assert!(p.locks.contains(&"target:rhel-prod".to_string()));
}

#[test]
fn release_must_match_service_image() {
    let spec = sample();
    let mut other = release("erp-app", "9.9.9", 'e');
    other.image = "docker.io/evil/erp-app".into();
    let req = request("prod", Action::Deploy { service: "erp-app".into(), release: "9.9.9".into() });
    assert!(plan(&ctx(&spec, &req, Some(&other), None)).is_err());
}

#[test]
fn scale_respects_declared_bounds() {
    let spec = sample();
    let ok = request("prod", Action::Scale { service: "erp-app".into(), min: 3, max: 8 });
    let p = plan(&ctx(&spec, &ok, None, None)).expect("plan");
    let StepAction::GitopsUpdate { edits, .. } = &p.steps[0].action else { panic!("gitops") };
    let FileEdit::Write { file, content } = &edits[0] else { panic!("écriture") };
    assert!(file.ends_with("overlays/prod/socle-scaling.yaml"));
    assert!(content.contains("minReplicas: 3") && content.contains("maxReplicas: 8"));

    let too_big = request("prod", Action::Scale { service: "erp-app".into(), min: 3, max: 50 });
    assert!(plan(&ctx(&spec, &too_big, None, None)).is_err());
    let vm = request("prod", Action::Scale { service: "billing-worker".into(), min: 1, max: 2 });
    assert!(plan(&ctx(&spec, &vm, None, None)).is_err(), "pas d'HPA sur VM");
}

#[test]
fn infra_apply_replays_the_approved_plan() {
    let spec = sample();
    let req = request("prod", Action::InfraApply { stack: "aws-core".into(), plan_operation: "op-0".into() });
    let mut c = ctx(&spec, &req, None, None);
    c.infra_plan_summary = Some("abc123");
    let p = plan(&c).expect("plan");
    let names: Vec<&str> = p.steps.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["Initialisation", "Plan", "Contrôle : plan identique au plan approuvé", "Application"]);
    let StepAction::Exec(init) = &p.steps[0].action else { panic!() };
    assert!(init.args.contains(&"-backend-config=backend/prod.hcl".to_string()));
    let StepAction::Exec(pl) = &p.steps[1].action else { panic!() };
    assert_eq!(pl.success_codes, vec![0, 2]);
    assert!(pl.args.contains(&"-var-file=env/prod.tfvars".to_string()));
    assert!(matches!(&p.steps[2].action, StepAction::TfPlanGuard { expected: Some(h), .. } if h == "abc123"));
    assert_eq!(p.cleanup.len(), 1);
    assert!(p.cleanup[0].starts_with("infrastructure/terraform/aws/.socle-"));

    let dev = request("dev", Action::InfraPlan { stack: "aws-core".into() });
    assert!(plan(&ctx(&spec, &dev, None, None)).is_err(), "stack non déclarée en dev");
}

#[test]
fn backups_restores_and_drills() {
    let spec = sample();
    let req = request("prod", Action::Backup { policy: "erp-k8s".into() });
    let p = plan(&ctx(&spec, &req, None, None)).expect("plan");
    let snapshot = p.outputs.get("snapshot").expect("nom de sauvegarde Velero");
    assert!(snapshot.starts_with("socle-erp-k8s-"));
    let StepAction::Exec(e) = &p.steps[0].action else { panic!() };
    assert!(e.args.contains(&"--kubecontext=eks-prod".to_string()));
    assert!(e.args.contains(&"--ttl=168h0m0s".to_string()));

    let files = request("prod", Action::Backup { policy: "erp-files".into() });
    let p = plan(&ctx(&spec, &files, None, None)).expect("plan");
    let StepAction::Exec(e) = &p.steps[0].action else { panic!() };
    assert!(e.args.contains(&"playbooks/backup_restic.yml".to_string()));

    let wrong_env = request("dev", Action::Backup { policy: "erp-files".into() });
    assert!(plan(&ctx(&spec, &wrong_env, None, None)).is_err());

    let restore = request(
        "prod",
        Action::Restore {
            policy: "erp-files".into(),
            snapshot: None,
            point_in_time: Some(Utc::now()),
            in_place: false,
        },
    );
    assert!(plan(&ctx(&spec, &restore, None, None)).is_err(), "PITR indisponible pour restic");

    let drill = request("prod", Action::RestoreDrill { policy: "erp-files".into() });
    let p = plan(&ctx(&spec, &drill, None, None)).expect("exercice");
    assert!(p.summary.contains("RTO cible 1h"));
    let no_drill = request("prod", Action::RestoreDrill { policy: "erp-k8s".into() });
    assert!(plan(&ctx(&spec, &no_drill, None, None)).is_err());
}

#[test]
fn patch_and_runbook() {
    let spec = sample();
    let req = request("prod", Action::Patch { target: "rhel-prod".into() });
    let p = plan(&ctx(&spec, &req, None, None)).expect("plan");
    assert!(matches!(&p.steps[0].action, StepAction::Silence { matchers, .. } if matchers["env"] == "prod"));
    let StepAction::Exec(e) = &p.steps[1].action else { panic!() };
    assert!(e.args.contains(&"playbooks/patch_rhel.yml".to_string()));

    let rb = request("prod", Action::Runbook { runbook: "flush-cache".into() });
    let p = plan(&ctx(&spec, &rb, None, None)).expect("runbook");
    assert_eq!(p.steps.len(), 2);
    let rb_dev = request("dev", Action::Runbook { runbook: "flush-cache".into() });
    assert!(plan(&ctx(&spec, &rb_dev, None, None)).is_err());
}

#[test]
fn describe_masks_nothing_sensitive_and_is_readable() {
    let spec = sample();
    let new = release("erp-app", "1.3.0", 'c');
    let req = request("prod", Action::Deploy { service: "erp-app".into(), release: "1.3.0".into() });
    let p = plan(&ctx(&spec, &req, Some(&new), None)).expect("plan");
    let lines = p.describe();
    assert!(lines.iter().any(|l| l.contains("Synced + Healthy")));
    assert!(lines.iter().any(|l| l.starts_with("[vérification 1]")));
}
