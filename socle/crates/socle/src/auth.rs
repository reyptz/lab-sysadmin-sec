//! Authentification : jetons d'API (`socle_pat_…`, stockés hachés) et jetons
//! OIDC (Entra ID, Keycloak, Okta, Google…) vérifiés contre les clés JWKS du
//! fournisseur. Les groupes du jeton OIDC sont mappés sur les équipes de
//! `socle.yaml`, qui portent les rôles.

use std::sync::Arc;
use std::time::{Duration, Instant};

use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde_json::Value;
use socle_core::rbac::{self, Principal, PrincipalKind};
use tokio::sync::RwLock;

use crate::engine::Engine;
use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct OidcConfig {
    pub issuer: String,
    pub audience: String,
    /// URL JWKS ; découverte via `/.well-known/openid-configuration` si absente.
    pub jwks_url: Option<String>,
    pub groups_claim: String,
    pub subject_claim: String,
}

struct KeyCache {
    fetched: Option<Instant>,
    keys: JwkSet,
}

pub struct OidcVerifier {
    cfg: OidcConfig,
    http: reqwest::Client,
    cache: RwLock<KeyCache>,
}

const ALLOWED_ALGS: [Algorithm; 8] = [
    Algorithm::RS256,
    Algorithm::RS384,
    Algorithm::RS512,
    Algorithm::PS256,
    Algorithm::PS384,
    Algorithm::PS512,
    Algorithm::ES256,
    Algorithm::ES384,
];

impl OidcVerifier {
    pub fn new(cfg: OidcConfig) -> Self {
        Self {
            cfg,
            http: reqwest::Client::builder().timeout(Duration::from_secs(10)).build().unwrap_or_default(),
            cache: RwLock::new(KeyCache { fetched: None, keys: JwkSet { keys: Vec::new() } }),
        }
    }

    async fn jwks_url(&self) -> Result<String> {
        if let Some(u) = &self.cfg.jwks_url {
            return Ok(u.clone());
        }
        let discovery = format!("{}/.well-known/openid-configuration", self.cfg.issuer.trim_end_matches('/'));
        let doc: Value = self
            .http
            .get(&discovery)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|e| Error::internal(format!("découverte OIDC impossible : {e}")))?
            .json()
            .await
            .map_err(|e| Error::internal(format!("document OIDC illisible : {e}")))?;
        doc.get("jwks_uri")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| Error::internal("jwks_uri absent du document de découverte OIDC"))
    }

    async fn refresh(&self) -> Result<()> {
        let url = self.jwks_url().await?;
        let keys: JwkSet = self
            .http
            .get(&url)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|e| Error::internal(format!("JWKS injoignable : {e}")))?
            .json()
            .await
            .map_err(|e| Error::internal(format!("JWKS illisible : {e}")))?;
        let mut cache = self.cache.write().await;
        cache.keys = keys;
        cache.fetched = Some(Instant::now());
        Ok(())
    }

    /// Vérifie signature, émetteur, audience et validité ; renvoie le sujet
    /// et les groupes.
    pub async fn verify(&self, token: &str) -> Result<(String, Vec<String>)> {
        let header = jsonwebtoken::decode_header(token).map_err(|_| Error::Unauthenticated("jeton malformé".into()))?;
        if !ALLOWED_ALGS.contains(&header.alg) {
            return Err(Error::Unauthenticated("algorithme de signature refusé".into()));
        }
        let kid = header.kid.clone().ok_or_else(|| Error::Unauthenticated("jeton sans identifiant de clé".into()))?;
        let stale = self.cache.read().await.fetched.is_none_or(|t| t.elapsed() > Duration::from_secs(600));
        if stale || self.cache.read().await.keys.find(&kid).is_none() {
            self.refresh().await?;
        }
        let cache = self.cache.read().await;
        let jwk = cache.keys.find(&kid).ok_or_else(|| Error::Unauthenticated("clé de signature inconnue".into()))?;
        let key =
            DecodingKey::from_jwk(jwk).map_err(|_| Error::Unauthenticated("clé de signature inutilisable".into()))?;
        let mut validation = Validation::new(header.alg);
        validation.set_issuer(&[self.cfg.issuer.as_str()]);
        validation.set_audience(&[self.cfg.audience.as_str()]);
        validation.leeway = 60;
        let data = jsonwebtoken::decode::<Value>(token, &key, &validation)
            .map_err(|e| Error::Unauthenticated(format!("jeton refusé : {e}")))?;
        let claims = data.claims;
        let subject = claims
            .get(&self.cfg.subject_claim)
            .and_then(Value::as_str)
            .or_else(|| claims.get("sub").and_then(Value::as_str))
            .ok_or_else(|| Error::Unauthenticated("sujet absent du jeton".into()))?
            .to_string();
        let groups = claims
            .get(&self.cfg.groups_claim)
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default();
        Ok((subject, groups))
    }
}

pub struct Authenticator {
    engine: Arc<Engine>,
    oidc: Option<OidcVerifier>,
}

impl Authenticator {
    pub fn new(engine: Arc<Engine>, oidc: Option<OidcConfig>) -> Self {
        Self { engine, oidc: oidc.map(OidcVerifier::new) }
    }

    pub fn oidc_enabled(&self) -> bool {
        self.oidc.is_some()
    }

    /// Authentifie l'en-tête `Authorization: Bearer …`.
    pub async fn authenticate(&self, authorization: Option<&str>) -> Result<Principal> {
        let token = authorization
            .and_then(|h| h.strip_prefix("Bearer ").or_else(|| h.strip_prefix("bearer ")))
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .ok_or_else(|| Error::Unauthenticated("en-tête Authorization: Bearer manquant".into()))?;
        if token.starts_with("socle_pat_") {
            return self.engine.authenticate_token(token).await;
        }
        let verifier = self
            .oidc
            .as_ref()
            .ok_or_else(|| Error::Unauthenticated("OIDC non configuré : utilisez un jeton d'API".into()))?;
        let (subject, groups) = verifier.verify(token).await?;
        let spec = self.engine.spec();
        Ok(Principal {
            display_name: subject.clone(),
            teams: rbac::resolve_teams(&spec, &subject, &groups),
            id: subject,
            kind: PrincipalKind::User,
        })
    }
}
