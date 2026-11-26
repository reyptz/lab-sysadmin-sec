//! Infrastructure multi-cloud (Terraform / OpenTofu). Le cycle est en deux
//! temps pour éliminer tout écart entre ce qui est approuvé et ce qui est
//! appliqué : `infra_plan` calcule et empreinte le plan ; `infra_apply`
//! recalcule le plan, vérifie qu'il est identique au plan approuvé, puis
//! applique ce fichier de plan précis.

use super::{PlanContext, exec, join, op_tail};
use crate::names::expand;
use crate::operation::{ExecStep, Plan, Step, StepAction};
use crate::{CoreError, Result};

pub(super) fn plan_infra(ctx: &PlanContext<'_>, stack_name: &str, expected: Option<&str>) -> Result<Plan> {
    let env = ctx.request.environment.as_str();
    let stack = ctx.spec.stack(stack_name)?;
    if !stack.environments.iter().any(|e| e == env) {
        return Err(CoreError::invalid(format!("la stack {stack_name} n'est pas déclarée pour l'environnement {env}")));
    }
    let program = stack.tool.program();
    let chdir = format!("-chdir={}", stack.path);
    let planfile = format!(".socle-{}.tfplan", op_tail(ctx.operation_id));

    let mut init = vec![chdir.clone(), "init".into(), "-input=false".into(), "-no-color".into()];
    if let Some(b) = &stack.backend_config {
        init.push(format!("-backend-config={}", expand(b, env, stack_name)));
    }
    let mut plan_args = vec![
        chdir.clone(),
        "plan".into(),
        "-input=false".into(),
        "-no-color".into(),
        "-lock-timeout=300s".into(),
        "-detailed-exitcode".into(),
        format!("-out={planfile}"),
    ];
    for f in &stack.var_files {
        plan_args.push(format!("-var-file={}", expand(f, env, stack_name)));
    }

    let mut steps = vec![
        exec("Initialisation", program, init, None, 900),
        Step {
            name: "Plan".into(),
            action: StepAction::Exec(ExecStep {
                program: program.into(),
                args: plan_args,
                cwd: None,
                env: Default::default(),
                timeout_secs: 1_800,
                // 0 : aucun changement ; 2 : changements à appliquer.
                success_codes: vec![0, 2],
            }),
        },
        Step {
            name: if expected.is_some() {
                "Contrôle : plan identique au plan approuvé".into()
            } else {
                "Empreinte du plan".into()
            },
            action: StepAction::TfPlanGuard {
                program: program.into(),
                chdir: stack.path.clone(),
                planfile: planfile.clone(),
                expected: expected.map(str::to_string),
            },
        },
    ];
    let summary = if expected.is_some() {
        steps.push(exec(
            "Application",
            program,
            vec![
                chdir,
                "apply".into(),
                "-input=false".into(),
                "-no-color".into(),
                "-lock-timeout=300s".into(),
                planfile.clone(),
            ],
            None,
            3_600,
        ));
        format!("Application de la stack {stack_name} en {env} ({program})")
    } else {
        format!("Plan de la stack {stack_name} en {env} ({program})")
    };
    Ok(Plan {
        summary,
        steps,
        locks: vec![format!("stack:{stack_name}:env:{env}")],
        cleanup: vec![join(&stack.path, &planfile)],
        ..Plan::default()
    })
}
