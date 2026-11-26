//! Sauvegardes, restaurations et exercices : un seul modèle de politique,
//! plusieurs outils (restic, pgBackRest, Velero, commandes fournisseur).

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde_json::json;

use super::{PlanContext, ansible_playbook, exec, kube_target, op_tail, vm_target};
use crate::operation::{Plan, Step};
use crate::spec::{BackupPolicy, BackupSource, CommandSource, Retention};
use crate::{CoreError, Result, format_ts};

const RESTIC_PLAYBOOK: &str = "playbooks/backup_restic.yml";
const PGBACKREST_PLAYBOOK: &str = "playbooks/backup_pgbackrest.yml";

fn policy_for<'a>(ctx: &PlanContext<'a>, name: &str) -> Result<&'a BackupPolicy> {
    let policy = ctx.spec.backup(name)?;
    if policy.environment != ctx.request.environment {
        return Err(CoreError::invalid(format!("la politique {name} concerne l'environnement {}", policy.environment)));
    }
    Ok(policy)
}

fn retention_json(r: &Retention) -> serde_json::Value {
    json!({
        "last": r.last, "hourly": r.hourly, "daily": r.daily,
        "weekly": r.weekly, "monthly": r.monthly, "yearly": r.yearly,
    })
}

fn command_step(
    ctx: &PlanContext<'_>,
    name: &str,
    title: &str,
    argv: &[String],
    c: &CommandSource,
    snapshot: Option<&str>,
    pit: Option<DateTime<Utc>>,
) -> Result<Step> {
    let (program, args) = argv.split_first().ok_or_else(|| CoreError::invalid("commande vide"))?;
    let pit = pit.map(|t| format_ts(&t)).unwrap_or_default();
    let tail = op_tail(ctx.operation_id);
    let args = args
        .iter()
        .map(|a| {
            a.replace("{policy}", name)
                .replace("{env}", &ctx.request.environment)
                .replace("{operation}", &tail)
                .replace("{snapshot}", snapshot.unwrap_or("latest"))
                .replace("{point_in_time}", &pit)
        })
        .collect();
    let timeout = c.timeout.map_or(3_600, |t| t.as_secs());
    Ok(exec(title, program, args, None, timeout))
}

pub(super) fn plan_backup(ctx: &PlanContext<'_>, name: &str) -> Result<Plan> {
    let policy = policy_for(ctx, name)?;
    let env = ctx.request.environment.as_str();
    let mut outputs = BTreeMap::new();
    let step = match &policy.source {
        BackupSource::Files(f) => ansible_playbook(
            ctx.spec,
            format!("Sauvegarde restic ({})", f.paths.join(", ")),
            vm_target(ctx.spec, &f.target)?,
            RESTIC_PLAYBOOK,
            &json!({
                "socle_restic_action": "backup",
                "socle_policy": name,
                "socle_environment": env,
                "socle_operation": ctx.operation_id,
                "socle_repository": f.repository,
                "socle_copies": f.copies,
                "socle_paths": f.paths,
                "socle_exclude": f.exclude,
                "socle_retention": retention_json(&policy.retention),
            }),
            7_200,
        ),
        BackupSource::Postgres(p) => ansible_playbook(
            ctx.spec,
            format!("Sauvegarde pgBackRest (stanza {})", p.stanza),
            vm_target(ctx.spec, &p.target)?,
            PGBACKREST_PLAYBOOK,
            &json!({
                "socle_pgbackrest_action": "backup",
                "socle_stanza": p.stanza,
                "socle_policy": name,
                "socle_environment": env,
                "socle_operation": ctx.operation_id,
                "socle_retention": retention_json(&policy.retention),
            }),
            7_200,
        ),
        BackupSource::Kubernetes(k) => {
            let t = kube_target(ctx.spec, &k.target)?;
            let backup_name = format!("socle-{name}-{}", op_tail(ctx.operation_id));
            let mut args = vec![
                format!("--kubecontext={}", t.context),
                "backup".into(),
                "create".into(),
                backup_name.clone(),
                format!("--include-namespaces={}", k.namespaces.join(",")),
                format!("--ttl={}h0m0s", policy.retention.horizon_hours()),
                format!("--labels=socle.policy={name},socle.environment={env}"),
                "--wait".into(),
            ];
            if let Some(loc) = &k.storage_location {
                args.push(format!("--storage-location={loc}"));
            }
            outputs.insert("snapshot".to_string(), backup_name);
            exec("Sauvegarde Velero", "velero", args, None, 7_200)
        }
        BackupSource::Command(c) => command_step(ctx, name, "Sauvegarde (commande)", &c.backup, c, None, None)?,
    };
    Ok(Plan {
        summary: format!("Sauvegarde {name} en {env} ({}, RPO {})", policy.source.kind_name(), policy.rpo),
        steps: vec![step],
        locks: vec![format!("backup:{name}")],
        outputs,
        ..Plan::default()
    })
}

