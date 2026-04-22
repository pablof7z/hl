use thiserror::Error;

#[derive(Debug, Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum CoreError {
    #[error("not initialized")]
    NotInitialized,
    #[error("not authenticated")]
    NotAuthenticated,
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("signer error: {0}")]
    Signer(String),
    #[error("relay error: {0}")]
    Relay(String),
    #[error("cache error: {0}")]
    Cache(String),
    #[error("not found")]
    NotFound,
    #[error("{0}")]
    Other(String),
}

impl From<anyhow::Error> for CoreError {
    fn from(value: anyhow::Error) -> Self {
        Self::Other(value.to_string())
    }
}
