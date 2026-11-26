//! Planification : transforme une requête validée en un [`Plan`] déterministe
//! (étapes, vérifications, compensation, verrous). Les planificateurs sont des
//! fonctions pures ; un unique exécuteur générique joue ensuite les étapes.
//! Le plan est haché : une approbation porte sur ce plan exact.

mod backup;
mod infra;
mod ops;
mod workload;

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};

use crate::operation::{Action, ExecStep, OperationRequest, Plan, Step, StepAction};
use crate::release::Release;
use crate::spec::Spec;
use crate::{CoreError, Result};

/// Entrées de planification résolues par le moteur.
pub struct PlanContext<'a> {
    pub spec: &'a Spec,
    pub request: &'a OperationRequest,
    pub operation_id: &'a str,
    pub actor: &'a str,
    pub now: DateTime<Utc>,
    /// Release à déployer (deploy) ou à rétablir (rollback).
    pub release: Option<&'a Release>,
    /// Release actuellement en place (sert au plan de compensation).
    pub current: Option<&'a Release>,
    /// Empreinte du plan d'infrastructure approuvé (infra_apply).
    pub infra_plan_summary: Option<&'a str>,
    /// Instantané résolu (restauration, exercice).
    pub snapshot: Option<&'a str>,
}

pub fn plan(ctx: &PlanContext<'_>) -> Result<Plan> {
    ctx.request.validate_basic()?;
    let env = &ctx.request.environment;
    ctx.spec.environment(env)?;
    match &ctx.request.action {
        Action::Deploy { service, .. } => {
            let release = ctx.release.ok_or_else(|| CoreError::not_found("release à déployer"))?;
            workload::plan_release(ctx, service, release, ctx.current, false)
        }
        Action::Rollback { service, .. } => {
            let release = ctx
                .release
                .ok_or_else(|| CoreError::invalid("aucune release antérieure connue pour ce retour arrière"))?;
            workload::plan_release(ctx, service, release, ctx.current, true)
        }
        Action::Scale { service, min, max } => workload::plan_scale(ctx, service, *min, *max),
        Action::InfraPlan { stack } => infra::plan_infra(ctx, stack, None),
        Action::InfraApply { stack, .. } => {
            let summary = ctx
                .infra_plan_summary
                .ok_or_else(|| CoreError::invalid("plan d'infrastructure approuvé introuvable"))?;
            infra::plan_infra(ctx, stack, Some(summary))
        }
        Action::Backup { policy } => backup::plan_backup(ctx, policy),
        Action::Restore { policy, point_in_time, in_place, .. } => {
            backup::plan_restore(ctx, policy, *point_in_time, *in_place)
        }
        Action::RestoreDrill { policy } => backup::plan_drill(ctx, policy),
        Action::Patch { target } => ops::plan_patch(ctx, target),
        Action::Runbook { runbook } => ops::plan_runbook(ctx, runbook),
    }
}

// ---------------------------------------------------------------------------
// Briques communes
// ---------------------------------------------------------------------------

pub(crate) fn exec(
    name: impl Into<String>,
    program: &str,
    args: Vec<String>,
    cwd: Option<String>,
    timeout_secs: u64,
) -> Step {
    Step {
        name: name.into(),
        action: StepAction::Exec(ExecStep {
            program: program.to_string(),
            args,
            cwd,
            env: BTreeMap::new(),
            timeout_secs,
            success_codes: Vec::new(),
        }),
    }
}

/// Joint un dossier relatif et un chemin relatif (`.` neutre).
pub(crate) fn join(dir: &str, path: &str) -> String {
    let dir = dir.trim_end_matches('/');
    if dir.is_empty() || dir == "." { path.to_string() } else { format!("{dir}/{path}") }
}

/// Fin de l'identifiant d'opération (partie aléatoire d'un UUIDv7), utilisée
/// pour nommer des artefacts de façon unique et stable.
pub(crate) fn op_tail(id: &str) -> String {
    let compact: String = id.chars().filter(char::is_ascii_alphanumeric).collect();
    compact[compact.len().saturating_sub(12)..].to_ascii_lowercase()
}

/// Exécution d'un playbook Ansible sur une cible VM. Toutes les options sont
/// passées sous la forme `--option=valeur` (aucune ambiguïté possible avec
/// une valeur commençant par un tiret) et les variables en JSON.
pub(crate) fn ansible_playbook(
    spec: &Spec,
    name: impl Into<String>,
    target: &crate::spec::VmTarget,
    playbook: &str,
    vars: &serde_json::Value,
    timeout_secs: u64,
) -> Step {
    exec(
        name,
        "ansible-playbook",
        vec![
            format!("--inventory={}", target.inventory),
            format!("--limit={}", target.group),
            format!("--extra-vars={vars}"),
            playbook.to_string(),
        ],
        Some(spec.platform.ansible_dir.clone()),
        timeout_secs,
    )
}

pub(crate) fn vm_target<'a>(spec: &'a Spec, name: &str) -> Result<&'a crate::spec::VmTarget> {
    match spec.target(name)? {
        crate::spec::Target::Vm(m) => Ok(m),
        _ => Err(CoreError::invalid(format!("la cible {name} n'est pas une VM"))),
    }
}

pub(crate) fn kube_target<'a>(spec: &'a Spec, name: &str) -> Result<&'a crate::spec::KubernetesTarget> {
    match spec.target(name)? {
        crate::spec::Target::Kubernetes(k) => Ok(k),
        _ => Err(CoreError::invalid(format!("la cible {name} n'est pas un cluster Kubernetes"))),
    }
}

/// Fusionne des variables Ansible déclarées dans la spécification avec les
/// variables `socle_*` (ces dernières sont prioritaires et réservées).
pub(crate) fn merge_vars(base: serde_json::Value, extra: &BTreeMap<String, serde_json::Value>) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (k, v) in extra {
        map.insert(k.clone(), v.clone());
    }
    if let serde_json::Value::Object(o) = base {
        for (k, v) in o {
            map.insert(k, v);
        }
    }
    serde_json::Value::Object(map)
}

/// Lots Ansible (`serial`) selon la stratégie : un hôte témoin d'abord en canary.
pub(crate) fn ansible_serial(strategy: crate::spec::Strategy, batch: &str) -> Vec<serde_json::Value> {
    use serde_json::Value;
    let batch_value = batch.parse::<u64>().map_or_else(|_| Value::String(batch.to_string()), Value::from);
    match strategy {
        crate::spec::Strategy::Canary => vec![Value::from(1), batch_value, Value::String("100%".into())],
        _ => vec![batch_value],
    }
}

#[cfg(test)]
mod tests;