pub(super) fn plan_restore(
    ctx: &PlanContext<'_>,
    name: &str,
    pit: Option<DateTime<Utc>>,
    in_place: bool,
) -> Result<Plan> {
    let policy = policy_for(ctx, name)?;
    let env = ctx.request.environment.as_str();
    let tail = op_tail(ctx.operation_id);
    if pit.is_some() && !matches!(policy.source, BackupSource::Postgres(_) | BackupSource::Command(_)) {
        return Err(CoreError::invalid(
            "la restauration à un instant donné n'est disponible que pour PostgreSQL (pgBackRest) ou les commandes fournisseur",
        ));
    }
    let step = match &policy.source {
        BackupSource::Files(f) => ansible_playbook(
            ctx.spec,
            "Restauration restic".to_string(),
            vm_target(ctx.spec, &f.target)?,
            RESTIC_PLAYBOOK,
            &json!({
                "socle_restic_action": "restore",
                "socle_policy": name,
                "socle_environment": env,
                "socle_operation": ctx.operation_id,
                "socle_repository": f.repository,
                "socle_snapshot": ctx.snapshot.unwrap_or("latest"),
                "socle_restore_target": if in_place { "/".to_string() } else { format!("/var/lib/socle/restore/{tail}") },
            }),
            7_200,
        ),
        BackupSource::Postgres(p) => {
            if !in_place {
                return Err(CoreError::invalid(
                    "pgBackRest restaure l'instance en place : confirmez avec in_place, ou lancez un exercice de restauration",
                ));
            }
            ansible_playbook(
                ctx.spec,
                format!("Restauration pgBackRest (stanza {})", p.stanza),
                vm_target(ctx.spec, &p.target)?,
                PGBACKREST_PLAYBOOK,
                &json!({
                    "socle_pgbackrest_action": "restore",
                    "socle_stanza": p.stanza,
                    "socle_policy": name,
                    "socle_environment": env,
                    "socle_operation": ctx.operation_id,
                    "socle_point_in_time": pit.map(|t| format_ts(&t)),
                }),
                7_200,
            )
        }
        BackupSource::Kubernetes(k) => {
            let t = kube_target(ctx.spec, &k.target)?;
            let snapshot =
                ctx.snapshot.ok_or_else(|| CoreError::invalid("aucune sauvegarde Velero réussie à restaurer"))?;
            let mut args = vec![
                format!("--kubecontext={}", t.context),
                "restore".into(),
                "create".into(),
                format!("socle-restore-{tail}"),
                format!("--from-backup={snapshot}"),
                "--wait".into(),
            ];
            if !in_place {
                let mapping: Vec<String> = k.namespaces.iter().map(|n| format!("{n}:{n}-restored")).collect();
                args.push(format!("--namespace-mappings={}", mapping.join(",")));
            }
            exec("Restauration Velero", "velero", args, None, 7_200)
        }
        BackupSource::Command(c) => {
            let argv = c.restore.as_ref().ok_or_else(|| {
                CoreError::invalid(format!("la politique {name} ne déclare pas de commande de restauration"))
            })?;
            command_step(ctx, name, "Restauration (commande)", argv, c, ctx.snapshot, pit)?
        }
    };
    let mode = if in_place { "en place" } else { "à côté de l'existant" };
    Ok(Plan {
        summary: format!(
            "Restauration {name} en {env} ({mode}{})",
            pit.map(|t| format!(", état au {}", format_ts(&t))).unwrap_or_default()
        ),
        steps: vec![step],
        locks: vec![format!("backup:{name}"), format!("env:{env}:restore")],
        ..Plan::default()
    })
}

