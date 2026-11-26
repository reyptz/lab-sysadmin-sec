//! Conformité des sauvegardes : respect du RPO, exercices de restauration
//! mesurés contre le RTO, et rétention GFS unifiée (même sémantique que
//! restic, appliquée aussi aux outils qui n'en disposent pas).

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Datelike, Timelike, Utc};
use serde::{Deserialize, Serialize};

use crate::spec::{BackupPolicy, Retention};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunKind {
    Backup,
    Drill,
}

/// Résumé d'une exécution (issu des opérations `backup` et `restore_drill`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupRun {
    pub kind: RunKind,
    pub succeeded: bool,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DrillStatus {
    pub every_secs: u64,
    pub last_success: Option<DateTime<Utc>>,
    pub last_duration_secs: Option<i64>,
    /// La dernière restauration réussie a-t-elle tenu le RTO ?
    pub within_rto: Option<bool>,
    pub overdue: bool,
    pub next_due: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupStatus {
    pub policy: String,
    pub environment: String,
    pub source: String,
    pub rpo_secs: u64,
    pub rto_secs: u64,
    pub last_success: Option<DateTime<Utc>>,
    pub last_attempt: Option<DateTime<Utc>>,
    pub age_secs: Option<i64>,
    pub rpo_compliant: bool,
    pub next_due: DateTime<Utc>,
    pub drill: Option<DrillStatus>,
}

impl BackupStatus {
    /// Conformité globale : RPO tenu et exercices à jour et dans le RTO.
    pub fn compliant(&self) -> bool {
        self.rpo_compliant && self.drill.as_ref().is_none_or(|d| !d.overdue && d.within_rto != Some(false))
    }
}

pub fn status(name: &str, policy: &BackupPolicy, runs: &[BackupRun], now: DateTime<Utc>) -> BackupStatus {
    let backups = runs.iter().filter(|r| r.kind == RunKind::Backup);
    let last_success = backups.clone().filter(|r| r.succeeded).map(|r| r.finished_at).max();
    let last_attempt = backups.map(|r| r.started_at).max();
    let age = last_success.map(|t| (now - t).num_seconds());
    let rpo = i64::try_from(policy.rpo.as_secs()).unwrap_or(i64::MAX);
    let next_due = last_attempt.map_or(now, |t| t + policy.every.as_chrono());

    let drill = policy.drill.as_ref().map(|d| {
        let drills: Vec<&BackupRun> = runs.iter().filter(|r| r.kind == RunKind::Drill).collect();
        let last_ok = drills.iter().filter(|r| r.succeeded).max_by_key(|r| r.finished_at);
        let last_try = drills.iter().map(|r| r.started_at).max();
        let duration = last_ok.map(|r| (r.finished_at - r.started_at).num_seconds());
        let rto = i64::try_from(policy.rto.as_secs()).unwrap_or(i64::MAX);
        DrillStatus {
            every_secs: d.every.as_secs(),
            last_success: last_ok.map(|r| r.finished_at),
            last_duration_secs: duration,
            within_rto: duration.map(|s| s <= rto),
            overdue: last_ok.is_none_or(|r| now - r.finished_at > d.every.as_chrono()),
            next_due: last_try.map_or(now, |t| t + d.every.as_chrono()),
        }
    });

    BackupStatus {
        policy: name.to_string(),
        environment: policy.environment.clone(),
        source: policy.source.kind_name().to_string(),
        rpo_secs: policy.rpo.as_secs(),
        rto_secs: policy.rto.as_secs(),
        last_success,
        last_attempt,
        age_secs: age,
        rpo_compliant: age.is_some_and(|a| a <= rpo),
        next_due,
        drill,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub time: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionDecision {
    /// Instantanés conservés, avec les règles qui les retiennent.
    pub keep: BTreeMap<String, Vec<String>>,
    pub prune: Vec<String>,
}

/// Rétention GFS : pour chaque règle, on garde l'instantané le plus récent
/// de chacun des N derniers « paniers » (heure, jour, semaine ISO, mois,
/// année) qui en contiennent au moins un.
pub fn retention_decision(snapshots: &[Snapshot], r: &Retention) -> RetentionDecision {
    let mut sorted: Vec<&Snapshot> = snapshots.iter().collect();
    sorted.sort_by(|a, b| b.time.cmp(&a.time).then_with(|| a.id.cmp(&b.id)));

    let mut keep: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for s in sorted.iter().take(r.last as usize) {
        keep.entry(s.id.clone()).or_default().push("last".into());
    }

    type BucketFn = fn(&DateTime<Utc>) -> (i32, u32, u32, u32);
    let rules: [(&str, u32, BucketFn); 5] = [
        ("hourly", r.hourly, |t| (t.year(), t.ordinal(), t.hour(), 0)),
        ("daily", r.daily, |t| (t.year(), t.ordinal(), 0, 0)),
        ("weekly", r.weekly, |t| {
            let w = t.iso_week();
            (w.year(), w.week(), 0, 0)
        }),
        ("monthly", r.monthly, |t| (t.year(), t.month(), 0, 0)),
        ("yearly", r.yearly, |t| (t.year(), 0, 0, 0)),
    ];
    for (label, count, bucket) in rules {
        let mut remaining = count;
        let mut last_bucket = None;
        for s in &sorted {
            if remaining == 0 {
                break;
            }
            let b = bucket(&s.time);
            if last_bucket != Some(b) {
                keep.entry(s.id.clone()).or_default().push(label.into());
                last_bucket = Some(b);
                remaining -= 1;
            }
        }
    }

    let kept: BTreeSet<&String> = keep.keys().collect();
    let prune = sorted.iter().filter(|s| !kept.contains(&s.id)).map(|s| s.id.clone()).collect();
    RetentionDecision { keep, prune }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::tests::sample;

    fn t(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).expect("date").with_timezone(&Utc)
    }

    #[test]
    fn rpo_compliance() {
        let spec = sample();
        let policy = &spec.backups["erp-files"]; // every 15m, rpo 15m, rto 1h, drill 7d
        let now = t("2026-11-25T12:00:00Z");
        let empty = status("erp-files", policy, &[], now);
        assert!(!empty.rpo_compliant);
        assert_eq!(empty.next_due, now);

        let runs = vec![
            BackupRun {
                kind: RunKind::Backup,
                succeeded: true,
                started_at: t("2026-11-25T11:49:00Z"),
                finished_at: t("2026-11-25T11:50:00Z"),
            },
            BackupRun {
                kind: RunKind::Backup,
                succeeded: false,
                started_at: t("2026-11-25T11:58:00Z"),
                finished_at: t("2026-11-25T11:59:00Z"),
            },
            BackupRun {
                kind: RunKind::Drill,
                succeeded: true,
                started_at: t("2026-11-22T02:00:00Z"),
                finished_at: t("2026-11-22T02:40:00Z"),
            },
        ];
        let s = status("erp-files", policy, &runs, now);
        assert!(s.rpo_compliant, "dernière réussite il y a 10 min");
        assert_eq!(s.age_secs, Some(600));
        assert_eq!(s.next_due, t("2026-11-25T12:13:00Z"));
        let d = s.drill.as_ref().expect("exercice");
        assert_eq!(d.last_duration_secs, Some(2400));
        assert_eq!(d.within_rto, Some(true));
        assert!(!d.overdue);
        assert!(s.compliant());

        let later = status("erp-files", policy, &runs, t("2026-11-25T12:20:00Z"));
        assert!(!later.rpo_compliant, "30 min > RPO de 15 min");
    }

    #[test]
    fn gfs_retention() {
        let snaps: Vec<Snapshot> = (0..40)
            .map(|d| Snapshot { id: format!("s{d:02}"), time: t("2026-11-25T02:00:00Z") - chrono::TimeDelta::days(d) })
            .collect();
        let r = Retention { last: 2, daily: 7, weekly: 4, monthly: 2, ..Retention::default() };
        let decision = retention_decision(&snaps, &r);
        // 7 jours consécutifs (s00..s06) + semaines + mois.
        for d in 0..7 {
            assert!(decision.keep.contains_key(&format!("s{d:02}")), "s{d:02}");
        }
        assert!(decision.keep["s00"].contains(&"last".to_string()));
        assert!(decision.prune.contains(&"s39".to_string()) || decision.keep.contains_key("s39"));
        assert_eq!(decision.keep.len() + decision.prune.len(), 40);
        assert!(decision.keep.len() < 20);
    }
}
