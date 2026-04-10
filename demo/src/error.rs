use thiserror::Error;

/// Storage layer errors.
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("database query failed: {0}")]
    Query(#[from] sqlx::Error),

    #[error("entity not found: {entity} {id}")]
    NotFound { entity: &'static str, id: String },

    #[error("duplicate key: {0}")]
    Duplicate(String),
}

/// Application-level errors that map to JSON-RPC error codes.
#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Storage(#[from] StorageError),

    #[error("protocol error {code}: {message}")]
    Protocol { code: i32, message: String },

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("database not configured")]
    NoDB,
}

impl AppError {
    /// Map to JSON-RPC error code.
    pub fn jsonrpc_code(&self) -> i32 {
        match self {
            AppError::Storage(StorageError::NotFound { .. }) => -32044,
            AppError::Storage(StorageError::Duplicate(_)) => -32053,
            AppError::Storage(StorageError::Query(_)) => -32050,
            AppError::Protocol { code, .. } => *code,
            AppError::Unauthorized(_) => -32021,
            AppError::NoDB => -32050,
        }
    }
}
