mod onebot11_simulator;

use qimen_config::AppConfig;
use qimen_error::{QimenError, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("simulate-onebot11") => onebot11_simulator::run(&args[1..]).await,
        Some("help" | "--help" | "-h") => {
            print_help();
            Ok(())
        }
        Some(command) => Err(QimenError::Config(format!(
            "unknown qimenctl command '{command}'"
        ))),
        None => {
            let config = AppConfig::load_from_path("config/base.toml")?;
            println!("loaded {} bot definitions", config.bots.len());
            println!("run 'qimenctl --help' for test commands");
            Ok(())
        }
    }
}

fn print_help() {
    println!(
        "qimenctl commands:\n\
         \n  simulate-onebot11  Simulate a OneBot 11 reverse WebSocket client\n\
         \nRun 'qimenctl simulate-onebot11 --help' for command options."
    );
}
