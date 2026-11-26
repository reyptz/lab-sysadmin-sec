//! Charges applicatives : Kubernetes (GitOps Argo CD / Argo Rollouts) et VMs
//! RHEL (Ansible, par lots). Même logique de livraison pour les deux.

use serde_json::json;

use super::{PlanContext, ansible_playbook, ansible_serial, join};
use crate::names::expand;
use crate::operation::{FileEdit, Plan, Step, StepAction};
use crate::release::Release;
use crate::spec::{KubernetesTarget, ServiceInEnv, Strategy, Target, VmTarget};
use crate::{CoreError, Result};

pub(super) fn plan_release(
    ctx: &PlanContext<'_>,
    service: &str,
    release: &Release,
    current: Option<&Release>,
    is_rollback: bool,
) -> Result<Plan> {
    let env = ctx.request.environment.as_str();
    let sie = ctx.spec.service_in_env(service, env)?;
    if release.service != service {
        return Err(CoreError::invalid(format!("la release {} appartient au service {}", release.id, release.service)));
    }
    if release.image != sie.service.image {
        return Err(CoreError::invalid(format!(
            "la release {} provient de {} au lieu de {}",
            release.id, release.image, sie.service.image
        )));
    }
    let verb = if is_rollback { "Retour arrière" } else { "Déploiement" };
    let summary = format!(
        "{verb} de {service} {} en {env} (stratégie {}, cible {})",
        release.label(),
        sie.env.strategy.as_str(),
        sie.target_name
    );
    let mut plan = match sie.target {
        Target::Kubernetes(k) => kube_release(ctx, &sie, k, release, current, is_rollback)?,
        Target::Vm(m) => vm_release(ctx, &sie, m, release, current)?,
    };
    plan.summary = summary;
    plan.verify = health_steps(ctx, &sie);
    plan.locks.insert(0, format!("env:{env}:service:{service}"));
    Ok(plan)
}

fn commit_message(ctx: &PlanContext<'_>, service: &str, release: &Release, what: &str) -> String {
    format!(
        "{what}({service}): {} → {} ({})\n\nsocle-operation: {}\nsocle-actor: {}\n",
        ctx.request.environment,
        release.version,
        release.short_digest(),
        ctx.operation_id,
        ctx.actor
    )
}

fn argo_timeout(strategy: Strategy) -> u64 {
    match strategy {
        Strategy::Rolling => 900,
        Strategy::BlueGreen => 1_200,
        Strategy::Canary => 2_700,
    }
}

fn kube_release(
    ctx: &PlanContext<'_>,
    sie: &ServiceInEnv<'_>,
    k: &KubernetesTarget,
    release: &Release,
    current: Option<&Release>,
    is_rollback: bool,
) -> Result<Plan> {
    let env = ctx.request.environment.as_str();
    let g = sie
        .service
        .gitops
        .as_ref()
        .ok_or_else(|| CoreError::invalid(format!("{} : section gitops manquante", sie.name)))?;
    let overlay = sie.env.overlay(g, env, sie.name);
    let file = join(&overlay, "kustomization.yaml");
    let application = sie.env.argo_application(g, env, sie.name);
    let image_name = g.image_name.clone().unwrap_or_else(|| sie.name.to_string());
    let platform = &ctx.spec.platform;

    let update = |rel: &Release, what: &str| Step {
        name: format!("GitOps : {} → {}", sie.name, rel.label()),
        action: StepAction::GitopsUpdate {
            repo: platform.gitops_repo.clone(),
            branch: platform.gitops_branch.clone(),
            edits: vec![FileEdit::SetImage {
                file: file.clone(),
                image: image_name.clone(),
                new_name: rel.image.clone(),
                digest: rel.digest.clone(),
            }],
            message: commit_message(ctx, sie.name, rel, what),
            push: platform.gitops_push,
        },
    };
    let wait = |rel: &Release| Step {
        name: format!("Convergence Argo CD ({application})"),
        action: StepAction::WaitArgo {
            context: k.context.clone(),
            namespace: k.argocd_namespace.clone(),
            application: application.clone(),
            image_ref: Some(rel.image_ref()),
            timeout_secs: argo_timeout(sie.env.strategy),
        },
    };

    let mut steps = vec![update(release, if is_rollback { "rollback" } else { "deploy" })];
    if sie.env.wait {
        steps.push(wait(release));
    }
    let mut rollback = Vec::new();
    if let Some(cur) = current.filter(|c| c.digest != release.digest) {
        rollback.push(update(cur, "revert"));
        if sie.env.wait {
            rollback.push(wait(cur));
        }
    }
    Ok(Plan { steps, rollback, ..Plan::default() })
}

