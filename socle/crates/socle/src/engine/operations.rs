//! Cycle de vie des opérations : planification, portes, approbations,
//! exécution (avec bail renouvelé et verrous), vérification et retour arrière.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::json;
use socle_core::CoreError;
use socle_core::gates::{self, GateDecision, GateInput, GateReport, OpenIncident};
use socle_core::incident::{IncidentSource, NewIncident, Severity};
use socle_core::operation::{
    Action, ApprovalDecision, GateStatus, OpStatus, Operation, OperationKind, OperationRequest, Outcome, Plan,
    Resolved, Step,
};
use socle_core::planner::{self, PlanContext};
use socle_core::rbac::{Permission, Principal};
use socle_core::release::Release;
use socle_core::spec::{Spec, Tier};
use uuid::Uuid;

use super::Engine;
use crate::error::{Error, Result};
use crate::executor::{OpLogger, StepContext, StepData};
use crate::model::{Deployment, LogLine, OperationFilter, Versioned};
use crate::notify::Event;

/// Résultat de la planification (aperçu ou soumission).
#[derive(Debug, Clone, Serialize)]
pub struct Prepared {
    pub plan: Plan,
    pub plan_hash: String,
    pub description: Vec<String>,
    pub report: GateReport,
    pub resolved: Resolved,
}

fn op_resource(op: &Operation) -> String {
    format!("operation/{}", op.id)
}

fn op_event(op: &Operation, kind: &str, title: String) -> Event {
    let mut e = Event::new(kind, title);
    e.environment = Some(op.request.environment.clone());
    e.link = Some(format!("#/operations/{}", op.id));
    e.text = op.plan.summary.clone();
    e.data = json!({
        "operation": op.id, "kind": op.kind().as_str(), "status": op.status.as_str(),
        "subject": op.request.action.subject(), "requested_by": op.requested_by,
    });
    e
}

impl Engine {
    async fn open_incident_summaries(&self) -> Result<Vec<OpenIncident>> {
        Ok(self
            .store
            .list_incidents(true, 500)
            .await?
            .into_iter()
            .map(|i| OpenIncident { id: i.id, severity: i.severity, environment: i.environment, services: i.services })
            .collect())
    }

    /// Statut de promotion : `None` si le service n'est pas déclaré dans
    /// l'environnement source (porte non applicable).
    async fn promotion_status(&self, spec: &Spec, src: &str, service: &str, release_id: &str) -> Result<Option<bool>> {
        let declared = spec.services.get(service).is_some_and(|s| s.environments.contains_key(src));
        if !declared {
            return Ok(None);
        }
        Ok(Some(self.store.succeeded_in_env(src, service, release_id).await?))
    }

    async fn latest_snapshot(&self, env: &str, policy: &str) -> Result<Option<String>> {
        let runs = self.store.finished_operations(Some(env), &["backup"], policy, 50).await?;
        Ok(runs.into_iter().filter(|o| o.status == OpStatus::Succeeded).find_map(|o| {
            o.outcome.and_then(|out| out.data.get("snapshot").and_then(|v| v.as_str().map(str::to_string)))
        }))
    }

