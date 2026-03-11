use qimen_config::AppConfig;
use qimen_error::Result;

fn main() -> Result<()> {
    let config = AppConfig::load_from_path("config/base.toml")?;
    println!("loaded {} bot definitions", config.bots.len());
    Ok(())
}
