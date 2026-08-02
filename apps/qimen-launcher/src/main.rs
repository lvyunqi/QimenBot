mod config;
mod github;
mod installer;
mod supervisor;

use config::ResolvedConfig;
use qimen_update_protocol::{LauncherCommandAction, enqueue_launcher_command};
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;
use tracing_subscriber::EnvFilter;

pub type DynError = Box<dyn std::error::Error + Send + Sync>;

#[tokio::main]
async fn main() -> ExitCode {
    init_logging();
    match execute().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(error = %error, "qimenbot 执行失败");
            ExitCode::FAILURE
        }
    }
}

async fn execute() -> Result<(), DynError> {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    let command = arguments
        .first()
        .and_then(|value| value.to_str())
        .unwrap_or("run");
    match command {
        "--version" | "-V" => {
            println!("qimenbot {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "--help" | "-h" | "help" => {
            print_help();
            Ok(())
        }
        "run" => {
            let config = ResolvedConfig::load(option_path(&arguments, "--config")?)?;
            supervisor::run(config).await
        }
        "check" => queue_command(&arguments, LauncherCommandAction::Check),
        "install" => queue_command(&arguments, LauncherCommandAction::Install),
        "restart" => queue_command(&arguments, LauncherCommandAction::Restart),
        other => Err(format!("unknown qimenbot command '{other}'").into()),
    }
}

fn queue_command(arguments: &[OsString], action: LauncherCommandAction) -> Result<(), DynError> {
    let config = ResolvedConfig::load(option_path(arguments, "--config")?)?;
    let id = enqueue_launcher_command(&config.update_dir, action)?;
    println!("qimenbot command queued: {id}");
    Ok(())
}

fn option_path(arguments: &[OsString], name: &str) -> Result<Option<PathBuf>, DynError> {
    Ok(option_value(arguments, name)?.map(PathBuf::from))
}

fn option_value(arguments: &[OsString], name: &str) -> Result<Option<OsString>, DynError> {
    let mut index = 0;
    while index < arguments.len() {
        if arguments[index] == name {
            return arguments
                .get(index + 1)
                .cloned()
                .map(Some)
                .ok_or_else(|| format!("option {name} requires a value").into());
        }
        index += 1;
    }
    Ok(None)
}

fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}

fn print_help() {
    println!(
        "QimenBot process supervisor and updater\n\n\
         Usage:\n  \
         qimenbot run [--config PATH]\n  \
         qimenbot check|install|restart [--config PATH]\n\n\
         Updates are applied only from the configured GitHub repository."
    );
}
