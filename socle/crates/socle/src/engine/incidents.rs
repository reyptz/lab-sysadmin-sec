//! Incidents : ouverture manuelle ou automatique (alertes, opérations en
//! échec, RPO dépassé), cycle de vie, chronologie et postmortem.

use serde::{Deserialize, Serialize};
use serde_json::json;
use socle_core::incident::{
    AlertmanagerPayload, Incident, IncidentSource, IncidentStatus, NewIncident, Severity, signals_from_alertmanager,
};
use socle_core::rbac::{Permission, Principal};
use socle_core::{CoreError, names};
use uuid::Uuid;

use super::Engine;
use crate::error::{Error, Result};
use crate::model::Versioned;
use crate::notify::Event;

/// Modification des champs descriptifs d'un incident.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncidentPatch {
    #[serde(default)]
    pub severity: Option<Severity>,
    #[serde(default)]
    pub commander: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub root_cause: Option<String>,
    #[serde(default)]
    pub postmortem_url: Option<String>,
    #[serde(default)]
    pub action_items: Option<Vec<String>>,
}

fn incident_event(i: &Incident, kind: &str, title: String) -> Event {
    let mut e = Event::new(kind, title);
    e.environment.clone_from(&i.environment);
    e.severity = Some(i.severity);
    e.link = Some(format!("#/incidents/{}", i.id));
    e.text = i.summary.clone();
    e.dedup_key = Some(format!("socle-incident-{}", i.id));
    e.resolve = matches!(i.status, IncidentStatus::Resolved | IncidentStatus::Closed);
    e.data =
        json!({"incident": i.id, "status": i.status.as_str(), "severity": i.severity.as_str(), "services": i.services});
    e
}

impl Engine {
    fn validate_incident(&self, input: &NewIncident) -> Result<()> {
        let spec = self.spec();
        let title = input.title.trim();
        if title.is_empty() || title.len() > 200 {
            return Err(CoreError::invalid("titre d'incident obligatoire (200 caractères max.)").into());
        }
        if let Some(env) = &input.environment {
            spec.environment(env)?;
        }
        for s in &input.services {
            spec.service(s)?;
        }
        Ok(())
    }

    pub async fn open_incident(&self, p: &Principal, input: NewIncident) -> Result<Incident> {
        let spec = self.spec();
        self.authorize(
            &spec,
            p,
            Permission::ManageIncidents,
            input.environment.as_deref(),
            input.services.first().map(String::as_str),
        )?;
        self.validate_incident(&input)?;
        let incident =
            Incident::open(Uuid::now_v7().to_string(), input, IncidentSource::Manual, None, &p.id, self.now());
        self.persist_new_incident(p, &incident).await?;
        Ok(incident)
    }

    async fn persist_new_incident(&self, p: &Principal, incident: &Incident) -> Result<()> {
        let audit = self.audit(
            p,
            "incident.opened",
            &format!("incident/{}", incident.id),
            incident.severity.as_str(),
            json!({"title": incident.title, "environment": incident.environment, "services": incident.services, "source": incident.source}),
        );
        self.store.insert_incident(incident, audit).await?;
        self.notify(incident_event(
            incident,
            "incident.opened",
            format!("[{}] {}", incident.severity.as_str().to_uppercase(), incident.title),
        ));
        Ok(())
    }

    /// Ouverture par socle lui-même, dédupliquée par empreinte.
    pub(crate) async fn open_system_incident(
        &self,
        input: NewIncident,
        source: IncidentSource,
        fingerprint: String,
        operation: Option<&str>,
    ) -> Result<Incident> {
        if let Some(existing) = self.store.find_active_incident(&fingerprint).await? {
            return Ok(existing.value);
        }
        let system = Principal::system("incidents");
        let now = self.now();
        let mut incident =
            Incident::open(Uuid::now_v7().to_string(), input, source, Some(fingerprint), &system.id, now);
        if let Some(op) = operation {
            incident.link_operation(op, &system.id, now);
        }
        self.persist_new_incident(&system, &incident).await?;
        Ok(incident)
    }

