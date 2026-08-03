use crate::{
    CatalogIndex, CatalogPlugin, MarketplaceError, PluginManifest, Result, VersionManifest,
};
use semver::Version;
use std::fs;
use std::path::{Path, PathBuf};

pub fn load_catalog_directory(root: impl AsRef<Path>) -> Result<CatalogIndex> {
    let root = root.as_ref();
    let plugins_root = root.join("plugins");
    if !plugins_root.is_dir() {
        return Err(MarketplaceError::InvalidMetadata(format!(
            "marketplace plugins directory '{}' does not exist",
            plugins_root.display()
        )));
    }

    let mut plugin_directories = fs::read_dir(&plugins_root)?
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    plugin_directories.sort();

    let mut plugins = Vec::new();
    for directory in plugin_directories {
        plugins.push(load_plugin_directory(&directory)?);
    }
    plugins.sort_by(|left, right| left.manifest.id.cmp(&right.manifest.id));
    let index = CatalogIndex {
        schema_version: crate::model::CATALOG_SCHEMA_VERSION,
        plugins,
    };
    index.validate()?;
    Ok(index)
}

pub fn write_catalog_index(index: &CatalogIndex, destination: impl AsRef<Path>) -> Result<()> {
    index.validate()?;
    let destination = destination.as_ref();
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_vec_pretty(index)?;
    let temporary = temporary_path(destination);
    fs::write(&temporary, raw)?;
    atomic_replace(&temporary, destination)?;
    Ok(())
}

fn load_plugin_directory(directory: &Path) -> Result<CatalogPlugin> {
    let manifest_path = directory.join("plugin.toml");
    let manifest = read_toml::<PluginManifest>(&manifest_path)?;
    let directory_name = directory
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if directory_name != manifest.id {
        return Err(MarketplaceError::InvalidMetadata(format!(
            "plugin directory '{}' must match plugin ID '{}'",
            directory.display(),
            manifest.id
        )));
    }

    let versions_root = directory.join("versions");
    if !versions_root.is_dir() {
        return Err(MarketplaceError::InvalidMetadata(format!(
            "plugin '{}' is missing its versions directory",
            manifest.id
        )));
    }
    let mut version_paths = fs::read_dir(&versions_root)?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("toml"))
        .collect::<Vec<_>>();
    version_paths.sort();

    let mut versions = Vec::new();
    for path in version_paths {
        let version = read_toml::<VersionManifest>(&path)?;
        let expected_file = format!("{}.toml", version.version);
        if path.file_name().and_then(|value| value.to_str()) != Some(expected_file.as_str()) {
            return Err(MarketplaceError::InvalidMetadata(format!(
                "version file '{}' must be named '{}'",
                path.display(),
                expected_file
            )));
        }
        versions.push(version);
    }
    versions.sort_by(|left, right| {
        match (
            Version::parse(&left.version),
            Version::parse(&right.version),
        ) {
            (Ok(left), Ok(right)) => right.cmp_precedence(&left),
            _ => right.version.cmp(&left.version),
        }
    });
    let plugin = CatalogPlugin { manifest, versions };
    plugin.validate()?;
    Ok(plugin)
}

fn read_toml<T>(path: &Path) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let raw = fs::read_to_string(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            MarketplaceError::InvalidMetadata(format!(
                "required marketplace file '{}' was not found",
                path.display()
            ))
        } else {
            error.into()
        }
    })?;
    toml::from_str(&raw).map_err(|error| {
        MarketplaceError::InvalidMetadata(format!("failed to parse '{}': {error}", path.display()))
    })
}

fn temporary_path(destination: &Path) -> PathBuf {
    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("index.json");
    destination.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()))
}

fn atomic_replace(source: &Path, destination: &Path) -> Result<()> {
    if let Err(first_error) = fs::rename(source, destination) {
        if destination.exists() {
            fs::remove_file(destination)?;
            fs::rename(source, destination)?;
        } else {
            return Err(first_error.into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn directory_loader_is_deterministic_and_checks_file_names() {
        let root = temp_root();
        let plugin = root.join("plugins/status-tools");
        fs::create_dir_all(plugin.join("versions")).unwrap();
        fs::write(
            plugin.join("plugin.toml"),
            r#"schema_version = 1
id = "status-tools"
name = "Status Tools"
summary = "Reports status"
type = "static"
repository = "example/status-tools"
repository_id = 42
license = "MIT"
"#,
        )
        .unwrap();
        fs::write(
            plugin.join("versions/1.0.0.toml"),
            r#"schema_version = 1
version = "1.0.0"
released_at = "2026-08-03T00:00:00Z"
release_tag = "v1.0.0"
channel = "stable"
qimenbot = ">=0.1.16"

[[drivers]]
driver = "onebot11"
scenes = ["private", "group"]
events = ["message"]
outbound = ["reply"]
"#,
        )
        .unwrap();

        let index = load_catalog_directory(&root).unwrap();
        assert_eq!(index.plugins[0].manifest.id, "status-tools");
        let destination = root.join("index.json");
        write_catalog_index(&index, &destination).unwrap();
        let first = fs::read(&destination).unwrap();
        write_catalog_index(&index, &destination).unwrap();
        assert_eq!(first, fs::read(&destination).unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    fn temp_root() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "qimen-marketplace-catalog-{}-{nonce}",
            std::process::id()
        ))
    }
}