    /// Résout le contexte, planifie et évalue les portes.
    pub(crate) async fn prepare(
        &self,
        spec: &Spec,
        p: &Principal,
        req: &OperationRequest,
        op_id: &str,
    ) -> Result<Prepared> {
        req.validate_basic()?;
        let env_name = req.environment.as_str();
        let env = spec.environment(env_name)?;
        let policy = spec.policy_for(env_name)?;
        let kind = req.action.kind();
        self.authorize(spec, p, kind.permission(), Some(env_name), req.action.service())?;
        let now = self.now();

        let mut resolved = Resolved { spec_hash: spec.hash(), ..Resolved::default() };
        let mut release: Option<Release> = None;
        let mut current: Option<Release> = None;
        let mut promoted_ok = None;
        let mut infra_summary: Option<String> = None;
        let mut snapshot: Option<String> = None;

        match &req.action {
            Action::Deploy { service, release: reference } => {
                resolved.target = Some(spec.service_in_env(service, env_name)?.target_name.to_string());
                let r = self.resolve_release(service, reference).await?;
                current = self.current_release(env_name, service).await?;
                if let Some(src) = &env.promote_from {
                    promoted_ok = self.promotion_status(spec, src, service, &r.id).await?;
                }
                release = Some(r);
            }
            Action::Rollback { service, to_release } => {
                resolved.target = Some(spec.service_in_env(service, env_name)?.target_name.to_string());
                current = self.current_release(env_name, service).await?;
                let cur = current
                    .as_ref()
                    .ok_or_else(|| CoreError::invalid(format!("aucun déploiement connu de {service} en {env_name}")))?;
                let target = match to_release {
                    Some(reference) => {
                        let r = self.resolve_release(service, reference).await?;
                        if !self.store.succeeded_in_env(env_name, service, &r.id).await? {
                            return Err(CoreError::invalid(
                                "seule une release déjà déployée avec succès dans cet environnement peut être rétablie",
                            )
                            .into());
                        }
                        r
                    }
                    None => {
                        let history = self
                            .store
                            .finished_operations(Some(env_name), &["deploy", "rollback"], service, 100)
                            .await?;
                        let previous = history
                            .iter()
                            .filter(|o| o.status == OpStatus::Succeeded)
                            .filter_map(|o| o.resolved.release_id.as_deref())
                            .find(|id| *id != cur.id)
                            .ok_or_else(|| CoreError::invalid("aucune release antérieure à rétablir"))?;
                        self.store
                            .get_release(previous)
                            .await?
                            .ok_or_else(|| CoreError::not_found(format!("release {previous}")))?
                    }
                };
                if target.id == cur.id {
                    return Err(CoreError::invalid(format!("{} est déjà en place", target.label())).into());
                }
                release = Some(target);
            }
            Action::Scale { service, .. } => {
                resolved.target = Some(spec.service_in_env(service, env_name)?.target_name.to_string());
            }
            Action::InfraApply { stack, plan_operation } => {
                let plan_op = self
                    .store
                    .get_operation(plan_operation)
                    .await?
                    .ok_or_else(|| CoreError::not_found(format!("opération {plan_operation}")))?
                    .value;
                if plan_op.kind() != OperationKind::InfraPlan
                    || plan_op.request.action.subject() != stack
                    || plan_op.environment() != env_name
                {
                    return Err(CoreError::invalid(format!(
                        "{plan_operation} n'est pas un plan de la stack {stack} en {env_name}"
                    ))
                    .into());
                }
                if plan_op.status != OpStatus::Succeeded {
                    return Err(CoreError::invalid("le plan d'infrastructure référencé n'a pas abouti").into());
                }
                if plan_op.finished_at.is_none_or(|t| now - t > chrono::TimeDelta::hours(24)) {
                    return Err(CoreError::invalid(
                        "plan d'infrastructure trop ancien (24 h max.) : relancez infra plan",
                    )
                    .into());
                }
                let data = plan_op.outcome.as_ref().map(|o| &o.data);
                let changes = data.and_then(|d| d.get("tf_changes")).and_then(serde_json::Value::as_u64).unwrap_or(0);
                if changes == 0 {
                    return Err(CoreError::invalid("le plan référencé ne contient aucun changement").into());
                }
                infra_summary =
                    data.and_then(|d| d.get("tf_plan_summary")).and_then(|v| v.as_str()).map(str::to_string);
            }
            Action::Restore { policy: name, snapshot: requested, .. } => {
                snapshot = match requested {
                    Some(s) => Some(s.clone()),
                    None => self.latest_snapshot(env_name, name).await?,
                };
            }
            Action::RestoreDrill { policy: name } => {
                snapshot = self.latest_snapshot(env_name, name).await?;
            }
            Action::Patch { target } => resolved.target = Some(target.clone()),
            Action::InfraPlan { .. } | Action::Backup { .. } | Action::Runbook { .. } => {}
        }
        if let Some(r) = &release {
            resolved.release_id = Some(r.id.clone());
            resolved.image_ref = Some(r.image_ref());
        }
        if let Some(c) = &current {
            resolved.previous_release_id = Some(c.id.clone());
            resolved.previous_image_ref = Some(c.image_ref());
        }
        resolved.snapshot.clone_from(&snapshot);

        let plan = planner::plan(&PlanContext {
            spec,
            request: req,
            operation_id: op_id,
            actor: &p.id,
            now,
            release: release.as_ref(),
            current: current.as_ref(),
            infra_plan_summary: infra_summary.as_deref(),
            snapshot: snapshot.as_deref(),
        })?;
        let open_incidents = self.open_incident_summaries().await?;
        let maintenance_active = self.store.active_maintenance(env_name, &now).await?.is_some();
        let report = gates::evaluate(&GateInput {
            env_name,
            env,
            policy,
            request: req,
            release: release.as_ref(),
            promoted_from_ok: promoted_ok,
            open_incidents: &open_incidents,
            maintenance_active,
            now,
        });
        Ok(Prepared { plan_hash: plan.hash(), description: plan.describe(), plan, report, resolved })
    }

