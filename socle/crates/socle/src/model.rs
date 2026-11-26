//! Entités propres au moteur (persistées par le [`crate::store::Store`]).

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Valeur accompagnée de sa version (verrouillage optimiste).
#[derive(Debug, Clone)]
pub struct Versioned<T> {
    pub value: T,
    pub version: i64,
}

/// Release actuellement en place pour un service dans un environnement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deployment {
    pub environment: String,
    pub service: String,
    pub release_id: String,
    pub version: String,
    pub image_ref: String,
    pub operation_id: String,
    pub deployed_by: String,
    pub deployed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogLine {
    pub seq: i64,
    pub ts: DateTime<Utc>,
    pub level: String,
    pub step: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiToken {
    pub id: String,
    pub name: String,
    pub subject: String,
    pub teams: BTreeSet<String>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub revoked: bool,
    #[serde(default)]
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaintenanceStatus {
    Active,
    Ended,
}

impl MaintenanceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            MaintenanceStatus::Active => "active",
            MaintenanceStatus::Ended => "ended",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenanceWindow {
    pub id: String,
    pub environment: String,
    pub reason: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub created_by: String,
    pub status: MaintenanceStatus,
    #[serde(default)]
    pub silence_ids: Vec<String>,
    #[serde(default)]
    pub ended_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub ended_by: Option<String>,
}

/// Filtre de liste des opérations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OperationFilter {
    #[serde(default)]
    pub environment: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
}
