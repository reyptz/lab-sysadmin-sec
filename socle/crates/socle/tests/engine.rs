//! Scénarios de bout en bout du moteur (base SQLite réelle, exécuteur simulé).

#![allow(clippy::unwrap_used)]

mod common;

use chrono::{TimeZone, Utc};
use common::{digest, good_release, harness, principal};
use socle::error::Error;
use socle_core::CoreError;
use socle_core::duration::HumanDuration;
use socle_core::incident::{AlertmanagerPayload, IncidentStatus};
use socle_core::operation::{Action, ApprovalDecision, GateStatus, OpStatus, OperationRequest};
use socle_core::release::VulnSummary;

fn deploy(env: &str, release: &str, ticket: Option<&str>) -> OperationRequest {
    OperationRequest {
        environment: env.into(),
        action: Action::Deploy { service: "erp-app".into(), release: release.into() },
        reason: "Livraison de la version".into(),
        change_ticket: ticket.map(str::to_string),
        emergency: false,
    }
}

#[tokio::test]
async fn release_flows_from_dev_to_prod_with_four_eyes() {
    let h = harness().await;
    let dev = principal("dev@example.org", &["erp"]);
    let sre = principal("sre@example.org", &["sre"]);
    let e = &h.engine;

    let r = e.register_release(&dev, good_release("1.0.0", 'a')).await.unwrap();
    // Idempotence : même digest → même release.
    assert_eq!(e.register_release(&dev, good_release("1.0.0", 'a')).await.unwrap().id, r.id);

    // dev : politique souple, exécution directe.
    let op = e.submit(&dev, deploy("dev", "1.0.0", None)).await.unwrap();
    assert_eq!(op.status, OpStatus::Queued);
    let done = e.run_next().await.unwrap().unwrap();
    assert_eq!(done.status, OpStatus::Succeeded, "{:?}", done.outcome);
    assert!(h.runner.executed.lock().unwrap().iter().any(|s| s.starts_with("GitOps")));

    // prod : promotion validée, ticket fourni → une approbation requise.
    let op = e.submit(&dev, deploy("prod", &r.digest, Some("CHG-42"))).await.unwrap();
    assert_eq!(op.status, OpStatus::AwaitingApproval, "{:#?}", op.gates);
    assert!(op.gates.iter().all(|g| g.status == GateStatus::Passed));
    assert!(e.run_next().await.unwrap().is_none(), "rien d'exécutable avant approbation");

    // Le demandeur n'a pas le rôle d'approbateur ; l'admin ne peut pas
    // approuver sa propre opération (quatre yeux).
    assert!(matches!(
        e.decide(&dev, &op.id, ApprovalDecision::Approved, "").await,
        Err(Error::Core(CoreError::Forbidden(_)))
    ));
    let admin = principal("root@example.org", &["platform"]);
    let own = e.submit(&admin, deploy("prod", "1.0.0", Some("CHG-43"))).await.unwrap();
    assert!(matches!(
        e.decide(&admin, &own.id, ApprovalDecision::Approved, "").await,
        Err(Error::Core(CoreError::Forbidden(_)))
    ));
    e.cancel(&admin, &own.id, "doublon").await.unwrap();

    let approved = e.decide(&sre, &op.id, ApprovalDecision::Approved, "RAS").await.unwrap();
    assert_eq!(approved.status, OpStatus::Queued);
    let done = e.run_next().await.unwrap().unwrap();
    assert_eq!(done.status, OpStatus::Succeeded);

    let envs = e.environments(&dev).await.unwrap();
    let prod = envs.iter().find(|v| v.name == "prod").unwrap();
    assert_eq!(prod.services[0].version.as_deref(), Some("1.0.0"));

    let report = e.verify_audit(&admin).await.unwrap();
    assert!(report.valid, "{report:?}");
    assert!(report.events_checked >= 10);
}

