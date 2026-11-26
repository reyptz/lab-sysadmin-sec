//! Journal d'audit infalsifiable : chaque événement contient l'empreinte
//! SHA-256 du précédent. Toute modification, suppression ou insertion a
//! posteriori casse la chaîne et est détectée par `verify_chain`
//! (ISO 27001 A.8.15, SOC 2 CC7.2).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::format_ts;

pub const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditEvent {
    pub seq: i64,
    pub ts: DateTime<Utc>,
    pub actor: String,
    pub action: String,
    pub resource: String,
    pub outcome: String,
    pub details: serde_json::Value,
    pub prev_hash: String,
    pub hash: String,
}

/// Événement à sceller (numéro de séquence et chaînage attribués à l'écriture).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditDraft {
    pub actor: String,
    pub action: String,
    pub resource: String,
    pub outcome: String,
    pub details: serde_json::Value,
}

impl AuditDraft {
    pub fn new(
        actor: impl Into<String>,
        action: impl Into<String>,
        resource: impl Into<String>,
        outcome: impl Into<String>,
        details: serde_json::Value,
    ) -> Self {
        Self { actor: actor.into(), action: action.into(), resource: resource.into(), outcome: outcome.into(), details }
    }

    pub fn seal(self, seq: i64, ts: DateTime<Utc>, prev_hash: &str) -> AuditEvent {
        let ts = crate::truncate_micros(ts);
        let hash =
            compute_hash(seq, &ts, &self.actor, &self.action, &self.resource, &self.outcome, &self.details, prev_hash);
        AuditEvent {
            seq,
            ts,
            actor: self.actor,
            action: self.action,
            resource: self.resource,
            outcome: self.outcome,
            details: self.details,
            prev_hash: prev_hash.to_string(),
            hash,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn compute_hash(
    seq: i64,
    ts: &DateTime<Utc>,
    actor: &str,
    action: &str,
    resource: &str,
    outcome: &str,
    details: &serde_json::Value,
    prev_hash: &str,
) -> String {
    #[derive(Serialize)]
    struct Canonical<'a> {
        seq: i64,
        ts: String,
        actor: &'a str,
        action: &'a str,
        resource: &'a str,
        outcome: &'a str,
        details: &'a serde_json::Value,
        prev_hash: &'a str,
    }
    let canonical = Canonical { seq, ts: format_ts(ts), actor, action, resource, outcome, details, prev_hash };
    let bytes = serde_json::to_vec(&canonical).unwrap_or_default();
    hex::encode(Sha256::digest(&bytes))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainReport {
    pub valid: bool,
    pub events_checked: u64,
    pub head_seq: Option<i64>,
    pub head_hash: Option<String>,
    #[serde(default)]
    pub broken_at: Option<i64>,
    #[serde(default)]
    pub reason: Option<String>,
}

/// Vérifie un segment de chaîne. `expected_prev` est l'empreinte qui précède
/// le premier événement (`GENESIS_HASH` pour une vérification complète) et
/// `expected_first_seq` son numéro attendu.
pub fn verify_chain(events: &[AuditEvent], expected_prev: &str, expected_first_seq: i64) -> ChainReport {
    let mut prev = expected_prev.to_string();
    let mut checked = 0u64;
    for (next_seq, e) in (expected_first_seq..).zip(events) {
        let fail = |reason: String| ChainReport {
            valid: false,
            events_checked: checked,
            head_seq: None,
            head_hash: None,
            broken_at: Some(e.seq),
            reason: Some(reason),
        };
        if e.seq != next_seq {
            return fail(format!("séquence {} attendue, {} trouvée (événement supprimé ou inséré)", next_seq, e.seq));
        }
        if e.prev_hash != prev {
            return fail("chaînage rompu : l'empreinte précédente ne correspond pas".into());
        }
        let recomputed =
            compute_hash(e.seq, &e.ts, &e.actor, &e.action, &e.resource, &e.outcome, &e.details, &e.prev_hash);
        if recomputed != e.hash {
            return fail("contenu modifié : l'empreinte recalculée diffère".into());
        }
        prev.clone_from(&e.hash);
        checked += 1;
    }
    ChainReport {
        valid: true,
        events_checked: checked,
        head_seq: events.last().map(|e| e.seq),
        head_hash: events.last().map(|e| e.hash.clone()),
        broken_at: None,
        reason: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chain(n: i64) -> Vec<AuditEvent> {
        let mut prev = GENESIS_HASH.to_string();
        (1..=n)
            .map(|seq| {
                let e = AuditDraft::new(
                    "alice",
                    "operation.submitted",
                    format!("op/{seq}"),
                    "ok",
                    serde_json::json!({"n": seq}),
                )
                .seal(seq, Utc::now(), &prev);
                prev.clone_from(&e.hash);
                e
            })
            .collect()
    }

    #[test]
    fn valid_chain() {
        let c = chain(5);
        let r = verify_chain(&c, GENESIS_HASH, 1);
        assert!(r.valid, "{r:?}");
        assert_eq!(r.events_checked, 5);
        assert_eq!(r.head_seq, Some(5));
    }

    #[test]
    fn tampering_is_detected() {
        let mut c = chain(5);
        c[2].actor = "mallory".into();
        let r = verify_chain(&c, GENESIS_HASH, 1);
        assert!(!r.valid);
        assert_eq!(r.broken_at, Some(3));

        let mut c = chain(5);
        c.remove(1);
        let r = verify_chain(&c, GENESIS_HASH, 1);
        assert!(!r.valid);
        assert_eq!(r.broken_at, Some(3));
    }

    #[test]
    fn roundtrip_through_json_keeps_hash() {
        let c = chain(3);
        let text = serde_json::to_string(&c).expect("json");
        let back: Vec<AuditEvent> = serde_json::from_str(&text).expect("json");
        assert!(verify_chain(&back, GENESIS_HASH, 1).valid);
    }
}