    /// Aperçu (« dry-run ») : rien n'est enregistré ni exécuté.
    pub async fn preview(&self, p: &Principal, req: &OperationRequest) -> Result<Prepared> {
        let spec = self.spec();
        self.prepare(&spec, p, req, &format!("apercu-{}", Uuid::new_v4().simple())).await
    }

    pub async fn submit(&self, p: &Principal, req: OperationRequest) -> Result<Operation> {
        let spec = self.spec();
        let id = Uuid::now_v7().to_string();
        let prepared = self.prepare(&spec, p, &req, &id).await?;
        let now = self.now();
        let status = match prepared.report.decision {
            GateDecision::Reject => OpStatus::Rejected,
            GateDecision::AwaitApproval => OpStatus::AwaitingApproval,
            GateDecision::Proceed => OpStatus::Queued,
        };
        let failures: Vec<String> = prepared
            .report
            .results
            .iter()
            .filter(|g| g.status == GateStatus::Failed)
            .map(|g| format!("{} : {}", g.gate, g.message))
            .collect();
        let overridden: Vec<&str> = prepared
            .report
            .results
            .iter()
            .filter(|g| g.status == GateStatus::Overridden)
            .map(|g| g.gate.as_str())
            .collect();
        let op = Operation {
            id,
            status,
            requested_by: p.id.clone(),
            created_at: now,
            updated_at: now,
            started_at: None,
            finished_at: (status == OpStatus::Rejected).then_some(now),
            plan_hash: prepared.plan_hash,
            plan: prepared.plan,
            gates: prepared.report.results.clone(),
            required_approvals: prepared.report.required_approvals,
            approvals: Vec::new(),
            resolved: prepared.resolved,
            outcome: (status == OpStatus::Rejected).then(|| Outcome {
                message: format!("Refusée par les portes de contrôle : {}", failures.join(" ; ")),
                ..Outcome::default()
            }),
            request: req,
        };
        let audit = self.audit(
            p,
            "operation.submitted",
            &op_resource(&op),
            status.as_str(),
            json!({
                "kind": op.kind().as_str(),
                "environment": op.environment(),
                "subject": op.request.action.subject(),
                "plan_hash": op.plan_hash,
                "spec_hash": op.resolved.spec_hash,
                "release": op.resolved.release_id,
                "change_ticket": op.request.change_ticket,
                "emergency": op.request.emergency,
                "reason": op.request.reason,
                "gate_failures": failures,
                "gates_overridden": overridden,
            }),
        );
        self.store.insert_operation(&op, audit).await?;
        let (event, title) = match status {
            OpStatus::Rejected => {
                ("operation.rejected", format!("{} refusée : {}", op.kind().label(), op.request.action.subject()))
            }
            OpStatus::AwaitingApproval => (
                "operation.awaiting_approval",
                format!(
                    "Approbation requise : {} de {} en {}",
                    op.kind().label(),
                    op.request.action.subject(),
                    op.environment()
                ),
            ),
            _ => ("operation.queued", format!("{} planifiée : {}", op.kind().label(), op.request.action.subject())),
        };
        self.notify(op_event(&op, event, title));
        if op.request.emergency {
            let mut e = op_event(&op, "operation.emergency", format!("Procédure d'urgence déclenchée par {}", p.id));
            e.severity = Some(Severity::Sev3);
            self.notify(e);
        }
        Ok(op)
    }

    pub async fn get_operation(&self, p: &Principal, id: &str) -> Result<Operation> {
        let spec = self.spec();
        let op =
            self.store.get_operation(id).await?.ok_or_else(|| CoreError::not_found(format!("opération {id}")))?.value;
        self.authorize(&spec, p, Permission::Read, Some(op.environment()), op.request.action.service())?;
        Ok(op)
    }

    pub async fn list_operations(&self, p: &Principal, filter: &OperationFilter) -> Result<Vec<Operation>> {
        let spec = self.spec();
        self.authorize(&spec, p, Permission::Read, filter.environment.as_deref(), None)?;
        self.store.list_operations(filter).await
    }