#[tokio::test]
async fn failed_verification_rolls_back_and_opens_incident() {
    let h = harness().await;
    let e = &h.engine;
    let admin = principal("root@example.org", &["platform"]);
    let sre = principal("sre@example.org", &["sre"]);
    for (v, fill) in [("1.0.0", 'a'), ("2.0.0", 'b')] {
        e.register_release(&admin, good_release(v, fill)).await.unwrap();
        e.submit(&admin, deploy("dev", v, None)).await.unwrap();
        e.run_next().await.unwrap();
    }
    let op = e.submit(&admin, deploy("prod", "1.0.0", Some("CHG-1"))).await.unwrap();
    e.decide(&sre, &op.id, ApprovalDecision::Approved, "").await.unwrap();
    assert_eq!(e.run_next().await.unwrap().unwrap().status, OpStatus::Succeeded);

    // La sonde de santé échoue après le passage en 2.0.0.
    *h.runner.fail_on.lock().unwrap() = Some("Sonde de santé".into());
    let op = e.submit(&admin, deploy("prod", "2.0.0", Some("CHG-2"))).await.unwrap();
    assert_eq!(op.plan.rollback.len(), 2, "rétablissement de 1.0.0 prévu");
    e.decide(&sre, &op.id, ApprovalDecision::Approved, "").await.unwrap();
    let done = e.run_next().await.unwrap().unwrap();
    assert_eq!(done.status, OpStatus::RolledBack, "{:?}", done.outcome);
    let outcome = done.outcome.unwrap();
    assert!(outcome.rolled_back);
    let incident = e.get_incident(&admin, outcome.incident_id.as_deref().unwrap()).await.unwrap();
    assert_eq!(incident.status, IncidentStatus::Open);
    assert_eq!(incident.linked_operations, vec![done.id.clone()]);

    // L'état de référence reste 1.0.0 et les journaux tracent le retour arrière.
    let current = e.environments(&admin).await.unwrap();
    let prod = current.iter().find(|v| v.name == "prod").unwrap();
    assert_eq!(prod.services[0].version.as_deref(), Some("1.0.0"));
    let logs = e.operation_logs(&admin, &done.id, 0, 1000).await.unwrap();
    assert!(logs.iter().any(|l| l.step.starts_with("retour arrière")));
}

#[tokio::test]
async fn supply_chain_gates_reject_vulnerable_release() {
    let h = harness().await;
    let e = &h.engine;
    let admin = principal("root@example.org", &["platform"]);
    let mut bad = good_release("3.0.0", 'c');
    bad.evidence.vulnerabilities = Some(VulnSummary { critical: 2, ..VulnSummary::default() });
    bad.evidence.signature_verified = None;
    e.register_release(&admin, bad).await.unwrap();

    // dev accepte (politique souple)…
    assert_eq!(e.submit(&admin, deploy("dev", "3.0.0", None)).await.unwrap().status, OpStatus::Queued);
    e.run_next().await.unwrap();
    // …prod refuse, même en urgence : les portes de sécurité ne se contournent pas.
    let mut req = deploy("prod", "3.0.0", Some("CHG-9"));
    req.emergency = true;
    req.reason = "Correctif urgent demandé par le métier".into();
    let op = e.submit(&admin, req).await.unwrap();
    assert_eq!(op.status, OpStatus::Rejected);
    let failed: Vec<&str> =
        op.gates.iter().filter(|g| g.status == GateStatus::Failed).map(|g| g.gate.as_str()).collect();
    assert!(failed.contains(&"vulnérabilités") && failed.contains(&"signature"), "{failed:?}");
}

#[tokio::test]
async fn rbac_and_preview() {
    let h = harness().await;
    let e = &h.engine;
    let viewer = principal("auditor@example.org", &["audit"]);
    let dev = principal("dev@example.org", &["erp"]);
    e.register_release(&dev, good_release("1.0.0", 'a')).await.unwrap();
    assert!(matches!(e.submit(&viewer, deploy("dev", "1.0.0", None)).await, Err(Error::Core(CoreError::Forbidden(_)))));
    assert!(e.list_operations(&viewer, &Default::default()).await.is_ok());
    // Un déployeur ne lit pas le journal d'audit.
    assert!(matches!(e.audit_events(&dev, None, 10).await, Err(Error::Core(CoreError::Forbidden(_)))));

    let preview = e.preview(&dev, &deploy("prod", "1.0.0", None)).await.unwrap();
    let failed: Vec<&str> =
        preview.report.results.iter().filter(|g| g.status == GateStatus::Failed).map(|g| g.gate.as_str()).collect();
    assert!(failed.contains(&"ticket") && failed.contains(&"promotion"), "{failed:?}");
    assert!(e.list_operations(&dev, &Default::default()).await.unwrap().is_empty(), "un aperçu n'enregistre rien");
}

