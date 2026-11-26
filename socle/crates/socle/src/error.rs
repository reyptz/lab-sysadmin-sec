use socle_core::CoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error("base de données : {0}")]
    Db(#[from] sqlx::Error),
    /// Mise à jour concurrente détectée (verrouillage optimiste).
    #[error("modification concurrente de {0} : réessayez")]
    Stale(String),
    #[error("non authentifié : {0}")]
    Unauthenticated(String),
    #[error("{0}")]
    Internal(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Internal(format!("sérialisation : {e}"))
    }
}
