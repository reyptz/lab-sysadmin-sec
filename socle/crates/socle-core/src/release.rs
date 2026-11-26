//! Releases immuables : un artefact identifié par son digest, accompagné des
//! preuves produites par la CI (tests, vulnérabilités, signature, SBOM).
//! Principe « build once, deploy many » : on promeut le même digest d'un
//! environnement à l'autre, jamais une reconstruction.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::names;
use crate::{CoreError, Result};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VulnSummary {
    #[serde(default)]
    pub critical: u32,
    #[serde(default)]
    pub high: u32,
    #[serde(default)]
    pub medium: u32,
    #[serde(default)]
    pub low: u32,
}

/// Preuves attachées à une release. `None` signifie « non fourni », ce qui
/// est distinct d'un échec et bloque une porte qui l'exige.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    #[serde(default)]
    pub tests_passed: Option<bool>,
    #[serde(default)]
    pub vulnerabilities: Option<VulnSummary>,
    #[serde(default)]
    pub signature_verified: Option<bool>,
    #[serde(default)]
    pub sbom_uri: Option<String>,
    #[serde(default)]
    pub provenance_uri: Option<String>,
    #[serde(default)]
    pub iac_scan_passed: Option<bool>,
    #[serde(default)]
    pub secret_scan_passed: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Release {
    pub id: String,
    pub service: String,
    pub version: String,
    pub image: String,
    pub digest: String,
    #[serde(default)]
    pub git_sha: Option<String>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub evidence: Evidence,
}

impl Release {
    /// Référence immuable `dépôt@sha256:…`.
    pub fn image_ref(&self) -> String {
        format!("{}@{}", self.image, self.digest)
    }

    /// 12 premiers caractères hexadécimaux du digest.
    pub fn short_digest(&self) -> &str {
        self.digest.get(7..19).unwrap_or(&self.digest)
    }

    pub fn label(&self) -> String {
        format!("{} ({})", self.version, self.short_digest())
    }
}

/// Données d'enregistrement d'une release, fournies par la CI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewRelease {
    pub service: String,
    pub version: String,
    pub digest: String,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub git_sha: Option<String>,
    #[serde(default)]
    pub evidence: Evidence,
}

impl NewRelease {
    /// Contrôle de cohérence : le digest est bien un SHA-256 et l'image, si
    /// elle est précisée, correspond au dépôt déclaré pour le service.
    pub fn validate(&self, expected_image: &str) -> Result<()> {
        if !names::is_valid_digest(&self.digest) {
            return Err(CoreError::invalid(format!("digest invalide « {} » (attendu : sha256:<64 hex>)", self.digest)));
        }
        if !names::is_valid_version(&self.version) {
            return Err(CoreError::invalid(format!("version invalide « {} »", self.version)));
        }
        if let Some(image) = &self.image
            && image != expected_image
        {
            return Err(CoreError::invalid(format!(
                "l'image {image} ne correspond pas au dépôt déclaré pour {} ({expected_image})",
                self.service
            )));
        }
        if let Some(sha) = &self.git_sha
            && !names::is_valid_git_sha(sha)
        {
            return Err(CoreError::invalid(format!("commit git invalide « {sha} »")));
        }
        for uri in [&self.evidence.sbom_uri, &self.evidence.provenance_uri].into_iter().flatten() {
            if uri.len() > 2048 || uri.contains(char::is_whitespace) {
                return Err(CoreError::invalid("URI de preuve invalide"));
            }
        }
        Ok(())
    }
}