#[tokio::test]
async fn change_window_and_emergency_override() {
    let h = harness().await;
    let e = &h.engine;
    let admin = principal("root@example.org", &["platform"]);
    e.register_release(&admin, good_release("1.0.0", 'a')).await.unwrap();
    e.submit(&admin, deploy("dev", "1.0.0", None)).await.unwrap();
    e.run_next().await.unwrap();

    h.clock.set(Utc.with_ymd_and_hms(2026, 11, 27, 20, 0, 0).unwrap()); // vendredi soir
    let op = e.submit(&admin, deploy("prod", "1.0.0", Some("CHG-5"))).await.unwrap();
    assert_eq!(op.status, OpStatus::Rejected);
    assert!(op.gates.iter().any(|g| g.gate == "fenêtre" && g.status == GateStatus::Failed));

    let mut req = deploy("prod", "1.0.0", Some("CHG-6"));
    req.emergency = true;
    req.reason = "Faille critique exploitée en production".into();
    let op = e.submit(&admin, req).await.unwrap();
    assert_eq!(op.status, OpStatus::AwaitingApproval);
    assert_eq!(op.required_approvals, 1);
    assert!(op.gates.iter().any(|g| g.gate == "fenêtre" && g.status == GateStatus::Overridden));
}

#[tokio::test]
async fn rollback_restores_previous_known_good_release() {
    let h = harness().await;
    let e = &h.engine;
    let admin = principal("root@example.org", &["platform"]);
    for (v, fill) in [("1.0.0", 'a'), ("1.1.0", 'b')] {
        e.register_release(&admin, good_release(v, fill)).await.unwrap();
        e.submit(&admin, deploy("dev", v, None)).await.unwrap();
        assert_eq!(e.run_next().await.unwrap().unwrap().status, OpStatus::Succeeded);
    }
    let rb = OperationRequest {
        environment: "dev".into(),
        action: Action::Rollback { service: "erp-app".into(), to_release: None },
        reason: "régression".into(),
        change_ticket: None,
        emergency: false,
    };
    let op = e.submit(&admin, rb).await.unwrap();
    assert_eq!(op.resolved.image_ref.as_deref(), Some(format!("ghcr.io/acme/erp-app@{}", digest('a')).as_str()));
    assert_eq!(e.run_next().await.unwrap().unwrap().status, OpStatus::Succeeded);
    let envs = e.environments(&admin).await.unwrap();
    assert_eq!(envs.iter().find(|v| v.name == "dev").unwrap().services[0].version.as_deref(), Some("1.0.0"));
}

