use thiserror::Error;

/// Erreurs métier du noyau. Chaque variante correspond à une famille de
/// codes HTTP côté API (400, 403, 404, 409, 422).
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CoreError {
    #[error("spécification invalide :\n  - {}", .0.join("\n  - "))]
    InvalidSpec(Vec<String>),
    #[error("requête invalide : {0}")]
    InvalidRequest(String),
    #[error("introuvable : {0}")]
    NotFound(String),
    #[error("accès refusé : {0}")]
    Forbidden(String),
    #[error("transition interdite : {0}")]
    InvalidTransition(String),
    #[error("conflit : {0}")]
    Conflict(String),
}

pub type Result<T, E = CoreError> = std::result::Result<T, E>;

impl CoreError {
    pub fn invalid(msg: impl Into<String>) -> Self {
        Self::InvalidRequest(msg.into())
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }
    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::Forbidden(msg.into())
    }
}
