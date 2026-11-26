//! Portes de contrôle (policy-as-code). Deux familles :
//!
//! * **portes de sécurité** (tests, vulnérabilités, signature, SBOM) : jamais
//!   contournables, même en urgence — elles garantissent l'intégrité de la
//!   chaîne d'approvisionnement ;
//! * **portes de processus** (ticket, fenêtre, gel, incident, maintenance,
//!   promotion) : contournables par la procédure d'urgence si la politique
//!   l'autorise, avec trace dans l'audit.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::incident::Severity;
use crate::operation::{GateResult, GateStatus, OperationKind, OperationRequest};
use crate::release::Release;
use crate::spec::{Environment, Policy, Tier};
use crate::windows;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenIncident {
    pub id: String,
    pub severity: Severity,
    pub environment: Option<String>,
    pub services: Vec<String>,
}

pub struct GateInput<'a> {
    pub env_name: &'a str,
    pub env: &'a Environment,
    pub policy: &'a Policy,
    pub request: &'a OperationRequest,
    pub release: Option<&'a Release>,
    /// La release a-t-elle déjà réussi dans l'environnement source ?
    pub promoted_from_ok: Option<bool>,
    pub open_incidents: &'a [OpenIncident],
    pub maintenance_active: bool,
    pub now: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateDecision {
    Proceed,
    AwaitApproval,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateReport {
    pub results: Vec<GateResult>,
    pub required_approvals: u32,
    pub decision: GateDecision,
}

impl GateReport {
    pub fn failures(&self) -> Vec<&GateResult> {
        self.results.iter().filter(|r| r.status == GateStatus::Failed).collect()
    }
}

struct Collector {
    results: Vec<(GateResult, bool)>,
}

impl Collector {
    fn pass(&mut self, gate: &str, msg: impl Into<String>) {
        self.push(gate, GateStatus::Passed, msg, false);
    }
    fn warn(&mut self, gate: &str, msg: impl Into<String>) {
        self.push(gate, GateStatus::Warning, msg, false);
    }
    /// Échec d'une porte de sécurité (non contournable).
    fn block(&mut self, gate: &str, msg: impl Into<String>) {
        self.push(gate, GateStatus::Failed, msg, false);
    }
    /// Échec d'une porte de processus (contournable en urgence).
    fn fail(&mut self, gate: &str, msg: impl Into<String>) {
        self.push(gate, GateStatus::Failed, msg, true);
    }
    fn push(&mut self, gate: &str, status: GateStatus, msg: impl Into<String>, overridable: bool) {
        self.results.push((GateResult { gate: gate.to_string(), status, message: msg.into() }, overridable));
    }
}

/// Nombre d'approbations requises selon la nature de l'opération.
pub fn required_approvals(policy: &Policy, kind: OperationKind, tier: Tier, emergency: bool) -> u32 {
    match kind {
        // Changements standards pré-approuvés (lecture seule ou retour à un
        // état connu et sain).
        OperationKind::InfraPlan | OperationKind::Backup | OperationKind::RestoreDrill | OperationKind::Rollback => 0,
        // Une restauration écrase des données : au moins un second regard en production.
        OperationKind::Restore if tier == Tier::Production => policy.approvals.max(1),
        _ if emergency => policy.approvals.min(1),
        _ => policy.approvals,
    }
}

pub fn evaluate(input: &GateInput<'_>) -> GateReport {
    let kind = input.request.action.kind();
    let policy = input.policy;
    let mut c = Collector { results: Vec::new() };

    if input.request.emergency && !policy.allow_emergency {
        c.block("urgence", "la procédure d'urgence n'est pas autorisée sur cet environnement");
    }

    // --- Portes de sécurité (chaîne d'approvisionnement) -------------------
    if kind == OperationKind::Deploy {
        match input.release {
            None => c.block("release", "release introuvable"),
            Some(r) => {
                let ev = &r.evidence;
                if policy.require_tests {
                    match ev.tests_passed {
                        Some(true) => c.pass("tests", "tests automatisés réussis"),
                        Some(false) => c.block("tests", "les tests automatisés ont échoué"),
                        None => c.block("tests", "aucune preuve d'exécution des tests"),
                    }
                }
                if policy.max_critical.is_some() || policy.max_high.is_some() {
                    match &ev.vulnerabilities {
                        None => c.block("vulnérabilités", "aucun rapport d'analyse de vulnérabilités"),
                        Some(v) => {
                            let crit_ok = policy.max_critical.is_none_or(|m| v.critical <= m);
                            let high_ok = policy.max_high.is_none_or(|m| v.high <= m);
                            let msg = format!("{} critique(s), {} haute(s)", v.critical, v.high);
                            if crit_ok && high_ok {
                                c.pass("vulnérabilités", msg);
                            } else {
                                c.block("vulnérabilités", format!("{msg} : seuil dépassé"));
                            }
                        }
                    }
                }
                if policy.require_signature {
                    if ev.signature_verified == Some(true) {
                        c.pass("signature", "signature de l'image vérifiée");
                    } else {
                        c.block("signature", "image non signée ou signature non vérifiée");
                    }
                }
                if policy.require_sbom {
                    if ev.sbom_uri.is_some() {
                        c.pass("sbom", "SBOM disponible");
                    } else {
                        c.block("sbom", "SBOM absent");
                    }
                }
                if ev.secret_scan_passed == Some(false) {
                    c.block("secrets", "des secrets ont été détectés dans le code source");
                }
                if ev.iac_scan_passed == Some(false) {
                    c.warn("iac", "l'analyse IaC a remonté des écarts");
                }
            }
        }
        if policy.enforce_promotion
            && let Some(src) = &input.env.promote_from
        {
            match input.promoted_from_ok {
                Some(true) => c.pass("promotion", format!("release validée en {src}")),
                Some(false) => {
                    c.fail("promotion", format!("cette release n'a pas encore été déployée avec succès en {src}"))
                }
                // Service non déclaré dans l'environnement source : la
                // promotion ne s'applique pas (signalé, non bloquant).
                None => c.warn("promotion", format!("service absent de {src} : promotion non applicable")),
            }
        }
    }

    // --- Portes de processus ----------------------------------------------
    let needs_ticket = matches!(
        kind,
        OperationKind::Deploy
            | OperationKind::Scale
            | OperationKind::InfraApply
            | OperationKind::Restore
            | OperationKind::Patch
            | OperationKind::Runbook
    );
    if policy.require_change_ticket && needs_ticket {
        match &input.request.change_ticket {
            None => c.fail("ticket", "référence de ticket de changement obligatoire"),
            Some(t) => {
                let ok = policy
                    .change_ticket_pattern
                    .as_deref()
                    .and_then(|p| regex::Regex::new(p).ok())
                    .is_none_or(|re| re.is_match(t));
                if ok {
                    c.pass("ticket", format!("ticket {t}"));
                } else {
                    c.fail("ticket", format!("le ticket « {t} » ne respecte pas le format attendu"));
                }
            }
        }
    }

    let windowed = matches!(
        kind,
        OperationKind::Deploy
            | OperationKind::Scale
            | OperationKind::InfraApply
            | OperationKind::Patch
            | OperationKind::Runbook
    );
    if policy.enforce_change_windows && windowed {
        if let Some(f) = windows::active_freeze(&input.env.freezes, input.now) {
            c.fail("gel", format!("gel des changements en cours : {}", f.reason));
        }
        if windows::windows_open(&input.env.change_windows, input.now) {
            c.pass("fenêtre", "dans une fenêtre de changement");
        } else {
            let next = windows::next_opening(&input.env.change_windows, input.now)
                .map(|t| format!(" ; prochaine ouverture : {}", crate::format_ts(&t)))
                .unwrap_or_default();
            c.fail("fenêtre", format!("hors fenêtre de changement{next}"));
        }
    }

    if let Some(threshold) = policy.block_on_incident
        && matches!(kind, OperationKind::Deploy | OperationKind::Scale | OperationKind::InfraApply)
    {
        let service = input.request.action.service();
        let blocking: Vec<&OpenIncident> = input
            .open_incidents
            .iter()
            .filter(|i| i.severity.at_least(threshold))
            .filter(|i| i.environment.as_deref().is_none_or(|e| e == input.env_name))
            .filter(|i| i.services.is_empty() || service.is_none_or(|s| i.services.iter().any(|x| x == s)))
            .collect();
        if blocking.is_empty() {
            c.pass("incidents", "aucun incident bloquant");
        } else {
            let ids: Vec<&str> = blocking.iter().map(|i| i.id.as_str()).collect();
            c.fail("incidents", format!("incident(s) en cours : {}", ids.join(", ")));
        }
    }

    if input.maintenance_active
        && matches!(kind, OperationKind::Deploy | OperationKind::Scale | OperationKind::InfraApply)
    {
        c.fail(
            "maintenance",
            "environnement en maintenance : seuls les correctifs, sauvegardes et retours arrière sont admis",
        );
    }

    // --- Urgence : contournement tracé des portes de processus --------------
    let emergency = input.request.emergency && policy.allow_emergency;
    let results: Vec<GateResult> = c
        .results
        .into_iter()
        .map(|(mut r, overridable)| {
            if emergency && overridable && r.status == GateStatus::Failed {
                r.status = GateStatus::Overridden;
                r.message = format!("{} (contourné : procédure d'urgence)", r.message);
            }
            r
        })
        .collect();

    let required = required_approvals(policy, kind, input.env.tier, input.request.emergency);
    let decision = if results.iter().any(|r| r.status == GateStatus::Failed) {
        GateDecision::Reject
    } else if required > 0 {
        GateDecision::AwaitApproval
    } else {
        GateDecision::Proceed
    };
    GateReport { results, required_approvals: required, decision }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::Action;
    use crate::release::{Evidence, VulnSummary};
    use crate::spec::tests::sample;

    fn release(evidence: Evidence) -> Release {
        Release {
            id: "r1".into(),
            service: "erp-app".into(),
            version: "1.2.0".into(),
            image: "ghcr.io/acme/erp-app".into(),
            digest: format!("sha256:{}", "b".repeat(64)),
            git_sha: None,
            created_by: "ci".into(),
            created_at: Utc::now(),
            evidence,
        }
    }

    fn good_evidence() -> Evidence {
        Evidence {
            tests_passed: Some(true),
            vulnerabilities: Some(VulnSummary::default()),
            signature_verified: Some(true),
            sbom_uri: Some("oci://sbom".into()),
            ..Evidence::default()
        }
    }

    fn request(emergency: bool, ticket: Option<&str>) -> OperationRequest {
        OperationRequest {
            environment: "prod".into(),
            action: Action::Deploy { service: "erp-app".into(), release: "1.2.0".into() },
            reason: "Correctif critique de facturation".into(),
            change_ticket: ticket.map(str::to_string),
            emergency,
        }
    }

    // Mercredi 25/11/2026 10:00 Paris (ouvert) et vendredi (fermé).
    fn open_time() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-11-25T09:00:00Z").expect("date").with_timezone(&Utc)
    }
    fn closed_time() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-11-27T09:00:00Z").expect("date").with_timezone(&Utc)
    }

    fn eval(
        req: &OperationRequest,
        rel: &Release,
        promoted: bool,
        now: DateTime<Utc>,
        incidents: &[OpenIncident],
    ) -> GateReport {
        let spec = sample();
        let env = spec.environments["prod"].clone();
        let policy = spec.policies["strict"].clone();
        evaluate(&GateInput {
            env_name: "prod",
            env: &env,
            policy: &policy,
            request: req,
            release: Some(rel),
            promoted_from_ok: Some(promoted),
            open_incidents: incidents,
            maintenance_active: false,
            now,
        })
    }

    #[test]
    fn compliant_deploy_awaits_one_approval() {
        let r = eval(&request(false, Some("CHG-1042")), &release(good_evidence()), true, open_time(), &[]);
        assert_eq!(r.decision, GateDecision::AwaitApproval, "{:#?}", r.results);
        assert_eq!(r.required_approvals, 1);
    }

    #[test]
    fn security_gates_cannot_be_bypassed() {
        let mut ev = good_evidence();
        ev.vulnerabilities = Some(VulnSummary { critical: 1, ..VulnSummary::default() });
        ev.signature_verified = None;
        let r = eval(&request(true, Some("CHG-1")), &release(ev), true, open_time(), &[]);
        assert_eq!(r.decision, GateDecision::Reject);
        let failed: Vec<&str> = r.failures().iter().map(|g| g.gate.as_str()).collect();
        assert!(failed.contains(&"vulnérabilités") && failed.contains(&"signature"), "{failed:?}");
    }

    #[test]
    fn process_gates_fail_outside_window_and_without_ticket() {
        let r = eval(&request(false, None), &release(good_evidence()), false, closed_time(), &[]);
        assert_eq!(r.decision, GateDecision::Reject);
        let failed: Vec<&str> = r.failures().iter().map(|g| g.gate.as_str()).collect();
        assert!(failed.contains(&"ticket"));
        assert!(failed.contains(&"fenêtre"));
        assert!(failed.contains(&"promotion"));
        let fenetre = r.results.iter().find(|g| g.gate == "fenêtre").expect("fenêtre");
        assert!(fenetre.message.contains("prochaine ouverture"));
    }

    #[test]
    fn emergency_overrides_process_gates_with_single_approval() {
        let incidents = [OpenIncident {
            id: "inc-7".into(),
            severity: Severity::Sev1,
            environment: Some("prod".into()),
            services: vec![],
        }];
        let r = eval(&request(true, None), &release(good_evidence()), false, closed_time(), &incidents);
        assert_eq!(r.decision, GateDecision::AwaitApproval, "{:#?}", r.results);
        assert_eq!(r.required_approvals, 1);
        assert!(r.results.iter().any(|g| g.status == GateStatus::Overridden && g.gate == "incidents"));
    }

    /// Un service absent de l'environnement source (ex. déployé seulement en
    /// prod) ne doit pas être bloqué à vie par la porte de promotion.
    #[test]
    fn promotion_not_applicable_when_service_absent_from_source() {
        let spec = sample();
        let req = request(false, Some("CHG-1042"));
        let rel = release(good_evidence());
        let r = evaluate(&GateInput {
            env_name: "prod",
            env: &spec.environments["prod"],
            policy: &spec.policies["strict"],
            request: &req,
            release: Some(&rel),
            promoted_from_ok: None,
            open_incidents: &[],
            maintenance_active: false,
            now: open_time(),
        });
        assert_eq!(r.decision, GateDecision::AwaitApproval, "{:#?}", r.results);
        assert!(r.results.iter().any(|g| g.gate == "promotion" && g.status == GateStatus::Warning));
    }

    #[test]
    fn rollback_needs_no_approval() {
        let spec = sample();
        let req = OperationRequest {
            environment: "prod".into(),
            action: Action::Rollback { service: "erp-app".into(), to_release: None },
            reason: String::new(),
            change_ticket: None,
            emergency: false,
        };
        let r = evaluate(&GateInput {
            env_name: "prod",
            env: &spec.environments["prod"],
            policy: &spec.policies["strict"],
            request: &req,
            release: None,
            promoted_from_ok: None,
            open_incidents: &[],
            maintenance_active: true,
            now: closed_time(),
        });
        assert_eq!(r.decision, GateDecision::Proceed, "{:#?}", r.results);
    }
}
