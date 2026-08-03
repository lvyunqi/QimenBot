use crate::model::{dynamic_library_extension, validate_file_name, validate_plugin_id};
use crate::{
    CatalogPlugin, MarketplaceClient, MarketplaceError, PluginAsset, ReleaseChannel, Result,
    VersionManifest,
};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const LOCK_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstalledVersion {
    pub version: String,
    pub repository_id: u64,
    pub target: String,
    pub sha256: String,
    pub channel: ReleaseChannel,
    pub data_schema_version: u32,
    pub rollback_safe: bool,
    pub installed_at: String,
}

impl InstalledVersion {
    fn validate(&self, plugin_id: &str) -> Result<()> {
        Version::parse(&self.version).map_err(|error| {
            MarketplaceError::InvalidMetadata(format!(
                "lock entry for '{plugin_id}' has invalid version '{}': {error}",
                self.version
            ))
        })?;
        if self.repository_id == 0
            || self.sha256.len() != 64
            || !self
                .sha256
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        {
            return Err(MarketplaceError::InvalidMetadata(format!(
                "lock entry for '{plugin_id}' has invalid repository or checksum data"
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstalledPlugin {
    pub active_file: String,
    #[serde(default)]
    pub pinned: bool,
    pub current: InstalledVersion,
    #[serde(default)]
    pub previous: Option<InstalledVersion>,
}

impl InstalledPlugin {
    fn validate(&self, plugin_id: &str) -> Result<()> {
        validate_plugin_id(plugin_id)?;
        validate_file_name(&self.active_file)?;
        self.current.validate(plugin_id)?;
        if let Some(previous) = &self.previous {
            previous.validate(plugin_id)?;
        }
        Ok(())
    }

    pub fn can_rollback(&self) -> bool {
        self.current.rollback_safe && self.previous.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketplaceLock {
    pub schema_version: u32,
    #[serde(default)]
    pub plugins: BTreeMap<String, InstalledPlugin>,
}

impl Default for MarketplaceLock {
    fn default() -> Self {
        Self {
            schema_version: LOCK_SCHEMA_VERSION,
            plugins: BTreeMap::new(),
        }
    }
}

impl MarketplaceLock {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = match fs::read_to_string(path) {
            Ok(raw) => raw,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(error.into()),
        };
        let lock = toml::from_str::<Self>(&raw)?;
        lock.validate()?;
        Ok(lock)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let raw = toml::to_string_pretty(self)?;
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("marketplace-lock.toml");
        let temporary = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));
        fs::write(&temporary, raw)?;
        atomic_replace(&temporary, path)?;
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != LOCK_SCHEMA_VERSION {
            return Err(MarketplaceError::InvalidMetadata(format!(
                "marketplace lock uses unsupported schema_version {}",
                self.schema_version
            )));
        }
        for (plugin_id, plugin) in &self.plugins {
            plugin.validate(plugin_id)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct MarketplacePaths {
    pub cache_dir: PathBuf,
    pub lock_path: PathBuf,
    pub plugin_bin_dir: PathBuf,
}

impl MarketplacePaths {
    pub fn new(
        cache_dir: impl Into<PathBuf>,
        lock_path: impl Into<PathBuf>,
        plugin_bin_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            cache_dir: cache_dir.into(),
            lock_path: lock_path.into(),
            plugin_bin_dir: plugin_bin_dir.into(),
        }
    }

    pub fn catalog_cache_dir(&self) -> PathBuf {
        self.cache_dir.join("catalog")
    }

    pub fn active_path(&self, installed: &InstalledPlugin) -> Result<PathBuf> {
        validate_file_name(&installed.active_file)?;
        Ok(self.plugin_bin_dir.join(&installed.active_file))
    }

    pub fn archive_path(&self, plugin_id: &str, installed: &InstalledVersion) -> Result<PathBuf> {
        validate_plugin_id(plugin_id)?;
        installed.validate(plugin_id)?;
        let extension = dynamic_library_extension(&installed.target).ok_or_else(|| {
            MarketplaceError::InvalidMetadata(format!(
                "installed target '{}' is not dynamically loadable",
                installed.target
            ))
        })?;
        let file_name = format!("plugin.{extension}");
        let path = self
            .cache_dir
            .join("installed")
            .join(plugin_id)
            .join(&installed.version)
            .join(installed.sha256.to_ascii_lowercase())
            .join(file_name);
        ensure_managed_child(&path, &self.cache_dir)?;
        Ok(path)
    }

    pub async fn prepare_install(
        &self,
        client: &MarketplaceClient,
        plugin: &CatalogPlugin,
        version: &VersionManifest,
        asset: &PluginAsset,
        existing: Option<&InstalledPlugin>,
    ) -> Result<PreparedInstall> {
        if let Some(existing) = existing {
            if existing.pinned {
                return Err(MarketplaceError::Conflict(format!(
                    "plugin '{}' is pinned at version {}; unpin it before updating",
                    plugin.manifest.id, existing.current.version
                )));
            }
            let current = Version::parse(&existing.current.version).map_err(|error| {
                MarketplaceError::InvalidMetadata(format!(
                    "installed version '{}' is invalid: {error}",
                    existing.current.version
                ))
            })?;
            let requested = version.parsed_version()?;
            if requested.cmp_precedence(&current).is_lt() {
                return Err(MarketplaceError::Conflict(format!(
                    "installing {} over {} would be a downgrade; use the reviewed rollback action",
                    requested, current
                )));
            }
            if requested.cmp_precedence(&current).is_eq() {
                if existing.current.sha256.eq_ignore_ascii_case(&asset.sha256) {
                    return Err(MarketplaceError::Conflict(format!(
                        "plugin '{}' version {} is already installed",
                        plugin.manifest.id, requested
                    )));
                }
                return Err(MarketplaceError::Conflict(format!(
                    "plugin '{}' version {} has a different checksum than the installed lock; versions are immutable",
                    plugin.manifest.id, requested
                )));
            }
        }

        let active_file = existing
            .map(|installed| installed.active_file.clone())
            .unwrap_or_else(|| managed_file_name(&plugin.manifest.id, &asset.target));
        validate_file_name(&active_file)?;
        let active_path = self.plugin_bin_dir.join(&active_file);
        ensure_install_destination(&active_path, existing.is_some())?;

        let installed_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let next_version = InstalledVersion {
            version: version.version.clone(),
            repository_id: plugin.manifest.repository_id,
            target: asset.target.clone(),
            sha256: asset.sha256.to_ascii_lowercase(),
            channel: version.channel,
            data_schema_version: version.data_schema_version,
            rollback_safe: version.rollback_safe,
            installed_at,
        };
        let archive_path = self.archive_path(&plugin.manifest.id, &next_version)?;
        if archive_path.exists() {
            verify_file(&archive_path, asset)?;
        } else {
            let download_path = self
                .cache_dir
                .join("downloads")
                .join(&plugin.manifest.id)
                .join(&version.version)
                .join(&asset.target)
                .join(&asset.asset_name);
            ensure_managed_child(&download_path, &self.cache_dir)?;
            if download_path.exists() {
                if verify_file(&download_path, asset).is_err() {
                    fs::remove_file(&download_path)?;
                    client
                        .download_release_asset(plugin, version, asset, &download_path)
                        .await?;
                }
            } else {
                client
                    .download_release_asset(plugin, version, asset, &download_path)
                    .await?;
            }
            copy_atomic(&download_path, &archive_path)?;
            verify_file(&archive_path, asset)?;
        }

        let current = InstalledPlugin {
            active_file: active_file.clone(),
            pinned: false,
            current: next_version,
            previous: existing.map(|installed| installed.current.clone()),
        };
        let transaction = ActiveFileTransaction::replace(
            active_path,
            archive_path.clone(),
            transaction_root(&self.cache_dir, &plugin.manifest.id),
        )?;
        Ok(PreparedInstall {
            plugin_id: plugin.manifest.id.clone(),
            archive_path,
            installed: current,
            transaction,
        })
    }

    pub fn prepare_rollback(
        &self,
        plugin_id: &str,
        installed: &InstalledPlugin,
    ) -> Result<(InstalledPlugin, ActiveFileTransaction)> {
        if !installed.current.rollback_safe {
            return Err(MarketplaceError::Conflict(format!(
                "plugin '{plugin_id}' version {} does not permit binary rollback because its data migration may be irreversible",
                installed.current.version
            )));
        }
        let previous = installed.previous.clone().ok_or_else(|| {
            MarketplaceError::Conflict(format!(
                "plugin '{plugin_id}' has no retained previous version"
            ))
        })?;
        let archive = self.archive_path(plugin_id, &previous)?;
        if !archive.is_file() {
            return Err(MarketplaceError::NotFound(format!(
                "retained plugin binary '{}' is missing",
                archive.display()
            )));
        }
        let actual = sha256_file(&archive)?;
        if !actual.eq_ignore_ascii_case(&previous.sha256) {
            return Err(MarketplaceError::ChecksumMismatch {
                expected: previous.sha256.clone(),
                actual,
            });
        }
        let next = InstalledPlugin {
            active_file: installed.active_file.clone(),
            pinned: installed.pinned,
            current: previous,
            previous: None,
        };
        let transaction = ActiveFileTransaction::replace(
            self.active_path(installed)?,
            archive,
            transaction_root(&self.cache_dir, plugin_id),
        )?;
        Ok((next, transaction))
    }

    pub fn prepare_uninstall(
        &self,
        plugin_id: &str,
        installed: &InstalledPlugin,
    ) -> Result<ActiveFileTransaction> {
        ActiveFileTransaction::remove(
            self.active_path(installed)?,
            transaction_root(&self.cache_dir, plugin_id),
        )
    }

    pub fn archive_existing(
        &self,
        plugin_id: &str,
        installed: &InstalledVersion,
        source: &Path,
    ) -> Result<PathBuf> {
        let archive = self.archive_path(plugin_id, installed)?;
        copy_atomic(source, &archive)?;
        let actual = sha256_file(&archive)?;
        if !actual.eq_ignore_ascii_case(&installed.sha256) {
            let _ = fs::remove_file(&archive);
            return Err(MarketplaceError::ChecksumMismatch {
                expected: installed.sha256.clone(),
                actual,
            });
        }
        Ok(archive)
    }
}

#[derive(Debug, Clone)]
pub struct PreparedInstall {
    pub plugin_id: String,
    pub archive_path: PathBuf,
    pub installed: InstalledPlugin,
    pub transaction: ActiveFileTransaction,
}

#[derive(Debug, Clone)]
pub struct ActiveFileTransaction {
    active_path: PathBuf,
    incoming_path: Option<PathBuf>,
    transaction_root: PathBuf,
    backup_path: PathBuf,
    had_active_file: bool,
}

impl ActiveFileTransaction {
    pub fn replace(
        active_path: PathBuf,
        incoming_path: PathBuf,
        transaction_root: PathBuf,
    ) -> Result<Self> {
        if !incoming_path.is_file() {
            return Err(MarketplaceError::NotFound(format!(
                "staged plugin binary '{}' does not exist",
                incoming_path.display()
            )));
        }
        Self::new(active_path, Some(incoming_path), transaction_root)
    }

    pub fn remove(active_path: PathBuf, transaction_root: PathBuf) -> Result<Self> {
        Self::new(active_path, None, transaction_root)
    }

    fn new(
        active_path: PathBuf,
        incoming_path: Option<PathBuf>,
        transaction_root: PathBuf,
    ) -> Result<Self> {
        let file_name = active_path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| MarketplaceError::UnsafePath(active_path.clone()))?;
        validate_file_name(file_name)?;
        let backup_path = transaction_root.join("previous").join(file_name);
        let had_active_file = active_path.exists();
        Ok(Self {
            active_path,
            incoming_path,
            transaction_root,
            backup_path,
            had_active_file,
        })
    }

    pub fn active_path(&self) -> &Path {
        &self.active_path
    }

    pub fn staging_directory(&self) -> Option<&Path> {
        self.incoming_path.as_deref().and_then(Path::parent)
    }

    pub fn apply(&self) -> Result<()> {
        if let Some(parent) = self.active_path.parent() {
            fs::create_dir_all(parent)?;
        }
        if let Some(parent) = self.backup_path.parent() {
            fs::create_dir_all(parent)?;
        }
        if self.backup_path.exists() {
            return Err(MarketplaceError::Conflict(format!(
                "plugin transaction '{}' has already been applied",
                self.transaction_root.display()
            )));
        }
        if self.active_path.exists() {
            move_file(&self.active_path, &self.backup_path)?;
        }
        if let Some(incoming) = &self.incoming_path
            && let Err(error) = copy_atomic(incoming, &self.active_path)
        {
            let _ = self.rollback();
            return Err(error);
        }
        Ok(())
    }

    pub fn rollback(&self) -> Result<()> {
        if self.backup_path.exists() {
            if self.active_path.exists() {
                fs::remove_file(&self.active_path)?;
            }
            if let Some(parent) = self.active_path.parent() {
                fs::create_dir_all(parent)?;
            }
            move_file(&self.backup_path, &self.active_path)?;
        } else if !self.had_active_file && self.active_path.exists() {
            fs::remove_file(&self.active_path)?;
        }
        Ok(())
    }

    pub fn finalize(&self) -> Result<()> {
        if self.transaction_root.exists() {
            fs::remove_dir_all(&self.transaction_root)?;
        }
        Ok(())
    }
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(lower_hex(&hasher.finalize()))
}

fn verify_file(path: &Path, asset: &PluginAsset) -> Result<()> {
    let metadata = fs::metadata(path)?;
    if metadata.len() != asset.size_bytes {
        return Err(MarketplaceError::Conflict(format!(
            "cached asset '{}' has size {}, expected {}",
            path.display(),
            metadata.len(),
            asset.size_bytes
        )));
    }
    let actual = sha256_file(path)?;
    if !actual.eq_ignore_ascii_case(&asset.sha256) {
        return Err(MarketplaceError::ChecksumMismatch {
            expected: asset.sha256.clone(),
            actual,
        });
    }
    Ok(())
}

fn copy_atomic(source: &Path, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("plugin");
    let temporary = destination.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));
    if temporary.exists() {
        fs::remove_file(&temporary)?;
    }
    fs::copy(source, &temporary)?;
    atomic_replace(&temporary, destination)
}

fn move_file(source: &Path, destination: &Path) -> Result<()> {
    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(_rename_error) if !destination.exists() => {
            copy_remove_file(source, destination)?;
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

fn copy_remove_file(source: &Path, destination: &Path) -> Result<()> {
    copy_atomic(source, destination)?;
    if let Err(error) = fs::remove_file(source) {
        let _ = fs::remove_file(destination);
        return Err(error.into());
    }
    Ok(())
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

fn ensure_managed_child(path: &Path, root: &Path) -> Result<()> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| MarketplaceError::UnsafePath(path.to_path_buf()))?;
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(MarketplaceError::UnsafePath(path.to_path_buf()));
    }
    Ok(())
}

fn ensure_install_destination(path: &Path, replacing_existing: bool) -> Result<()> {
    if !replacing_existing && path.exists() {
        return Err(MarketplaceError::Conflict(format!(
            "managed plugin destination '{}' already exists; move or associate that file before installing",
            path.display()
        )));
    }
    Ok(())
}

fn managed_file_name(plugin_id: &str, target: &str) -> String {
    let normalized = plugin_id.replace('-', "_");
    match dynamic_library_extension(target) {
        Some("dll") => format!("qimen_marketplace_{normalized}.dll"),
        Some("dylib") => format!("libqimen_marketplace_{normalized}.dylib"),
        _ => format!("libqimen_marketplace_{normalized}.so"),
    }
}

fn transaction_root(cache_dir: &Path, plugin_id: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    cache_dir
        .join("transactions")
        .join(format!("{plugin_id}-{}-{nonce}", std::process::id()))
}

fn lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_can_be_rolled_back_without_losing_the_previous_file() {
        let root = temp_root();
        let active = root.join("plugins/plugin.so");
        let incoming = root.join("incoming/plugin.so");
        fs::create_dir_all(active.parent().unwrap()).unwrap();
        fs::create_dir_all(incoming.parent().unwrap()).unwrap();
        fs::write(&active, b"old").unwrap();
        fs::write(&incoming, b"new").unwrap();
        let transaction =
            ActiveFileTransaction::replace(active.clone(), incoming, root.join("transaction"))
                .unwrap();
        transaction.apply().unwrap();
        assert_eq!(fs::read(&active).unwrap(), b"new");
        transaction.rollback().unwrap();
        assert_eq!(fs::read(&active).unwrap(), b"old");
        transaction.rollback().unwrap();
        assert_eq!(fs::read(&active).unwrap(), b"old");
        transaction.finalize().unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn new_install_rollback_removes_the_active_file_without_a_backup() {
        let root = temp_root();
        let active = root.join("plugins/plugin.so");
        let incoming = root.join("incoming/plugin.so");
        fs::create_dir_all(incoming.parent().unwrap()).unwrap();
        fs::write(&incoming, b"new").unwrap();
        let transaction =
            ActiveFileTransaction::replace(active.clone(), incoming, root.join("transaction"))
                .unwrap();

        transaction.apply().unwrap();
        assert_eq!(fs::read(&active).unwrap(), b"new");
        transaction.rollback().unwrap();
        assert!(!active.exists());
        transaction.rollback().unwrap();

        transaction.finalize().unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn copy_remove_fallback_moves_a_file_between_transaction_directories() {
        let root = temp_root();
        let source = root.join("source/plugin.so");
        let destination = root.join("backup/plugin.so");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, b"old").unwrap();

        copy_remove_file(&source, &destination).unwrap();

        assert!(!source.exists());
        assert_eq!(fs::read(&destination).unwrap(), b"old");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn new_install_does_not_overwrite_an_unmanaged_destination() {
        let root = temp_root();
        let active = root.join("qimen_marketplace_status_tools.so");
        fs::create_dir_all(&root).unwrap();
        fs::write(&active, b"unmanaged").unwrap();

        assert!(ensure_install_destination(&active, false).is_err());
        assert!(ensure_install_destination(&active, true).is_ok());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rollback_rejects_a_tampered_archived_binary() {
        let root = temp_root();
        let paths = MarketplacePaths::new(
            root.join("cache"),
            root.join("marketplace-lock.toml"),
            root.join("plugins"),
        );
        let previous = InstalledVersion {
            version: "1.0.0".into(),
            repository_id: 42,
            target: "x86_64-unknown-linux-gnu".into(),
            sha256: sha256_bytes(b"reviewed"),
            channel: ReleaseChannel::Stable,
            data_schema_version: 1,
            rollback_safe: true,
            installed_at: "2026-08-03T00:00:00Z".into(),
        };
        let installed = InstalledPlugin {
            active_file: "libstatus_tools.so".into(),
            pinned: false,
            current: InstalledVersion {
                version: "1.1.0".into(),
                rollback_safe: true,
                ..previous.clone()
            },
            previous: Some(previous.clone()),
        };
        let archive = paths.archive_path("status-tools", &previous).unwrap();
        fs::create_dir_all(archive.parent().unwrap()).unwrap();
        fs::write(&archive, b"tampered").unwrap();

        assert!(matches!(
            paths.prepare_rollback("status-tools", &installed),
            Err(MarketplaceError::ChecksumMismatch { .. })
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn lock_round_trip_preserves_pin_and_previous_version() {
        let root = temp_root();
        let path = root.join("config/marketplace-lock.toml");
        let version = InstalledVersion {
            version: "1.0.0".into(),
            repository_id: 42,
            target: "x86_64-unknown-linux-gnu".into(),
            sha256: "a".repeat(64),
            channel: ReleaseChannel::Stable,
            data_schema_version: 1,
            rollback_safe: true,
            installed_at: "2026-08-03T00:00:00Z".into(),
        };
        let mut lock = MarketplaceLock::default();
        lock.plugins.insert(
            "status-tools".into(),
            InstalledPlugin {
                active_file: "libstatus_tools.so".into(),
                pinned: true,
                current: version.clone(),
                previous: Some(version),
            },
        );
        lock.save(&path).unwrap();
        assert_eq!(MarketplaceLock::load(&path).unwrap().plugins, lock.plugins);
        fs::remove_dir_all(root).unwrap();
    }

    fn temp_root() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "qimen-marketplace-install-{}-{nonce}",
            std::process::id()
        ))
    }

    fn sha256_bytes(bytes: &[u8]) -> String {
        lower_hex(&Sha256::digest(bytes))
    }
}