#[tokio::test]
async fn scheduler_runs_backups_and_tracks_rpo() {
    let h = harness().await;
    let e = &h.engine;
    let admin = principal("root@example.org", &["platform"]);

    e.scheduler_tick().await.unwrap();
    let done = e.run_next().await.unwrap().expect("sauvegarde planifiée");
    assert_eq!(done.status, OpStatus::Succeeded);
    assert!(done.requested_by.starts_with("system:scheduler"));
    let status = &e.backup_statuses(&admin).await.unwrap()[0];
    assert!(status.rpo_compliant);
    assert!(status.drill.as_ref().unwrap().overdue, "aucun exercice encore");

    // Le tour suivant lance l'exercice de restauration.
    e.scheduler_tick().await.unwrap();
    assert_eq!(e.run_next().await.unwrap().unwrap().kind().as_str(), "restore_drill");

    // Sauvegardes en échec pendant 1 h : RPO (30 min) dépassé → incident.
    *h.runner.fail_on.lock().unwrap() = Some("Sauvegarde".into());
    for _ in 0..4 {
        h.clock.advance(16 * 60);
        e.scheduler_tick().await.unwrap();
        while e.run_next().await.unwrap().is_some() {}
    }
    let incidents = e.list_incidents(&admin, true, 10).await.unwrap();
    let rpo = incidents.iter().find(|i| i.title.contains("RPO")).expect("incident RPO");
    assert_eq!(rpo.severity.as_str(), "sev2");

    // Retour à la normale : l'incident se résout seul.
    *h.runner.fail_on.lock().unwrap() = None;
    h.clock.advance(16 * 60);
    e.scheduler_tick().await.unwrap();
    while e.run_next().await.unwrap().is_some() {}
    e.scheduler_tick().await.unwrap();
    let rpo = e.get_incident(&admin, &rpo.id).await.unwrap();
    assert_eq!(rpo.status, IncidentStatus::Resolved);
}

#[tokio::test]
async fn alertmanager_alerts_are_deduplicated_and_resolved() {
    let h = harness().await;
    let e = &h.engine;
    let sre = principal("sre@example.org", &["sre"]);
    let payload = |status: &str| -> AlertmanagerPayload {
        serde_json::from_value(serde_json::json!({
            "status": status,
            "alerts": [{"status": status, "fingerprint": "f00d",
                        "labels": {"alertname": "ErrorBudgetBurn", "severity": "critical", "env": "prod", "service": "erp-app"},
                        "annotations": {"summary": "Budget d'erreur consommé trop vite"}}]
        }))
        .unwrap()
    };
    let first = e.ingest_alerts(&sre, &payload("firing")).await.unwrap();
    let second = e.ingest_alerts(&sre, &payload("firing")).await.unwrap();
    assert_eq!(first, second, "même empreinte → même incident");
    let incident = e.get_incident(&sre, &first[0]).await.unwrap();
    assert_eq!(incident.services, vec!["erp-app".to_string()]);

    // Un incident Sev2 ouvert bloque les déploiements en production.
    let admin = principal("root@example.org", &["platform"]);
    e.register_release(&admin, good_release("1.0.0", 'a')).await.unwrap();
    e.submit(&admin, deploy("dev", "1.0.0", None)).await.unwrap();
    e.run_next().await.unwrap();
    let blocked = e.submit(&admin, deploy("prod", "1.0.0", Some("CHG-3"))).await.unwrap();
    assert!(blocked.gates.iter().any(|g| g.gate == "incidents" && g.status == GateStatus::Failed));

    e.ingest_alerts(&sre, &payload("resolved")).await.unwrap();
    assert_eq!(e.get_incident(&sre, &first[0]).await.unwrap().status, IncidentStatus::Resolved);
}

#[tokio::test]
async fn maintenance_freezes_deployments_until_it_ends() {
    let h = harness().await;
    let e = &h.engine;
    let admin = principal("root@example.org", &["platform"]);
    e.register_release(&admin, good_release("1.0.0", 'a')).await.unwrap();
    e.submit(&admin, deploy("dev", "1.0.0", None)).await.unwrap();
    e.run_next().await.unwrap();

    let m = e.start_maintenance(&admin, "prod", HumanDuration::from_mins(60), "Migration de la base").await.unwrap();
    let op = e.submit(&admin, deploy("prod", "1.0.0", Some("CHG-7"))).await.unwrap();
    assert!(op.gates.iter().any(|g| g.gate == "maintenance" && g.status == GateStatus::Failed));
    e.end_maintenance(&admin, &m.id).await.unwrap();
    let op = e.submit(&admin, deploy("prod", "1.0.0", Some("CHG-8"))).await.unwrap();
    assert_eq!(op.status, OpStatus::AwaitingApproval);
}

