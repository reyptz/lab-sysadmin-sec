//! Maintenance (correctifs RHEL par lots, avec silence des alertes) et
//! runbooks exécutables.

use serde_json::json;

use super::{PlanContext, ansible_playbook, ansible_serial, exec, merge_vars, vm_target};
use crate::operation::{Plan, Step, StepAction};
use crate::spec::{RunbookStep, Strategy};
use crate::{CoreError, Result};

pub(super) fn plan_patch(ctx: &PlanContext<'_>, target_name: &str) -> Result<Plan> {
    let env = ctx.request.environment.as_str();
    let m = vm_target(ctx.spec, target_name)?;
    let patching = m
        .patching
        .as_ref()
        .ok_or_else(|| CoreError::invalid(format!("aucune configuration de correctifs pour la cible {target_name}")))?;
    let mut steps = Vec::new();
    if let Some(am) = &ctx.spec.observability.alertmanager_url {
        let mut matchers = ctx.spec.environment(env)?.alert_labels.clone();
        if matchers.is_empty() {
            matchers.insert("environment".into(), env.into());
        }
        steps.push(Step {
            name: "Silence des alertes pendant l'intervention".into(),
            action: StepAction::Silence {
                alertmanager_url: am.clone(),
                matchers,
                duration_secs: 7_200,
                comment: format!("socle : correctifs {target_name} (opération {})", ctx.operation_id),
            },
        });
    }
    steps.push(ansible_playbook(
        ctx.spec,
        format!("Correctifs RHEL sur {} (lots de {})", m.group, patching.batch),
        m,
        &patching.playbook,
        &json!({
            "socle_serial": ansible_serial(Strategy::Rolling, &patching.batch),
            "socle_security_only": patching.security_only,
            "socle_reboot": patching.reboot,
            "socle_environment": env,
            "socle_operation": ctx.operation_id,
        }),
        7_200,
    ));
    Ok(Plan {
        summary: format!(
            "Correctifs {} sur {target_name} en {env}",
            if patching.security_only { "de sécurité" } else { "complets" }
        ),
        steps,
        locks: vec![format!("target:{target_name}")],
        ..Plan::default()
    })
}

pub(super) fn plan_runbook(ctx: &PlanContext<'_>, name: &str) -> Result<Plan> {
    let env = ctx.request.environment.as_str();
    let r = ctx.spec.runbook(name)?;
    if !r.environments.is_empty() && !r.environments.iter().any(|e| e == env) {
        return Err(CoreError::invalid(format!("le runbook {name} n'est pas autorisé en {env}")));
    }
    let mut steps = Vec::new();
    for s in &r.steps {
        steps.push(match s {
            RunbookStep::Command { name, program, args, cwd, timeout } => exec(
                name.clone(),
                program,
                args.iter().map(|a| a.replace("{env}", env)).collect(),
                cwd.clone(),
                timeout.map_or(600, |t| t.as_secs()),
            ),
            RunbookStep::Ansible { name, target, playbook, extra_vars } => ansible_playbook(
                ctx.spec,
                name.clone(),
                vm_target(ctx.spec, target)?,
                playbook,
                &merge_vars(json!({"socle_environment": env, "socle_operation": ctx.operation_id}), extra_vars),
                3_600,
            ),
            RunbookStep::HttpCheck { name, url, expect_status } => Step {
                name: name.clone(),
                action: StepAction::HttpCheck {
                    url: url.replace("{env}", env),
                    expect_status: *expect_status,
                    retries: 3,
                    interval_secs: 5,
                    timeout_secs: 10,
                },
            },
        });
    }
    Ok(Plan {
        summary: format!("Runbook {name} en {env} : {}", r.description),
        steps,
        locks: vec![format!("runbook:{name}:env:{env}")],
        ..Plan::default()
    })
}
