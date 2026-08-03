use qimen_plugin_marketplace::{MarketplaceClient, load_catalog_directory, write_catalog_index};
use std::path::PathBuf;
use std::time::Duration;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("marketplace validation failed: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut check_only = false;
    let mut verify_github = false;
    let mut positional = Vec::new();
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--check" => check_only = true,
            "--verify-github" => verify_github = true,
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            _ if argument.starts_with('-') => {
                return Err(format!("unknown option '{argument}'").into());
            }
            _ => positional.push(argument),
        }
    }

    let marketplace_root = positional
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("marketplace"));
    let output = positional
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("docs/public/marketplace/index.json"));
    if positional.len() > 2 {
        return Err("too many positional arguments".into());
    }

    let index = load_catalog_directory(&marketplace_root)?;
    println!("validated {} marketplace plugin(s)", index.plugins.len());

    if verify_github {
        let client = MarketplaceClient::new(
            "https://lvyunqi.github.io/QimenBot/marketplace/index.json",
            Duration::from_secs(30),
        )?;
        let results = client.verify_catalog_sources(&index).await;
        let mut failed = false;
        for result in results {
            if result.valid {
                println!(
                    "verified {} (repository ID {})",
                    result.plugin_id, result.repository_id
                );
            } else {
                failed = true;
                for message in result.messages {
                    eprintln!("{}: {}", result.plugin_id, message);
                }
            }
        }
        if failed {
            return Err("one or more GitHub sources failed verification".into());
        }
    }

    if !check_only {
        write_catalog_index(&index, &output)?;
        publish_schemas(&marketplace_root, &output)?;
        println!("wrote {}", output.display());
    }
    Ok(())
}

fn publish_schemas(
    marketplace_root: &std::path::Path,
    output: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = marketplace_root.join("schemas");
    let destination = output
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("schemas");
    std::fs::create_dir_all(&destination)?;
    for file_name in ["plugin.schema.json", "version.schema.json"] {
        std::fs::copy(source.join(file_name), destination.join(file_name))?;
    }
    Ok(())
}

fn print_help() {
    println!(
        "Usage: qimen-marketplace-index [--check] [--verify-github] [MARKETPLACE_DIR] [OUTPUT_JSON]"
    );
}