#[tokio::test]
async fn busy_lock_requeues_with_backoff() {
    let h = harness().await;
    let e = &h.engine;
    let admin = principal("root@example.org", &["platform"]);
    e.register_release(&admin, good_release("1.0.0", 'a')).await.unwrap();
    let op = e.submit(&admin, deploy("dev", "1.0.0", None)).await.unwrap();

    let now = h.clock.now().timestamp_millis();
    assert!(e.store().try_locks(&["env:dev:service:erp-app".into()], "autre-worker", "x", 60_000, now).await.unwrap());
    let after = e.run_next().await.unwrap().unwrap();
    assert_eq!(after.status, OpStatus::Queued, "verrou occupé : remise en file");
    assert!(e.run_next().await.unwrap().is_none(), "délai de reprise respecté");

    e.store().release_locks("autre-worker").await.unwrap();
    h.clock.advance(20);
    let done = e.run_next().await.unwrap().unwrap();
    assert_eq!(done.id, op.id);
    assert_eq!(done.status, OpStatus::Succeeded);
}

#[tokio::test]
async fn infra_apply_requires_a_recent_successful_plan() {
    let h = harness().await;
    let admin = principal("root@example.org", &["platform"]);
    let spec_with_stack = common::SPEC.replace(
        "environments:\n  dev:",
        "stacks:\n  core: { provider: aws-eu, path: infra, environments: [dev] }\nenvironments:\n  dev:",
    );
    // Moteur dédié avec une stack déclarée.
    let dir = tempfile::tempdir().unwrap();
    let store = common::test_store(dir.path()).await;
    let c = h.clock.clone();
    let engine = socle::engine::Engine::new(
        socle_core::spec::Spec::from_yaml(&spec_with_stack).unwrap(),
        None,
        store,
        h.runner.clone(),
        socle::engine::EngineConfig::new(dir.path().to_path_buf()),
    )
    .with_clock(std::sync::Arc::new(move || c.now()));

    let submitted = engine
        .submit(
            &admin,
            OperationRequest {
                environment: "dev".into(),
                action: Action::InfraPlan { stack: "core".into() },
                reason: String::new(),
                change_ticket: None,
                emergency: false,
            },
        )
        .await
        .unwrap();
    assert_eq!(submitted.status, OpStatus::Queued);
    let plan = engine.run_next().await.unwrap().unwrap();
    assert_eq!(plan.status, OpStatus::Succeeded);
    let apply = engine
        .submit(
            &admin,
            OperationRequest {
                environment: "dev".into(),
                action: Action::InfraApply { stack: "core".into(), plan_operation: plan.id.clone() },
                reason: String::new(),
                change_ticket: None,
                emergency: false,
            },
        )
        .await
        .unwrap();
    assert!(apply.plan.steps.iter().any(|s| s.name == "Application"));

    // Plan trop ancien (> 24 h) : refusé.
    h.clock.advance(25 * 3600);
    let stale = engine
        .submit(
            &admin,
            OperationRequest {
                environment: "dev".into(),
                action: Action::InfraApply { stack: "core".into(), plan_operation: plan.id },
                reason: String::new(),
                change_ticket: None,
                emergency: false,
            },
        )
        .await;
    assert!(stale.is_err());
}

#[tokio::test]
async fn api_tokens_authenticate_and_can_be_revoked() {
    let h = harness().await;
    let e = &h.engine;
    let admin = principal("root@example.org", &["platform"]);
    let (token, secret) = e
        .create_token(Some(&admin), "ci", "github:acme/erp", vec!["erp".into()], Some(HumanDuration::from_mins(60)))
        .await
        .unwrap();
    let p = e.authenticate_token(&secret).await.unwrap();
    assert!(p.teams.contains("erp"));
    assert!(e.authenticate_token("socle_pat_inconnu").await.is_err());
    e.revoke_token(&admin, &token.id).await.unwrap();
    assert!(matches!(e.authenticate_token(&secret).await, Err(Error::Unauthenticated(_))));
    // Expiration.
    let (_, short) = e
        .create_token(Some(&admin), "court", "x", vec!["erp".into()], Some(HumanDuration::from_secs(60)))
        .await
        .unwrap();
    h.clock.advance(120);
    assert!(e.authenticate_token(&short).await.is_err());
}
