use qimen_error::Result;
use qimen_official_host::run_official_host;

// Force the linker to include plugin crate object files containing
// inventory::submit! registrations. On Windows/MSVC, `use crate as _`
// alone is insufficient — the linker may drop object files that only
// contain inventory constructors if no concrete symbol is referenced.
extern crate qimen_plugin_example;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();

    // Reference concrete symbols from each plugin crate so that the
    // linker is forced to include the object files with inventory entries.
    std::hint::black_box(qimen_plugin_example::BasicModule::__QIMEN_MODULE_ID);

    let config_path =
        std::env::var("QIMEN_CONFIG_PATH").unwrap_or_else(|_| "config/base.toml".to_string());
    run_official_host(&config_path).await
}