    async fn mutate_incident<F>(&self, p: &Principal, id: &str, action: &str, mut f: F) -> Result<Incident>
    where
        F: FnMut(&mut Incident) -> Result<()>,
    {
        let spec = self.spec();
        for attempt in 0..3 {
            let Versioned { value: mut incident, version } =
                self.store.get_incident(id).await?.ok_or_else(|| CoreError::not_found(format!("incident {id}")))?;
            self.authorize(&spec, p, Permission::ManageIncidents, incident.environment.as_deref(), None)?;
            let before = incident.status;
            f(&mut incident)?;
            let audit = self.audit(
                p,
                action,
                &format!("incident/{id}"),
                incident.status.as_str(),
                json!({"from": before.as_str(), "to": incident.status.as_str(), "severity": incident.severity.as_str()}),
            );
            match self.store.update_incident(&incident, version, audit).await {
                Ok(_) => {
                    if before != incident.status {
                        let kind = match incident.status {
                            IncidentStatus::Resolved => "incident.resolved",
                            IncidentStatus::Closed => "incident.closed",
                            _ => "incident.updated",
                        };
                        self.notify(incident_event(
                            &incident,
                            kind,
                            format!("Incident {} : {}", incident.status.as_str(), incident.title),
                        ));
                    }
                    return Ok(incident);
                }
                Err(Error::Stale(_)) if attempt < 2 => continue,
                Err(e) => return Err(e),
            }
        }
        Err(Error::Stale(format!("incident {id}")))
    }

    pub async fn transition_incident(
        &self,
        p: &Principal,
        id: &str,
        to: IncidentStatus,
        note: Option<&str>,
    ) -> Result<Incident> {
        let now = self.now();
        let who = p.id.clone();
        self.mutate_incident(p, id, "incident.status", |i| Ok(i.transition(to, &who, note, now)?)).await
    }

    pub async fn add_incident_note(&self, p: &Principal, id: &str, note: &str) -> Result<Incident> {
        let now = self.now();
        let who = p.id.clone();
        self.mutate_incident(p, id, "incident.note", |i| Ok(i.add_note(&who, note, now)?)).await
    }

    pub async fn update_incident(&self, p: &Principal, id: &str, patch: IncidentPatch) -> Result<Incident> {
        let now = self.now();
        let who = p.id.clone();
        if let Some(url) = &patch.postmortem_url
            && !names::is_http_url(url)
        {
            return Err(CoreError::invalid("lien de postmortem invalide").into());
        }
        self.mutate_incident(p, id, "incident.updated", |i| {
            if let Some(s) = patch.severity {
                i.set_severity(s, &who, now);
            }
            if let Some(c) = &patch.commander {
                i.commander = Some(c.clone());
            }
            if let Some(s) = &patch.summary {
                i.summary.clone_from(s);
            }
            if let Some(r) = &patch.root_cause {
                i.root_cause = Some(r.clone());
            }
            if let Some(u) = &patch.postmortem_url {
                i.postmortem_url = Some(u.clone());
            }
            if let Some(items) = &patch.action_items {
                i.action_items.clone_from(items);
            }
            i.updated_at = now;
            Ok(())
        })
        .await
    }

    pub async fn get_incident(&self, p: &Principal, id: &str) -> Result<Incident> {
        let spec = self.spec();
        let incident =
            self.store.get_incident(id).await?.ok_or_else(|| CoreError::not_found(format!("incident {id}")))?.value;
        self.authorize(&spec, p, Permission::Read, incident.environment.as_deref(), None)?;
        Ok(incident)
    }

    pub async fn list_incidents(&self, p: &Principal, active_only: bool, limit: i64) -> Result<Vec<Incident>> {
        let spec = self.spec();
        self.authorize(&spec, p, Permission::Read, None, None)?;
        self.store.list_incidents(active_only, limit).await
    }

    /// Récepteur Alertmanager : ouvre, déduplique et résout les incidents.
    pub async fn ingest_alerts(&self, p: &Principal, payload: &AlertmanagerPayload) -> Result<Vec<String>> {
        let spec = self.spec();
        self.authorize(&spec, p, Permission::ManageIncidents, None, None)?;
        let mut touched = Vec::new();
        for signal in signals_from_alertmanager(payload) {
            let environment = signal.environment.filter(|e| spec.environments.contains_key(e));
            let service = signal.service.filter(|s| spec.services.contains_key(s));
            if signal.firing {
                if !signal.severity.at_least(self.cfg.incident_min_severity) {
                    continue;
                }
                let incident = self
                    .open_system_incident(
                        NewIncident {
                            title: signal.title.chars().take(200).collect(),
                            severity: signal.severity,
                            environment,
                            services: service.into_iter().collect(),
                            summary: signal.description,
                        },
                        IncidentSource::Alert { alertname: signal.alertname },
                        signal.fingerprint,
                        None,
                    )
                    .await?;
                touched.push(incident.id);
            } else if let Some(Versioned { value: incident, .. }) =
                self.store.find_active_incident(&signal.fingerprint).await?
            {
                let system = Principal::system("alertmanager");
                let res = self
                    .transition_incident(
                        &system,
                        &incident.id,
                        IncidentStatus::Resolved,
                        Some("alerte résolue par Alertmanager"),
                    )
                    .await;
                if res.is_ok() {
                    touched.push(incident.id);
                }
            }
        }
        Ok(touched)
    }
}