    pub async fn operation_logs(&self, p: &Principal, id: &str, after: i64, limit: i64) -> Result<Vec<LogLine>> {
        self.get_operation(p, id).await?;
        self.store.logs(id, after, limit).await
    }

    /// Approbation ou refus (principe des quatre yeux, plan figé).
    pub async fn decide(
        &self,
        p: &Principal,
        id: &str,
        decision: ApprovalDecision,
        comment: &str,
    ) -> Result<Operation> {
        let spec = self.spec();
        for attempt in 0..3 {
            let Versioned { value: mut op, version } =
                self.store.get_operation(id).await?.ok_or_else(|| CoreError::not_found(format!("opération {id}")))?;
            self.authorize(&spec, p, Permission::Approve, Some(op.environment()), op.request.action.service())?;
            if op.resolved.spec_hash != spec.hash() && decision == ApprovalDecision::Approved {
                return Err(CoreError::Conflict(
                    "la spécification a changé depuis la planification : l'opération doit être soumise à nouveau"
                        .into(),
                )
                .into());
            }
            let now = self.now();
            let ready = op.decide(&p.id, decision, comment, now)?;
            let action = match decision {
                ApprovalDecision::Approved => "operation.approved",
                ApprovalDecision::Rejected => "operation.rejected",
            };
            let audit = self.audit(
                p,
                action,
                &op_resource(&op),
                op.status.as_str(),
                json!({"comment": comment, "plan_hash": op.plan_hash, "approvals": op.approvals_granted(), "required": op.required_approvals}),
            );
            match self.store.update_operation(&op, version, vec![audit], None).await {
                Ok(_) => {
                    let title = match (decision, ready) {
                        (ApprovalDecision::Rejected, _) => format!("Opération refusée par {}", p.id),
                        (_, true) => format!("Opération approuvée par {} : exécution planifiée", p.id),
                        (_, false) => {
                            format!("Approbation {}/{} par {}", op.approvals_granted(), op.required_approvals, p.id)
                        }
                    };
                    self.notify(op_event(&op, action, title));
                    return Ok(op);
                }
                Err(Error::Stale(_)) if attempt < 2 => continue,
                Err(e) => return Err(e),
            }
        }
        Err(Error::Stale(format!("opération {id}")))
    }

    /// Annulation par le demandeur ou un administrateur, avant exécution.
    pub async fn cancel(&self, p: &Principal, id: &str, reason: &str) -> Result<Operation> {
        let spec = self.spec();
        let Versioned { value: mut op, version } =
            self.store.get_operation(id).await?.ok_or_else(|| CoreError::not_found(format!("opération {id}")))?;
        let is_admin = socle_core::rbac::is_allowed(&spec, p, Permission::Admin, None, None);
        if op.requested_by != p.id && !is_admin {
            return Err(CoreError::forbidden("seul le demandeur ou un administrateur peut annuler").into());
        }
        let now = self.now();
        op.transition(OpStatus::Cancelled, now)?;
        op.outcome =
            Some(Outcome { message: format!("Annulée par {} : {}", p.id, reason.trim()), ..Outcome::default() });
        let audit = self.audit(p, "operation.cancelled", &op_resource(&op), "cancelled", json!({"reason": reason}));
        self.store.update_operation(&op, version, vec![audit], None).await?;
        Ok(op)
    }

    // -----------------------------------------------------------------------
    // Exécution
    // -----------------------------------------------------------------------

    /// Réserve et exécute la prochaine opération prête (boucle des workers).
    pub async fn run_next(&self) -> Result<Option<Operation>> {
        let now_ms = self.now_ms();
        let lease_ms = i64::try_from(self.cfg.lease.as_millis()).unwrap_or(120_000);
        let Some(id) = self.store.claim_operation(&self.cfg.worker_id, now_ms + lease_ms, now_ms, None).await? else {
            return Ok(None);
        };
        self.run_claimed(&id).await.map(Some)
    }

