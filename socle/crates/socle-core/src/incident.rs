//! Gestion des incidents : cycle de vie, chronologie, indicateurs (MTTA,
//! MTTR), corrélation des alertes Alertmanager et génération du postmortem.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{CoreError, Result, format_ts};

/// Gravité : `Sev1` est la plus élevée (l'ordre dérivé place `Sev1` en premier).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Sev1,
    Sev2,
    Sev3,
    Sev4,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Sev1 => "sev1",
            Severity::Sev2 => "sev2",
            Severity::Sev3 => "sev3",
            Severity::Sev4 => "sev4",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "sev1" => Some(Severity::Sev1),
            "sev2" => Some(Severity::Sev2),
            "sev3" => Some(Severity::Sev3),
            "sev4" => Some(Severity::Sev4),
            _ => None,
        }
    }
    /// Vrai si `self` est au moins aussi grave que `threshold`.
    pub fn at_least(self, threshold: Severity) -> bool {
        self <= threshold
    }
    /// Correspondance avec l'API PagerDuty Events v2.
    pub fn pagerduty(self) -> &'static str {
        match self {
            Severity::Sev1 => "critical",
            Severity::Sev2 => "error",
            Severity::Sev3 => "warning",
            Severity::Sev4 => "info",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncidentStatus {
    Open,
    Acknowledged,
    Mitigated,
    Resolved,
    Closed,
}

impl IncidentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            IncidentStatus::Open => "open",
            IncidentStatus::Acknowledged => "acknowledged",
            IncidentStatus::Mitigated => "mitigated",
            IncidentStatus::Resolved => "resolved",
            IncidentStatus::Closed => "closed",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "open" => Some(IncidentStatus::Open),
            "acknowledged" => Some(IncidentStatus::Acknowledged),
            "mitigated" => Some(IncidentStatus::Mitigated),
            "resolved" => Some(IncidentStatus::Resolved),
            "closed" => Some(IncidentStatus::Closed),
            _ => None,
        }
    }
    /// Un incident « actif » n'est ni résolu ni clos.
    pub fn is_active(self) -> bool {
        matches!(self, IncidentStatus::Open | IncidentStatus::Acknowledged | IncidentStatus::Mitigated)
    }
    pub fn can_transition_to(self, next: IncidentStatus) -> bool {
        use IncidentStatus as S;
        matches!(
            (self, next),
            (S::Open, S::Acknowledged | S::Mitigated | S::Resolved)
                | (S::Acknowledged, S::Mitigated | S::Resolved)
                | (S::Mitigated, S::Acknowledged | S::Resolved)
                | (S::Resolved, S::Closed | S::Open)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IncidentSource {
    Manual,
    Alert { alertname: String },
    Operation { operation_id: String },
    Backup { policy: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimelineKind {
    Created,
    Status,
    Severity,
    Note,
    Link,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub at: DateTime<Utc>,
    pub author: String,
    pub kind: TimelineKind,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Incident {
    pub id: String,
    pub title: String,
    pub severity: Severity,
    pub status: IncidentStatus,
    #[serde(default)]
    pub environment: Option<String>,
    #[serde(default)]
    pub services: Vec<String>,
    #[serde(default)]
    pub commander: Option<String>,
    pub source: IncidentSource,
    #[serde(default)]
    pub fingerprint: Option<String>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub acknowledged_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub mitigated_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub resolved_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub closed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub root_cause: Option<String>,
    #[serde(default)]
    pub postmortem_url: Option<String>,
    #[serde(default)]
    pub action_items: Vec<String>,
    #[serde(default)]
    pub linked_operations: Vec<String>,
    #[serde(default)]
    pub timeline: Vec<TimelineEntry>,
}

/// Paramètres d'ouverture d'un incident.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewIncident {
    pub title: String,
    pub severity: Severity,
    #[serde(default)]
    pub environment: Option<String>,
    #[serde(default)]
    pub services: Vec<String>,
    #[serde(default)]
    pub summary: String,
}

impl Incident {
    pub fn open(
        id: String,
        input: NewIncident,
        source: IncidentSource,
        fingerprint: Option<String>,
        author: &str,
        now: DateTime<Utc>,
    ) -> Self {
        let message = format!("Incident ouvert ({}) : {}", input.severity.as_str(), input.title);
        Self {
            id,
            title: input.title,
            severity: input.severity,
            status: IncidentStatus::Open,
            environment: input.environment,
            services: input.services,
            commander: None,
            source,
            fingerprint,
            created_by: author.to_string(),
            created_at: now,
            updated_at: now,
            acknowledged_at: None,
            mitigated_at: None,
            resolved_at: None,
            closed_at: None,
            summary: input.summary,
            root_cause: None,
            postmortem_url: None,
            action_items: Vec::new(),
            linked_operations: Vec::new(),
            timeline: vec![TimelineEntry { at: now, author: author.to_string(), kind: TimelineKind::Created, message }],
        }
    }

    /// Un postmortem est exigé pour les incidents Sev1 et Sev2.
    pub fn postmortem_required(&self) -> bool {
        self.severity.at_least(Severity::Sev2)
    }

    pub fn transition(
        &mut self,
        next: IncidentStatus,
        author: &str,
        note: Option<&str>,
        now: DateTime<Utc>,
    ) -> Result<()> {
        if !self.status.can_transition_to(next) {
            return Err(CoreError::InvalidTransition(format!(
                "incident {} : {} → {}",
                self.id,
                self.status.as_str(),
                next.as_str()
            )));
        }
        if next == IncidentStatus::Closed
            && self.postmortem_required()
            && self.root_cause.is_none()
            && self.postmortem_url.is_none()
        {
            return Err(CoreError::invalid(
                "un incident Sev1/Sev2 ne peut être clos qu'avec une cause racine ou un lien de postmortem",
            ));
        }
        match next {
            IncidentStatus::Acknowledged => {
                self.acknowledged_at.get_or_insert(now);
                if self.commander.is_none() {
                    self.commander = Some(author.to_string());
                }
            }
            IncidentStatus::Mitigated => {
                self.acknowledged_at.get_or_insert(now);
                self.mitigated_at.get_or_insert(now);
            }
            IncidentStatus::Resolved => {
                self.acknowledged_at.get_or_insert(now);
                self.resolved_at = Some(now);
            }
            IncidentStatus::Closed => self.closed_at = Some(now),
            IncidentStatus::Open => {
                self.resolved_at = None;
                self.closed_at = None;
            }
        }
        let mut message = format!("Statut : {} → {}", self.status.as_str(), next.as_str());
        if let Some(note) = note.filter(|n| !n.trim().is_empty()) {
            let _ = write!(message, " — {}", note.trim());
        }
        self.status = next;
        self.push(author, TimelineKind::Status, message, now);
        Ok(())
    }

    pub fn add_note(&mut self, author: &str, note: &str, now: DateTime<Utc>) -> Result<()> {
        let note = note.trim();
        if note.is_empty() {
            return Err(CoreError::invalid("note vide"));
        }
        if note.len() > 8_000 {
            return Err(CoreError::invalid("note trop longue (8000 caractères max.)"));
        }
        self.push(author, TimelineKind::Note, note.to_string(), now);
        Ok(())
    }

    pub fn set_severity(&mut self, severity: Severity, author: &str, now: DateTime<Utc>) {
        if severity != self.severity {
            let message = format!("Gravité : {} → {}", self.severity.as_str(), severity.as_str());
            self.severity = severity;
            self.push(author, TimelineKind::Severity, message, now);
        }
    }

    pub fn link_operation(&mut self, operation_id: &str, author: &str, now: DateTime<Utc>) {
        if !self.linked_operations.iter().any(|o| o == operation_id) {
            self.linked_operations.push(operation_id.to_string());
            self.push(author, TimelineKind::Link, format!("Opération liée : {operation_id}"), now);
        }
    }

    fn push(&mut self, author: &str, kind: TimelineKind, message: String, now: DateTime<Utc>) {
        self.timeline.push(TimelineEntry { at: now, author: author.to_string(), kind, message });
        self.updated_at = now;
    }

    /// Délai d'acquittement (Mean Time To Acknowledge, pour cet incident).
    pub fn time_to_acknowledge(&self) -> Option<chrono::TimeDelta> {
        self.acknowledged_at.map(|t| t - self.created_at)
    }

    /// Délai de résolution (Mean Time To Resolve, pour cet incident).
    pub fn time_to_resolve(&self) -> Option<chrono::TimeDelta> {
        self.resolved_at.map(|t| t - self.created_at)
    }

    /// Postmortem « sans recherche de coupable », en Markdown.
    pub fn render_postmortem(&self) -> String {
        let mut md = String::new();
        let _ = writeln!(md, "# Postmortem — {}\n", self.title);
        let _ = writeln!(md, "| Champ | Valeur |\n|---|---|");
        let _ = writeln!(md, "| Incident | `{}` |", self.id);
        let _ = writeln!(md, "| Gravité | {} |", self.severity.as_str().to_uppercase());
        let _ = writeln!(md, "| Statut | {} |", self.status.as_str());
        let _ = writeln!(md, "| Environnement | {} |", self.environment.as_deref().unwrap_or("—"));
        let _ = writeln!(
            md,
            "| Services | {} |",
            if self.services.is_empty() { "—".to_string() } else { self.services.join(", ") }
        );
        let _ = writeln!(md, "| Responsable | {} |", self.commander.as_deref().unwrap_or("—"));
        let _ = writeln!(md, "| Ouvert le | {} |", format_ts(&self.created_at));
        let fmt_delta =
            |d: Option<chrono::TimeDelta>| d.map(|d| format!("{} min", d.num_minutes())).unwrap_or_else(|| "—".into());
        let _ = writeln!(md, "| Délai d'acquittement | {} |", fmt_delta(self.time_to_acknowledge()));
        let _ = writeln!(md, "| Délai de résolution | {} |\n", fmt_delta(self.time_to_resolve()));
        let _ = writeln!(md, "## Résumé\n\n{}\n", or_todo(&self.summary));
        let _ = writeln!(md, "## Cause racine\n\n{}\n", or_todo(self.root_cause.as_deref().unwrap_or_default()));
        let _ = writeln!(md, "## Chronologie\n");
        for entry in &self.timeline {
            let _ = writeln!(md, "- `{}` **{}** — {}", format_ts(&entry.at), entry.author, entry.message);
        }
        let _ = writeln!(md, "\n## Opérations liées\n");
        if self.linked_operations.is_empty() {
            let _ = writeln!(md, "Aucune.");
        }
        for op in &self.linked_operations {
            let _ = writeln!(md, "- `{op}`");
        }
        let _ = writeln!(md, "\n## Actions correctives\n");
        if self.action_items.is_empty() {
            let _ = writeln!(md, "- [ ] _À définir en revue postmortem._");
        }
        for item in &self.action_items {
            let _ = writeln!(md, "- [ ] {item}");
        }
        let _ = writeln!(
            md,
            "\n> Postmortem sans recherche de coupable : on analyse les systèmes et les processus, pas les personnes."
        );
        md
    }
}

fn or_todo(s: &str) -> &str {
    if s.trim().is_empty() { "_À compléter._" } else { s }
}

// ---------------------------------------------------------------------------
// Alertmanager
// ---------------------------------------------------------------------------

/// Charge utile du récepteur webhook d'Alertmanager (version 4).
#[derive(Debug, Clone, Deserialize)]
pub struct AlertmanagerPayload {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub alerts: Vec<AlertmanagerAlert>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AlertmanagerAlert {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
    #[serde(default)]
    pub annotations: BTreeMap<String, String>,
    #[serde(default)]
    pub fingerprint: Option<String>,
}

/// Alerte normalisée, prête à être corrélée avec un incident.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertSignal {
    pub fingerprint: String,
    pub firing: bool,
    pub severity: Severity,
    pub alertname: String,
    pub title: String,
    pub description: String,
    pub environment: Option<String>,
    pub service: Option<String>,
}

pub fn alert_severity(label: Option<&str>) -> Severity {
    match label.map(str::to_ascii_lowercase).as_deref() {
        Some("sev1" | "page" | "emergency") => Severity::Sev1,
        Some("sev2" | "critical") => Severity::Sev2,
        Some("sev3" | "warning" | "major") => Severity::Sev3,
        _ => Severity::Sev4,
    }
}

pub fn signals_from_alertmanager(payload: &AlertmanagerPayload) -> Vec<AlertSignal> {
    payload
        .alerts
        .iter()
        .map(|a| {
            let alertname = a.labels.get("alertname").cloned().unwrap_or_else(|| "alerte".into());
            let fingerprint = a.fingerprint.clone().filter(|f| !f.is_empty()).unwrap_or_else(|| {
                let canon = serde_json::to_vec(&a.labels).unwrap_or_default();
                hex::encode(&Sha256::digest(&canon)[..8])
            });
            let pick = |keys: &[&str]| keys.iter().find_map(|k| a.labels.get(*k).cloned());
            AlertSignal {
                fingerprint: format!("alert:{fingerprint}"),
                firing: a.status != "resolved",
                severity: alert_severity(a.labels.get("severity").map(String::as_str)),
                title: a.annotations.get("summary").cloned().unwrap_or_else(|| alertname.clone()),
                description: a.annotations.get("description").cloned().unwrap_or_default(),
                environment: pick(&["environment", "env"]),
                service: pick(&["service", "app"]),
                alertname,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn incident(sev: Severity) -> Incident {
        Incident::open(
            "inc-1".into(),
            NewIncident {
                title: "API facturation indisponible".into(),
                severity: sev,
                environment: Some("prod".into()),
                services: vec!["erp-app".into()],
                summary: String::new(),
            },
            IncidentSource::Manual,
            None,
            "alice",
            Utc::now(),
        )
    }

    #[test]
    fn lifecycle_and_metrics() {
        let mut i = incident(Severity::Sev3);
        let t0 = i.created_at;
        i.transition(IncidentStatus::Acknowledged, "bob", None, t0 + chrono::TimeDelta::minutes(4)).expect("ack");
        assert_eq!(i.commander.as_deref(), Some("bob"));
        i.transition(IncidentStatus::Resolved, "bob", Some("rollback"), t0 + chrono::TimeDelta::minutes(30))
            .expect("resolve");
        assert_eq!(i.time_to_acknowledge().map(|d| d.num_minutes()), Some(4));
        assert_eq!(i.time_to_resolve().map(|d| d.num_minutes()), Some(30));
        i.transition(IncidentStatus::Closed, "bob", None, t0 + chrono::TimeDelta::hours(1))
            .expect("close sev3 sans postmortem");
        assert!(i.transition(IncidentStatus::Open, "bob", None, Utc::now()).is_err());
    }

    #[test]
    fn sev2_requires_postmortem_to_close() {
        let mut i = incident(Severity::Sev2);
        i.transition(IncidentStatus::Resolved, "bob", None, Utc::now()).expect("resolve");
        assert!(i.transition(IncidentStatus::Closed, "bob", None, Utc::now()).is_err());
        i.root_cause = Some("Migration de schéma non rétrocompatible".into());
        i.transition(IncidentStatus::Closed, "bob", None, Utc::now()).expect("close");
        let md = i.render_postmortem();
        assert!(md.contains("Migration de schéma"));
        assert!(md.contains("Chronologie"));
    }

    #[test]
    fn alertmanager_mapping() {
        let payload: AlertmanagerPayload = serde_json::from_value(serde_json::json!({
            "status": "firing",
            "alerts": [{
                "status": "firing",
                "labels": {"alertname": "ErrorBudgetBurn", "severity": "critical", "env": "prod", "service": "erp-app"},
                "annotations": {"summary": "Consommation rapide du budget d'erreur"},
                "fingerprint": "abc123"
            }]
        }))
        .expect("payload");
        let signals = signals_from_alertmanager(&payload);
        assert_eq!(signals.len(), 1);
        let s = &signals[0];
        assert!(s.firing);
        assert_eq!(s.severity, Severity::Sev2);
        assert_eq!(s.environment.as_deref(), Some("prod"));
        assert_eq!(s.service.as_deref(), Some("erp-app"));
        assert_eq!(s.fingerprint, "alert:abc123");
    }
}
