use thiserror::Error;

pub type Result<T> = std::result::Result<T, QimenError>;

#[derive(Debug, Error)]
pub enum QimenError {
    #[error("config error: {0}")]
    Config(String),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("runtime error: {0}")]
    Runtime(String),
    #[error("module error: {0}")]
    Module(String),
    #[error("transport error: {0}")]
    Transport(String),
    #[error("observability error: {0}")]
    Observability(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    TomlDe(#[from] toml::de::Error),
}
