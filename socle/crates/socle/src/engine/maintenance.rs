//! Fenêtres de maintenance : gel des déploiements sur l'environnement et
//! silences Alertmanager posés puis levés automatiquement.

use serde_json::json;
use socle_core::CoreError;
use socle_core::duration::HumanDuration;
use socle_core::rbac::{Permission, Principal};
use uuid::Uuid;

use super::Engine;
use crate::error::Result;
use crate::executor::{create_silence, expire_silence};
use crate::model::{MaintenanceStatus, MaintenanceWindow, Versioned};
use crate::notify::Event;

impl Engine {
    pub async fn start_maintenance(
        &self,
        p: &Principal,
        environment: &str,
        duration: HumanDuration,
        reason: &str,
    ) -> Result<MaintenanceWindow> {
        let spec = self.spec();
        let env = spec.environment(environment)?;
        self.authorize(&spec, p, Permission::ManageMaintenance, Some(environment), None)?;
        if duration.is_zero() || duration.as_secs() > 24 * 3_600 {
            return Err(CoreError::invalid("durée de maintenance comprise entre 1 s et 24 h").into());
        }
        if reason.trim().len() < 5 {
            return Err(CoreError::invalid("motif de maintenance obligatoire").into());
        }
        let now = self.now();
        if self.store.active_maintenance(environment, &now).await?.is_some() {
            return Err(CoreError::Conflict(format!("une maintenance est déjà en cours en {environment}")).into());
        }
        let id = Uuid::now_v7().to_string();
        let mut silence_ids = Vec::new();
        if let Some(am) = &spec.observability.alertmanager_url {
            let mut matchers = env.alert_labels.clone();
            if matchers.is_empty() {
                matchers.insert("environment".into(), environment.into());
            }
            match create_silence(
                &self.http,
                am,
                &matchers,
                duration.as_secs(),
                &format!("socle : maintenance {id} — {reason}"),
            )
            .await
            {
                Ok(sid) => silence_ids.push(sid),
                Err(e) => tracing::warn!("maintenance {id} : {e}"),
            }
        }
        let window = MaintenanceWindow {
            id,
            environment: environment.to_string(),
            reason: reason.trim().to_string(),
            starts_at: now,
            ends_at: now + duration.as_chrono(),
            created_by: p.id.clone(),
            status: MaintenanceStatus::Active,
            silence_ids,
            ended_at: None,
            ended_by: None,
        };
        let audit = self.audit(
            p,
            "maintenance.started",
            &format!("maintenance/{}", window.id),
            "active",
            json!({"environment": environment, "until": window.ends_at, "reason": window.reason, "silences": window.silence_ids}),
        );
        self.store.insert_maintenance(&window, audit).await?;
        let mut e = Event::new(
            "maintenance.started",
            format!("Maintenance en {environment} jusqu'à {}", socle_core::format_ts(&window.ends_at)),
        );
        e.environment = Some(environment.into());
        e.text = window.reason.clone();
        self.notify(e);
        Ok(window)
    }

    pub async fn end_maintenance(&self, p: &Principal, id: &str) -> Result<MaintenanceWindow> {
        let spec = self.spec();
        let Versioned { value: mut window, version } =
            self.store.get_maintenance(id).await?.ok_or_else(|| CoreError::not_found(format!("maintenance {id}")))?;
        self.authorize(&spec, p, Permission::ManageMaintenance, Some(&window.environment), None)?;
        if window.status == MaintenanceStatus::Ended {
            return Ok(window);
        }
        if let Some(am) = &spec.observability.alertmanager_url {
            for sid in &window.silence_ids {
                if let Err(e) = expire_silence(&self.http, am, sid).await {
                    tracing::warn!("maintenance {id} : {e}");
                }
            }
        }
        let now = self.now();
        window.status = MaintenanceStatus::Ended;
        window.ended_at = Some(now);
        window.ended_by = Some(p.id.clone());
        if window.ends_at > now {
            window.ends_at = now;
        }
        let audit = self.audit(
            p,
            "maintenance.ended",
            &format!("maintenance/{id}"),
            "ended",
            json!({"environment": window.environment}),
        );
        self.store.update_maintenance(&window, version, audit).await?;
        let mut e = Event::new("maintenance.ended", format!("Fin de maintenance en {}", window.environment));
        e.environment = Some(window.environment.clone());
        self.notify(e);
        Ok(window)
    }

    pub async fn list_maintenance(&self, p: &Principal, active_only: bool) -> Result<Vec<MaintenanceWindow>> {
        let spec = self.spec();
        self.authorize(&spec, p, Permission::Read, None, None)?;
        self.store.list_maintenance(active_only, 200).await
    }

    pub(crate) async fn expire_maintenance(&self) -> Result<()> {
        let system = Principal::system("scheduler");
        for id in self.store.expired_maintenance(&self.now()).await? {
            if let Err(e) = self.end_maintenance(&system, &id).await {
                tracing::warn!("clôture de la maintenance {id} : {e}");
            }
        }
        Ok(())
    }
}