pub(super) fn plan_drill(ctx: &PlanContext<'_>, name: &str) -> Result<Plan> {
    let policy = policy_for(ctx, name)?;
    let drill = policy
        .drill
        .as_ref()
        .ok_or_else(|| CoreError::invalid(format!("la politique {name} ne prévoit pas d'exercice de restauration")))?;
    let env = ctx.request.environment.as_str();
    let tail = op_tail(ctx.operation_id);
    let mut steps = Vec::new();
    match &policy.source {
        BackupSource::Files(f) => {
            let target = drill.target.as_deref().unwrap_or(&f.target);
            steps.push(ansible_playbook(
                ctx.spec,
                "Exercice de restauration restic".to_string(),
                vm_target(ctx.spec, target)?,
                RESTIC_PLAYBOOK,
                &json!({
                    "socle_restic_action": "drill",
                    "socle_policy": name,
                    "socle_environment": env,
                    "socle_operation": ctx.operation_id,
                    "socle_repository": f.repository,
                    "socle_snapshot": "latest",
                    "socle_restore_target": format!("/var/lib/socle/drill/{tail}"),
                }),
                policy.rto.as_secs().max(600) * 2,
            ));
        }
        BackupSource::Postgres(p) => {
            let target = drill.target.as_deref().unwrap_or(&p.target);
            steps.push(ansible_playbook(
                ctx.spec,
                "Exercice de restauration pgBackRest".to_string(),
                vm_target(ctx.spec, target)?,
                PGBACKREST_PLAYBOOK,
                &json!({
                    "socle_pgbackrest_action": "drill",
                    "socle_stanza": p.stanza,
                    "socle_policy": name,
                    "socle_environment": env,
                    "socle_operation": ctx.operation_id,
                }),
                policy.rto.as_secs().max(600) * 2,
            ));
        }
        BackupSource::Kubernetes(k) => {
            let target = drill.target.as_deref().unwrap_or(&k.target);
            let t = kube_target(ctx.spec, target)?;
            let snapshot =
                ctx.snapshot.ok_or_else(|| CoreError::invalid("aucune sauvegarde Velero réussie pour l'exercice"))?;
            let mapping: Vec<String> = k.namespaces.iter().map(|n| format!("{n}:{n}-drill")).collect();
            steps.push(exec(
                "Exercice de restauration Velero",
                "velero",
                vec![
                    format!("--kubecontext={}", t.context),
                    "restore".into(),
                    "create".into(),
                    format!("socle-drill-{tail}"),
                    format!("--from-backup={snapshot}"),
                    format!("--namespace-mappings={}", mapping.join(",")),
                    "--wait".into(),
                ],
                None,
                policy.rto.as_secs().max(600) * 2,
            ));
            let mut delete = vec![format!("--context={}", t.context), "delete".into(), "namespace".into()];
            delete.extend(k.namespaces.iter().map(|n| format!("{n}-drill")));
            delete.extend(["--wait=true".to_string(), "--ignore-not-found".to_string()]);
            steps.push(exec("Nettoyage de l'exercice", "kubectl", delete, None, 900));
        }
        BackupSource::Command(c) => {
            let argv = c.drill.as_ref().ok_or_else(|| CoreError::invalid("commande d'exercice absente"))?;
            steps.push(command_step(ctx, name, "Exercice (commande)", argv, c, ctx.snapshot, None)?);
        }
    }
    Ok(Plan {
        summary: format!("Exercice de restauration {name} en {env} (RTO cible {})", policy.rto),
        steps,
        locks: vec![format!("backup:{name}")],
        ..Plan::default()
    })
}