fn vm_release(
    ctx: &PlanContext<'_>,
    sie: &ServiceInEnv<'_>,
    m: &VmTarget,
    release: &Release,
    current: Option<&Release>,
) -> Result<Plan> {
    let env = ctx.request.environment.as_str();
    let a = sie
        .service
        .ansible
        .as_ref()
        .ok_or_else(|| CoreError::invalid(format!("{} : section ansible manquante", sie.name)))?;
    let vars = |rel: &Release| {
        super::merge_vars(
            json!({
                "socle_service": sie.name,
                "socle_environment": env,
                "socle_image": rel.image_ref(),
                "socle_version": rel.version,
                "socle_strategy": sie.env.strategy.as_str(),
                "socle_serial": ansible_serial(sie.env.strategy, &sie.env.batch),
                "socle_health_path": sie.service.health.as_ref().map(|h| h.path.clone()),
                "socle_operation": ctx.operation_id,
            }),
            &a.extra_vars,
        )
    };
    let steps = vec![ansible_playbook(
        ctx.spec,
        format!("Ansible : {} → {} sur {}", sie.name, release.label(), m.group),
        m,
        &a.playbook,
        &vars(release),
        3_600,
    )];
    let rollback = current
        .filter(|c| c.digest != release.digest)
        .map(|cur| {
            vec![ansible_playbook(
                ctx.spec,
                format!("Ansible : rétablir {} {}", sie.name, cur.label()),
                m,
                &a.playbook,
                &vars(cur),
                3_600,
            )]
        })
        .unwrap_or_default();
    Ok(Plan { steps, rollback, locks: vec![format!("target:{}", sie.target_name)], ..Plan::default() })
}

/// Vérifications post-changement : sonde HTTP puis indicateur SLO.
fn health_steps(ctx: &PlanContext<'_>, sie: &ServiceInEnv<'_>) -> Vec<Step> {
    let mut steps = Vec::new();
    if !sie.env.wait {
        return steps;
    }
    if let (Some(url), Some(h)) = (&sie.env.url, &sie.service.health) {
        steps.push(Step {
            name: "Sonde de santé".into(),
            action: StepAction::HttpCheck {
                url: format!("{}{}", url.trim_end_matches('/'), h.path),
                expect_status: h.expect_status,
                retries: h.retries,
                interval_secs: h.interval.as_secs(),
                timeout_secs: h.timeout.as_secs(),
            },
        });
    }
    if let (Some(slo), Some(prom)) = (&sie.service.slo, &ctx.spec.observability.prometheus_url)
        && let (Some(q), Some(max)) = (&slo.error_rate_query, slo.max_error_rate)
    {
        let samples = (slo.verification_window.as_secs() / 60).clamp(1, 30);
        steps.push(Step {
            name: "Vérification SLO (taux d'erreur)".into(),
            action: StepAction::PromCheck {
                prometheus_url: prom.clone(),
                query: expand(q, &ctx.request.environment, sie.name),
                max,
                samples: u32::try_from(samples).unwrap_or(1),
                interval_secs: 60,
            },
        });
    }
    steps
}

pub(super) fn plan_scale(ctx: &PlanContext<'_>, service: &str, min: u32, max: u32) -> Result<Plan> {
    let env = ctx.request.environment.as_str();
    let sie = ctx.spec.service_in_env(service, env)?;
    let Target::Kubernetes(k) = sie.target else {
        return Err(CoreError::invalid(
            "la mise à l'échelle s'applique aux cibles Kubernetes ; pour des VMs, modifiez la stack d'infrastructure",
        ));
    };
    if let Some(bounds) = sie.env.replicas
        && (min < bounds.min || max > bounds.max)
    {
        return Err(CoreError::invalid(format!(
            "bornes {min}–{max} hors des limites autorisées pour {service} en {env} ({}–{})",
            bounds.min, bounds.max
        )));
    }
    let g = sie.service.gitops.as_ref().ok_or_else(|| CoreError::invalid("section gitops manquante"))?;
    let overlay = sie.env.overlay(g, env, service);
    let workload = g.workload.clone().unwrap_or_else(|| service.to_string());
    let content = format!(
        "# Fichier géré par socle (`socle scale`) — ne pas modifier à la main.\n\
         apiVersion: autoscaling/v2\n\
         kind: HorizontalPodAutoscaler\n\
         metadata:\n  name: {workload}\n\
         spec:\n  minReplicas: {min}\n  maxReplicas: {max}\n"
    );
    let platform = &ctx.spec.platform;
    let application = sie.env.argo_application(g, env, service);
    let mut steps = vec![Step {
        name: format!("GitOps : bornes {min}–{max}"),
        action: StepAction::GitopsUpdate {
            repo: platform.gitops_repo.clone(),
            branch: platform.gitops_branch.clone(),
            edits: vec![FileEdit::Write { file: join(&overlay, "socle-scaling.yaml"), content }],
            message: format!(
                "scale({service}): {env} → min {min}, max {max}\n\nsocle-operation: {}\nsocle-actor: {}\n",
                ctx.operation_id, ctx.actor
            ),
            push: platform.gitops_push,
        },
    }];
    if sie.env.wait {
        steps.push(Step {
            name: format!("Convergence Argo CD ({application})"),
            action: StepAction::WaitArgo {
                context: k.context.clone(),
                namespace: k.argocd_namespace.clone(),
                application,
                image_ref: None,
                timeout_secs: 600,
            },
        });
    }
    Ok(Plan {
        summary: format!("Mise à l'échelle de {service} en {env} : {min} à {max} réplicas"),
        steps,
        locks: vec![format!("env:{env}:service:{service}")],
        ..Plan::default()
    })
}