    /// Mode local : exécute immédiatement une opération prête.
    pub async fn run_now(&self, id: &str) -> Result<Operation> {
        let now_ms = self.now_ms();
        let lease_ms = i64::try_from(self.cfg.lease.as_millis()).unwrap_or(120_000);
        match self.store.claim_operation(&self.cfg.worker_id, now_ms + lease_ms, now_ms, Some(id)).await? {
            Some(_) => self.run_claimed(id).await,
            None => {
                let op = self.store.get_operation(id).await?.map(|v| v.value);
                Err(CoreError::Conflict(format!(
                    "l'opération {id} n'est pas prête à être exécutée (statut : {})",
                    op.map_or("inconnue", |o| o.status.as_str())
                ))
                .into())
            }
        }
    }

    async fn recheck(&self, spec: &Spec, op: &Operation) -> Result<Option<String>> {
        if op.resolved.spec_hash != spec.hash() {
            return Ok(Some("la spécification a changé depuis la planification".into()));
        }
        let env_name = op.environment();
        let env = spec.environment(env_name)?;
        let policy = spec.policy_for(env_name)?;
        let release = match &op.resolved.release_id {
            Some(id) => self.store.get_release(id).await?,
            None => None,
        };
        let promoted_ok = match (&env.promote_from, &release, op.kind()) {
            (Some(src), Some(r), OperationKind::Deploy) => {
                self.promotion_status(spec, src, op.request.action.subject(), &r.id).await?
            }
            _ => None,
        };
        let now = self.now();
        let open = self.open_incident_summaries().await?;
        let report = gates::evaluate(&GateInput {
            env_name,
            env,
            policy,
            request: &op.request,
            release: release.as_ref(),
            promoted_from_ok: promoted_ok,
            open_incidents: &open,
            maintenance_active: self.store.active_maintenance(env_name, &now).await?.is_some(),
            now,
        });
        let failures: Vec<String> = report
            .results
            .iter()
            .filter(|g| g.status == GateStatus::Failed)
            .map(|g| format!("{} : {}", g.gate, g.message))
            .collect();
        Ok((!failures.is_empty()).then(|| failures.join(" ; ")))
    }

    async fn run_steps(
        &self,
        steps: &[Step],
        op: &Operation,
        logger: &OpLogger,
        data: &mut StepData,
        phase: &str,
    ) -> std::result::Result<(), String> {
        for (i, step) in steps.iter().enumerate() {
            let label = format!("{phase} {}/{} — {}", i + 1, steps.len(), step.name);
            logger.log("info", &label, &step.action.describe()).await;
            let ctx = StepContext { operation_id: &op.id, label: &label, logger };
            match self.runner.run(step, &ctx).await {
                Ok(out) => {
                    data.extend(out);
                    logger.log("info", &label, "terminé").await;
                }
                Err(e) => {
                    logger.log("error", &label, &e).await;
                    return Err(format!("{} : {e}", step.name));
                }
            }
        }
        Ok(())
    }

