use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, MarketplaceError>;

#[derive(Debug, thiserror::Error)]
pub enum MarketplaceError {
    #[error("invalid marketplace metadata: {0}")]
    InvalidMetadata(String),
    #[error("marketplace item was not found: {0}")]
    NotFound(String),
    #[error("marketplace operation conflicts with local state: {0}")]
    Conflict(String),
    #[error("plugin is not compatible with this host: {0}")]
    Incompatible(String),
    #[error("marketplace is disabled")]
    Disabled,
    #[error("marketplace request failed: {0}")]
    Network(String),
    #[error("downloaded asset checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },
    #[error("marketplace path is not safe: {0}")]
    UnsafePath(PathBuf),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("TOML decode error: {0}")]
    TomlDecode(#[from] toml::de::Error),
    #[error("TOML encode error: {0}")]
    TomlEncode(#[from] toml::ser::Error),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}

impl MarketplaceError {
    pub fn is_user_error(&self) -> bool {
        matches!(
            self,
            Self::InvalidMetadata(_)
                | Self::NotFound(_)
                | Self::Conflict(_)
                | Self::Incompatible(_)
                | Self::Disabled
                | Self::ChecksumMismatch { .. }
                | Self::UnsafePath(_)
        )
    }
}
