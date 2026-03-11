use qimen_error::Result;
use qimen_official_host::run_official_host;

#[tokio::main]
async fn main() -> Result<()> {
    run_official_host("config/base.toml").await
}
