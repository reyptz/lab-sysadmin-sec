//! Noyau métier de **socle**, le plan de contrôle du cycle de vie des
//! déploiements et des infrastructures.
//!
//! Ce crate ne fait aucune entrée/sortie : il décrit le modèle déclaratif
//! (`socle.yaml`), évalue les politiques (portes DevSecOps, RBAC, fenêtres de
//! changement), produit des plans d'exécution déterministes et scelle le
//! journal d'audit. Toute la logique est donc testable unitairement et
//! partagée à l'identique par la CLI, l'API et la console.

pub mod audit;
pub mod backup;
pub mod duration;
pub mod error;
pub mod gates;
pub mod incident;
pub mod kustomize;
pub mod names;
pub mod operation;
pub mod planner;
pub mod rbac;
pub mod release;
pub mod spec;
pub mod windows;

pub use error::{CoreError, Result};

/// Horodatage tronqué à la microseconde : c'est la précision conservée en
/// base, ce qui garantit qu'une valeur relue est identique à la valeur écrite
/// (indispensable au chaînage du journal d'audit).
pub fn now_micros() -> chrono::DateTime<chrono::Utc> {
    truncate_micros(chrono::Utc::now())
}

/// Tronque un horodatage à la microseconde.
pub fn truncate_micros(ts: chrono::DateTime<chrono::Utc>) -> chrono::DateTime<chrono::Utc> {
    use chrono::SubsecRound;
    ts.trunc_subsecs(6)
}

/// Format canonique des horodatages persistés (RFC 3339, UTC, microsecondes).
/// L'ordre lexicographique de ce format est l'ordre chronologique.
pub fn format_ts(ts: &chrono::DateTime<chrono::Utc>) -> String {
    ts.to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
}

/// Empreinte SHA-256 (hexadécimale) d'une valeur sérialisable.
pub fn sha256_json<T: serde::Serialize>(value: &T) -> String {
    use sha2::{Digest, Sha256};
    // La sérialisation d'une structure Rust en JSON est déterministe (ordre
    // des champs fixe, BTreeMap triées) : l'empreinte est donc stable.
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    hex::encode(Sha256::digest(&bytes))
}
