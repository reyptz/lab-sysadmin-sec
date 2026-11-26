//! Fenêtres de changement et périodes de gel, évaluées dans le fuseau horaire
//! de l'environnement (heure d'été comprise).

use chrono::{DateTime, Datelike, FixedOffset, NaiveTime, TimeZone, Utc, Weekday};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Day {
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
    Sun,
}

impl Day {
    pub fn weekday(self) -> Weekday {
        match self {
            Day::Mon => Weekday::Mon,
            Day::Tue => Weekday::Tue,
            Day::Wed => Weekday::Wed,
            Day::Thu => Weekday::Thu,
            Day::Fri => Weekday::Fri,
            Day::Sat => Weekday::Sat,
            Day::Sun => Weekday::Sun,
        }
    }
}

/// Créneau hebdomadaire pendant lequel les changements sont autorisés.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeWindow {
    pub days: Vec<Day>,
    /// Heure de début, format `HH:MM`.
    pub start: String,
    /// Heure de fin (exclue), format `HH:MM`.
    pub end: String,
    /// Fuseau IANA, ex. `Europe/Paris`.
    pub timezone: String,
}

fn parse_hm(s: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(s, "%H:%M").ok()
}

impl ChangeWindow {
    pub fn validate(&self) -> Result<(), String> {
        if self.days.is_empty() {
            return Err("fenêtre de changement sans jour".into());
        }
        let start = parse_hm(&self.start).ok_or_else(|| format!("heure invalide « {} » (HH:MM)", self.start))?;
        let end = parse_hm(&self.end).ok_or_else(|| format!("heure invalide « {} » (HH:MM)", self.end))?;
        if start >= end {
            return Err(format!("fenêtre {}–{} : le début doit précéder la fin", self.start, self.end));
        }
        self.timezone.parse::<Tz>().map_err(|_| format!("fuseau horaire inconnu « {} »", self.timezone))?;
        Ok(())
    }

    fn parts(&self) -> Option<(NaiveTime, NaiveTime, Tz)> {
        Some((parse_hm(&self.start)?, parse_hm(&self.end)?, self.timezone.parse::<Tz>().ok()?))
    }

    pub fn contains(&self, now: DateTime<Utc>) -> bool {
        let Some((start, end, tz)) = self.parts() else {
            return false;
        };
        let local = now.with_timezone(&tz);
        let t = local.time();
        self.days.iter().any(|d| d.weekday() == local.weekday()) && t >= start && t < end
    }

    /// Prochaine ouverture strictement postérieure à `now` (sur 8 jours).
    pub fn next_opening(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let (start, _, tz) = self.parts()?;
        let today = now.with_timezone(&tz).date_naive();
        (0..8)
            .filter_map(|offset| today.checked_add_days(chrono::Days::new(offset)))
            .filter(|date| self.days.iter().any(|d| d.weekday() == date.weekday()))
            .filter_map(|date| tz.from_local_datetime(&date.and_time(start)).earliest())
            .map(|dt| dt.with_timezone(&Utc))
            .find(|dt| *dt > now)
    }
}

/// Période de gel : aucun changement planifié (hors urgence) entre `from` et `to`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Freeze {
    pub from: DateTime<FixedOffset>,
    pub to: DateTime<FixedOffset>,
    pub reason: String,
}

impl Freeze {
    pub fn validate(&self) -> Result<(), String> {
        if self.from >= self.to {
            return Err(format!("gel « {} » : `from` doit précéder `to`", self.reason));
        }
        Ok(())
    }
    pub fn contains(&self, now: DateTime<Utc>) -> bool {
        now >= self.from && now < self.to
    }
}

/// Une liste vide de fenêtres signifie « toujours ouvert ».
pub fn windows_open(windows: &[ChangeWindow], now: DateTime<Utc>) -> bool {
    windows.is_empty() || windows.iter().any(|w| w.contains(now))
}

pub fn next_opening(windows: &[ChangeWindow], now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    windows.iter().filter_map(|w| w.next_opening(now)).min()
}

pub fn active_freeze(freezes: &[Freeze], now: DateTime<Utc>) -> Option<&Freeze> {
    freezes.iter().find(|f| f.contains(now))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn office_hours() -> ChangeWindow {
        ChangeWindow {
            days: vec![Day::Mon, Day::Tue, Day::Wed, Day::Thu],
            start: "09:00".into(),
            end: "16:00".into(),
            timezone: "Europe/Paris".into(),
        }
    }

    fn utc(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).expect("date").with_timezone(&Utc)
    }

    #[test]
    fn window_respects_timezone_and_dst() {
        let w = office_hours();
        w.validate().expect("valide");
        // Mercredi 25/11/2026 08:30 UTC = 09:30 à Paris (UTC+1) : ouvert.
        assert!(w.contains(utc("2026-11-25T08:30:00Z")));
        // 07:30 UTC = 08:30 Paris : fermé.
        assert!(!w.contains(utc("2026-11-25T07:30:00Z")));
        // Été : mercredi 15/07/2026 07:30 UTC = 09:30 Paris (UTC+2) : ouvert.
        assert!(w.contains(utc("2026-07-15T07:30:00Z")));
        // Vendredi : fermé.
        assert!(!w.contains(utc("2026-11-27T10:00:00Z")));
    }

    #[test]
    fn next_opening_skips_closed_days() {
        let w = office_hours();
        // Vendredi 27/11 → lundi 30/11 09:00 Paris = 08:00 UTC.
        let next = w.next_opening(utc("2026-11-27T10:00:00Z")).expect("ouverture");
        assert_eq!(next, utc("2026-11-30T08:00:00Z"));
    }

    #[test]
    fn freeze_and_validation() {
        let f = Freeze {
            from: DateTime::parse_from_rfc3339("2026-12-19T18:00:00+01:00").expect("date"),
            to: DateTime::parse_from_rfc3339("2027-01-04T08:00:00+01:00").expect("date"),
            reason: "Gel de fin d'année".into(),
        };
        f.validate().expect("valide");
        assert!(f.contains(utc("2026-12-25T12:00:00Z")));
        assert!(!f.contains(utc("2027-01-05T12:00:00Z")));
        let mut bad = office_hours();
        bad.start = "17:00".into();
        assert!(bad.validate().is_err());
        bad = office_hours();
        bad.timezone = "Mars/Olympus".into();
        assert!(bad.validate().is_err());
        assert!(windows_open(&[], utc("2026-11-27T03:00:00Z")));
    }
}
