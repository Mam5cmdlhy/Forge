use thiserror::Error;

#[derive(Error, Debug)]
pub enum ForgeError {
    #[error("Entity not found: {0}")]
    NotFound(String),

    #[error("Validation failed: {0}")]
    Validation(String),

    #[error("Invalid workflow transition from '{from}' to '{to}'")]
    InvalidTransition { from: String, to: String },

    #[error("Project error: {0}")]
    Project(String),

    #[error("YAML error: {0}")]
    Yaml(String),

    #[error("Storage error: {0}")]
    Store(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, ForgeError>;