    fn spawn_heartbeat(&self, op_id: String, holder: String) -> tokio::task::JoinHandle<()> {
        let store = self.store.clone();
        let clock = self.clock.clone();
        let worker = self.cfg.worker_id.clone();
        let lease = self.cfg.lease;
        let lease_ms = i64::try_from(lease.as_millis()).unwrap_or(120_000);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(lease / 3).await;
                let until = clock().timestamp_millis() + lease_ms;
                if let Err(e) = store.renew_lease(&op_id, &worker, until).await {
                    tracing::warn!(operation = %op_id, "renouvellement du bail impossible : {e}");
                }
                let _ = store.renew_locks(&holder, until).await;
            }
        })
    }

    async fn run_claimed(&self, id: &str) -> Result<Operation> {
        let spec = self.spec();
        let Versioned { value: mut op, mut version } =
            self.store.get_operation(id).await?.ok_or_else(|| CoreError::not_found(format!("opération {id}")))?;
        let system = Principal::system(&format!("worker:{}", self.cfg.worker_id));

        // 1. Intégrité du plan persisté, puis portes sensibles au temps.
        let problem = if op.plan.hash() != op.plan_hash {
            Some("intégrité du plan compromise (empreinte différente)".to_string())
        } else {
            self.recheck(&spec, &op).await?
        };
        if let Some(reason) = problem {
            return self
                .finish(
                    &spec,
                    op,
                    version,
                    &system,
                    OpStatus::Failed,
                    format!("Non exécutée : {reason}"),
                    false,
                    StepData::new(),
                    None,
                )
                .await;
        }

        // 2. Verrous (non bloquants ; nouvel essai différé si occupés).
        let holder = format!("{}:{}", self.cfg.worker_id, op.id);
        let now_ms = self.now_ms();
        let lease_ms = i64::try_from(self.cfg.lease.as_millis()).unwrap_or(120_000);
        if !self.store.try_locks(&op.plan.locks, &holder, &op.id, lease_ms, now_ms).await? {
            self.store.requeue_operation(&op.id, now_ms + 15_000).await?;
            let logger = OpLogger::new(self.store.clone(), &op.id).await?;
            logger
                .log("warn", "verrous", "une autre opération détient un verrou requis : nouvel essai dans 15 s")
                .await;
            logger.flush().await;
            return Ok(self.store.get_operation(id).await?.map_or(op, |v| v.value));
        }

        // 3. Démarrage.
        let started = self.now();
        op.started_at = Some(started);
        op.updated_at = started;
        let audit = self.audit(
            &system,
            "operation.started",
            &op_resource(&op),
            "running",
            json!({"worker": self.cfg.worker_id}),
        );
        version = self.store.update_operation(&op, version, vec![audit], None).await?;
        self.notify(op_event(
            &op,
            "operation.started",
            format!("{} démarrée : {}", op.kind().label(), op.request.action.subject()),
        ));
        let heartbeat = self.spawn_heartbeat(op.id.clone(), holder.clone());

        // 4. Exécution, vérification, compensation.
        let logger = OpLogger::new(self.store.clone(), &op.id).await?;
        logger.log("info", "début", &op.plan.summary).await;
        let mut data = StepData::new();
        let mut result = self.run_steps(&op.plan.steps, &op, &logger, &mut data, "exécution").await;
        if result.is_ok() && !op.plan.verify.is_empty() {
            result = self.run_steps(&op.plan.verify, &op, &logger, &mut data, "vérification").await;
        }
        let auto_rollback = spec.policy_for(op.environment()).map(|p| p.auto_rollback).unwrap_or(true);
        let (status, message, rolled_back) = match result {
            Ok(()) => (OpStatus::Succeeded, "Opération réussie".to_string(), false),
            Err(err) if auto_rollback && !op.plan.rollback.is_empty() => {
                logger.log("warn", "retour arrière", &format!("échec : {err} — retour arrière automatique")).await;
                match self.run_steps(&op.plan.rollback, &op, &logger, &mut data, "retour arrière").await {
                    Ok(()) => {
                        (OpStatus::RolledBack, format!("Échec ({err}) : état précédent rétabli automatiquement"), true)
                    }
                    Err(e2) => (
                        OpStatus::Failed,
                        format!("Échec ({err}) ; le retour arrière a également échoué : {e2}"),
                        false,
                    ),
                }
            }
            Err(err) => (OpStatus::Failed, format!("Échec : {err}"), false),
        };
        for file in &op.plan.cleanup {
            if socle_core::names::is_safe_relative_path(file) {
                let _ = tokio::fs::remove_file(self.cfg.repo_root.join(file)).await;
            }
        }
        logger.log(if status == OpStatus::Succeeded { "info" } else { "error" }, "fin", &message).await;
        logger.flush().await;
        heartbeat.abort();

        let result = self.finish(&spec, op, version, &system, status, message, rolled_back, data, Some(started)).await;
        self.store.release_locks(&holder).await?;
        result
    }

    /// Clôture une opération : statut terminal, résultat, déploiement,
    /// incident éventuel, audit et notifications.
    #[allow(clippy::too_many_arguments)]
    async fn finish(
        &self,
        spec: &Spec,
        mut op: Operation,
        version: i64,
        actor: &Principal,
        status: OpStatus,
        message: String,
        rolled_back: bool,
        mut data: StepData,
        started: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Operation> {
        let now = self.now();
        op.transition(status, now)?;
        if status == OpStatus::Succeeded {
            for (k, v) in &op.plan.outputs {
                data.insert(k.clone(), json!(v));
            }
        }
        if let Some(s) = started {
            data.insert("duration_secs".into(), json!((now - s).num_seconds()));
        }
        let mut outcome =
            Outcome { message, rolled_back, incident_id: None, data: data.into_iter().collect::<BTreeMap<_, _>>() };
        let kind = op.kind();
        let deployment = (status == OpStatus::Succeeded
            && matches!(kind, OperationKind::Deploy | OperationKind::Rollback))
        .then(|| {
            Some(Deployment {
                environment: op.environment().to_string(),
                service: op.request.action.subject().to_string(),
                release_id: op.resolved.release_id.clone()?,
                version: String::new(),
                image_ref: op.resolved.image_ref.clone()?,
                operation_id: op.id.clone(),
                deployed_by: op.requested_by.clone(),
                deployed_at: now,
            })
        })
        .flatten();
        let deployment = match deployment {
            Some(mut d) => {
                if let Some(r) = self.store.get_release(&d.release_id).await? {
                    d.version = r.version;
                }
                Some(d)
            }
            None => None,
        };

        let production = spec.environment(op.environment()).map(|e| e.tier == Tier::Production).unwrap_or(false);
        if status != OpStatus::Succeeded
            && production
            && matches!(
                kind,
                OperationKind::Deploy
                    | OperationKind::Rollback
                    | OperationKind::Scale
                    | OperationKind::InfraApply
                    | OperationKind::Restore
                    | OperationKind::Patch
                    | OperationKind::Runbook
            )
        {
            let incident = self
                .open_system_incident(
                    NewIncident {
                        title: format!(
                            "{} en échec : {} en {}",
                            kind.label(),
                            op.request.action.subject(),
                            op.environment()
                        ),
                        severity: if rolled_back { Severity::Sev3 } else { Severity::Sev2 },
                        environment: Some(op.environment().to_string()),
                        services: op.request.action.service().map(|s| vec![s.to_string()]).unwrap_or_default(),
                        summary: outcome.message.clone(),
                    },
                    IncidentSource::Operation { operation_id: op.id.clone() },
                    format!("op:{}", op.id),
                    Some(&op.id),
                )
                .await;
            match incident {
                Ok(i) => outcome.incident_id = Some(i.id),
                Err(e) => tracing::error!(operation = %op.id, "ouverture d'incident impossible : {e}"),
            }
        }
        let drift = kind == OperationKind::InfraPlan
            && status == OpStatus::Succeeded
            && op.requested_by.starts_with("system:")
            && outcome.data.get("tf_changes").and_then(serde_json::Value::as_u64).unwrap_or(0) > 0;
        op.outcome = Some(outcome);
        let audit = self.audit(
            actor,
            "operation.finished",
            &op_resource(&op),
            status.as_str(),
            json!({
                "message": op.outcome.as_ref().map(|o| o.message.clone()),
                "rolled_back": rolled_back,
                "incident": op.outcome.as_ref().and_then(|o| o.incident_id.clone()),
                "duration_secs": op.outcome.as_ref().and_then(|o| o.data.get("duration_secs").cloned()),
            }),
        );
        self.store.update_operation(&op, version, vec![audit], deployment.as_ref()).await?;

        let (event, title) = match status {
            OpStatus::Succeeded => {
                ("operation.succeeded", format!("{} réussie : {}", kind.label(), op.request.action.subject()))
            }
            OpStatus::RolledBack => (
                "operation.rolled_back",
                format!("{} annulée par retour arrière : {}", kind.label(), op.request.action.subject()),
            ),
            _ => ("operation.failed", format!("{} en échec : {}", kind.label(), op.request.action.subject())),
        };
        let mut e = op_event(&op, event, title);
        if let Some(o) = &op.outcome {
            e.text = o.message.clone();
        }
        self.notify(e);
        if drift {
            let mut e = op_event(
                &op,
                "drift.detected",
                format!("Dérive détectée sur la stack {} en {}", op.request.action.subject(), op.environment()),
            );
            e.severity = Some(Severity::Sev4);
            self.notify(e);
        }
        Ok(op)
    }

    /// Marque en échec les opérations dont le worker a disparu (bail expiré).
    pub async fn reap_expired(&self) -> Result<usize> {
        let spec = self.spec();
        let ids = self.store.expired_leases(self.now_ms()).await?;
        let system = Principal::system("reaper");
        let mut n = 0;
        for id in ids {
            let Some(Versioned { value: op, version }) = self.store.get_operation(&id).await? else {
                continue;
            };
            if op.status != OpStatus::Running {
                continue;
            }
            let res = self
                .finish(
                    &spec,
                    op,
                    version,
                    &system,
                    OpStatus::Failed,
                    "Exécution interrompue (perte du worker) : vérifiez l'état réel avant de relancer".into(),
                    false,
                    StepData::new(),
                    None,
                )
                .await;
            self.store.release_locks_for_operation(&id).await?;
            if res.is_ok() {
                n += 1;
            }
        }
        Ok(n)
    }
}
