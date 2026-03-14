use std::io::IsTerminal;

use qimen_error::{QimenError, Result};
use tracing_subscriber::EnvFilter;

pub fn init(level: &str, json_logs: bool) -> Result<()> {
    let filter = EnvFilter::try_new(level).unwrap_or_else(|_| EnvFilter::new("info"));

    if json_logs {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .json()
            .try_init()
            .map_err(|err| QimenError::Observability(err.to_string()))?;
    } else {
        let use_ansi = std::io::stderr().is_terminal();
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_ansi(use_ansi)
            .try_init()
            .map_err(|err| QimenError::Observability(err.to_string()))?;
    }

    Ok(())
}
