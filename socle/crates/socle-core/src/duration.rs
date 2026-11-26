//! Durées lisibles (`30s`, `15m`, `1h30m`, `7d`, `2w`) utilisées dans
//! `socle.yaml` pour les RPO/RTO, cadences de sauvegarde et délais.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct HumanDuration(u64);

impl HumanDuration {
    pub const fn from_secs(secs: u64) -> Self {
        Self(secs)
    }
    pub const fn from_mins(mins: u64) -> Self {
        Self(mins * 60)
    }
    pub const fn as_secs(self) -> u64 {
        self.0
    }
    pub fn as_chrono(self) -> chrono::TimeDelta {
        chrono::TimeDelta::seconds(i64::try_from(self.0).unwrap_or(i64::MAX / 1000))
    }
    pub fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Analyse une durée composée d'une ou plusieurs paires `<nombre><unité>`.
    pub fn parse(input: &str) -> Result<Self, String> {
        let s = input.trim();
        if s.is_empty() {
            return Err("durée vide".into());
        }
        if s.chars().all(|c| c.is_ascii_digit()) {
            return s.parse::<u64>().map(Self).map_err(|_| format!("durée invalide : {input}"));
        }
        let mut total: u64 = 0;
        let mut number = String::new();
        for c in s.chars() {
            if c.is_ascii_digit() {
                number.push(c);
                continue;
            }
            if number.is_empty() {
                return Err(format!("durée invalide : {input} (unité sans valeur)"));
            }
            let value: u64 = number.parse().map_err(|_| format!("durée invalide : {input}"))?;
            let factor = match c {
                's' => 1,
                'm' => 60,
                'h' => 3_600,
                'd' => 86_400,
                'w' => 604_800,
                _ => return Err(format!("unité inconnue « {c} » dans {input} (s, m, h, d, w)")),
            };
            total = value
                .checked_mul(factor)
                .and_then(|v| total.checked_add(v))
                .ok_or_else(|| format!("durée trop grande : {input}"))?;
            number.clear();
        }
        if !number.is_empty() {
            return Err(format!("durée invalide : {input} (unité manquante après {number})"));
        }
        Ok(Self(total))
    }
}

impl fmt::Display for HumanDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 == 0 {
            return write!(f, "0s");
        }
        let mut rest = self.0;
        for (unit, size) in [("w", 604_800), ("d", 86_400), ("h", 3_600), ("m", 60), ("s", 1)] {
            if rest >= size {
                write!(f, "{}{}", rest / size, unit)?;
                rest %= size;
            }
        }
        Ok(())
    }
}

impl FromStr for HumanDuration {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl Serialize for HumanDuration {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for HumanDuration {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Secs(u64),
            Text(String),
        }
        match Raw::deserialize(deserializer)? {
            Raw::Secs(s) => Ok(Self(s)),
            Raw::Text(t) => Self::parse(&t).map_err(serde::de::Error::custom),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_display_roundtrip() {
        for (input, secs, display) in [
            ("30s", 30, "30s"),
            ("15m", 900, "15m"),
            ("1h30m", 5_400, "1h30m"),
            ("90m", 5_400, "1h30m"),
            ("7d", 604_800, "1w"),
            ("2w", 1_209_600, "2w"),
            ("3600", 3_600, "1h"),
        ] {
            let d = HumanDuration::parse(input).expect(input);
            assert_eq!(d.as_secs(), secs, "{input}");
            assert_eq!(d.to_string(), display, "{input}");
        }
    }

    #[test]
    fn rejects_invalid_input() {
        for input in ["", "m", "10x", "h1", "1h30"] {
            assert!(HumanDuration::parse(input).is_err(), "{input} aurait dû échouer");
        }
    }
}
