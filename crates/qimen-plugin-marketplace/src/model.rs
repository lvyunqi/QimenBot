use crate::{MarketplaceError, Result};
use chrono::DateTime;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Component, Path};

pub const CATALOG_SCHEMA_VERSION: u32 = 1;
pub const PLUGIN_SCHEMA_VERSION: u32 = 1;
pub const VERSION_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginKind {
    Dynamic,
    Static,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrustLevel {
    #[default]
    Community,
    VerifiedBuild,
    Official,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReleaseChannel {
    Stable,
    Prerelease,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginDriver {
    #[serde(rename = "onebot11")]
    OneBot11,
    #[serde(rename = "qq-official")]
    QqOfficial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MessageScene {
    Private,
    Group,
    GroupAt,
    Channel,
    ChannelAt,
    ChannelPrivate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DriverEventKind {
    Message,
    Notice,
    Request,
    Meta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutboundCapability {
    Reply,
    Proactive,
    RichMessage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DriverSupport {
    pub driver: PluginDriver,
    pub scenes: Vec<MessageScene>,
    #[serde(default)]
    pub events: Vec<DriverEventKind>,
    #[serde(default)]
    pub outbound: Vec<OutboundCapability>,
}

impl DriverSupport {
    fn validate(&self, plugin_id: &str, version: &str) -> Result<()> {
        if self.scenes.is_empty() {
            return Err(MarketplaceError::InvalidMetadata(format!(
                "plugin '{plugin_id}' version '{version}' driver {:?} must declare at least one message scene",
                self.driver
            )));
        }
        if self.events.is_empty() && self.outbound.is_empty() {
            return Err(MarketplaceError::InvalidMetadata(format!(
                "plugin '{plugin_id}' version '{version}' driver {:?} must declare events or outbound capabilities",
                self.driver
            )));
        }
        validate_unique_values(&self.scenes, "driver scenes", plugin_id, version)?;
        validate_unique_values(&self.events, "driver events", plugin_id, version)?;
        validate_unique_values(&self.outbound, "driver outbound", plugin_id, version)?;
        if self.driver == PluginDriver::OneBot11
            && self
                .scenes
                .iter()
                .any(|scene| matches!(scene, MessageScene::GroupAt | MessageScene::ChannelAt))
        {
            return Err(MarketplaceError::InvalidMetadata(format!(
                "plugin '{plugin_id}' version '{version}' uses an official QQ @ scene for the onebot11 driver"
            )));
        }
        if self.outbound.contains(&OutboundCapability::RichMessage)
            && !self.outbound.iter().any(|capability| {
                matches!(
                    capability,
                    OutboundCapability::Reply | OutboundCapability::Proactive
                )
            })
        {
            return Err(MarketplaceError::InvalidMetadata(format!(
                "plugin '{plugin_id}' version '{version}' declares rich-message without reply or proactive sending"
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: String,
    #[serde(rename = "type")]
    pub kind: PluginKind,
    pub repository: String,
    pub repository_id: u64,
    pub license: String,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub trust: TrustLevel,
}

impl PluginManifest {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != PLUGIN_SCHEMA_VERSION {
            return Err(MarketplaceError::InvalidMetadata(format!(
                "plugin '{}' uses unsupported schema_version {}",
                self.id, self.schema_version
            )));
        }
        validate_plugin_id(&self.id)?;
        validate_text(&self.name, "name", 80)?;
        validate_text(&self.summary, "summary", 180)?;
        if self.description.chars().count() > 4_000 {
            return Err(MarketplaceError::InvalidMetadata(format!(
                "plugin '{}' description exceeds 4000 characters",
                self.id
            )));
        }
        validate_authors(&self.authors)?;
        validate_repository(&self.repository)?;
        if self.repository_id == 0 {
            return Err(MarketplaceError::InvalidMetadata(format!(
                "plugin '{}' repository_id must be a GitHub numeric repository ID",
                self.id
            )));
        }
        let license = self.license.trim();
        if license.is_empty()
            || matches!(
                license.to_ascii_lowercase().as_str(),
                "noassertion" | "unlicensed" | "proprietary" | "none"
            )
        {
            return Err(MarketplaceError::InvalidMetadata(format!(
                "plugin '{}' must declare an explicit open-source SPDX license",
                self.id
            )));
        }
        validate_unique_labels(&self.categories, "categories", 8)?;
        validate_unique_labels(&self.keywords, "keywords", 12)?;
        if let Some(homepage) = &self.homepage {
            validate_https_url(homepage, "homepage")?;
        }
        Ok(())
    }

    pub fn repository_url(&self) -> String {
        format!("https://github.com/{}", self.repository)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PluginAsset {
    pub target: String,
    pub asset_name: String,
    pub sha256: String,
    pub size_bytes: u64,
    #[serde(default)]
    pub min_glibc: Option<String>,
    #[serde(default)]
    pub github_attestation: bool,
}

impl PluginAsset {
    pub fn validate(&self, plugin_id: &str) -> Result<()> {
        validate_target(&self.target)?;
        validate_file_name(&self.asset_name)?;
        let expected_extension = dynamic_library_extension(&self.target).ok_or_else(|| {
            MarketplaceError::InvalidMetadata(format!(
                "plugin '{plugin_id}' target '{}' cannot host a dynamic library",
                self.target
            ))
        })?;
        if Path::new(&self.asset_name)
            .extension()
            .and_then(|value| value.to_str())
            != Some(expected_extension)
        {
            return Err(MarketplaceError::InvalidMetadata(format!(
                "plugin '{plugin_id}' asset '{}' must use .{} for target '{}'",
                self.asset_name, expected_extension, self.target
            )));
        }
        let expected_name = marketplace_asset_name(plugin_id, &self.target)?;
        if self.asset_name != expected_name {
            return Err(MarketplaceError::InvalidMetadata(format!(
                "plugin '{plugin_id}' asset '{}' must be named '{}' for target '{}'",
                self.asset_name, expected_name, self.target
            )));
        }
        if self.sha256.len() != 64
            || !self
                .sha256
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        {
            return Err(MarketplaceError::InvalidMetadata(format!(
                "plugin '{plugin_id}' asset '{}' has an invalid SHA256",
                self.asset_name
            )));
        }
        if self.size_bytes == 0 {
            return Err(MarketplaceError::InvalidMetadata(format!(
                "plugin '{plugin_id}' asset '{}' must declare a non-zero size_bytes",
                self.asset_name
            )));
        }
        if self.target.ends_with("-linux-gnu") {
            let glibc = self.min_glibc.as_deref().ok_or_else(|| {
                MarketplaceError::InvalidMetadata(format!(
                    "plugin '{plugin_id}' GNU/Linux asset '{}' must declare min_glibc",
                    self.asset_name
                ))
            })?;
            parse_short_version(glibc, "min_glibc")?;
        } else if self.min_glibc.is_some() {
            return Err(MarketplaceError::InvalidMetadata(format!(
                "plugin '{plugin_id}' asset '{}' declares min_glibc for a non-GNU target",
                self.asset_name
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionManifest {
    pub schema_version: u32,
    pub version: String,
    pub released_at: String,
    pub release_tag: String,
    pub channel: ReleaseChannel,
    pub qimenbot: String,
    #[serde(default)]
    pub dynamic_api: Option<String>,
    #[serde(default)]
    pub yanked: bool,
    #[serde(default)]
    pub data_schema_version: u32,
    #[serde(default)]
    pub rollback_safe: bool,
    #[serde(default)]
    pub changelog: String,
    pub drivers: Vec<DriverSupport>,
    #[serde(default)]
    pub assets: Vec<PluginAsset>,
}

impl VersionManifest {
    pub fn parsed_version(&self) -> Result<Version> {
        Version::parse(&self.version).map_err(|error| {
            MarketplaceError::InvalidMetadata(format!(
                "version '{}' is not valid SemVer: {error}",
                self.version
            ))
        })
    }

    pub fn qimenbot_requirement(&self) -> Result<VersionReq> {
        VersionReq::parse(&self.qimenbot).map_err(|error| {
            MarketplaceError::InvalidMetadata(format!(
                "version '{}' has an invalid qimenbot requirement '{}': {error}",
                self.version, self.qimenbot
            ))
        })
    }

    pub fn validate(&self, plugin: &PluginManifest) -> Result<()> {
        if self.schema_version != VERSION_SCHEMA_VERSION {
            return Err(MarketplaceError::InvalidMetadata(format!(
                "plugin '{}' version '{}' uses unsupported schema_version {}",
                plugin.id, self.version, self.schema_version
            )));
        }
        let version = self.parsed_version()?;
        self.qimenbot_requirement()?;
        DateTime::parse_from_rfc3339(&self.released_at).map_err(|error| {
            MarketplaceError::InvalidMetadata(format!(
                "plugin '{}' version '{}' has an invalid released_at: {error}",
                plugin.id, self.version
            ))
        })?;
        validate_release_tag(&self.release_tag)?;
        match self.channel {
            ReleaseChannel::Stable if !version.pre.is_empty() => {
                return Err(MarketplaceError::InvalidMetadata(format!(
                    "plugin '{}' version '{}' is prerelease SemVer but declares stable channel",
                    plugin.id, self.version
                )));
            }
            ReleaseChannel::Prerelease if version.pre.is_empty() => {
                return Err(MarketplaceError::InvalidMetadata(format!(
                    "plugin '{}' version '{}' declares prerelease channel without a SemVer prerelease",
                    plugin.id, self.version
                )));
            }
            _ => {}
        }
        if self.changelog.chars().count() > 4_000 {
            return Err(MarketplaceError::InvalidMetadata(format!(
                "plugin '{}' version '{}' changelog exceeds 4000 characters",
                plugin.id, self.version
            )));
        }
        if self.drivers.is_empty() || self.drivers.len() > 2 {
            return Err(MarketplaceError::InvalidMetadata(format!(
                "plugin '{}' version '{}' must declare one or two supported drivers",
                plugin.id, self.version
            )));
        }
        let mut drivers = HashSet::new();
        for driver in &self.drivers {
            driver.validate(&plugin.id, &self.version)?;
            if !drivers.insert(driver.driver) {
                return Err(MarketplaceError::InvalidMetadata(format!(
                    "plugin '{}' version '{}' declares driver {:?} more than once",
                    plugin.id, self.version, driver.driver
                )));
            }
        }
        match plugin.kind {
            PluginKind::Dynamic => {
                let api = self.dynamic_api.as_deref().ok_or_else(|| {
                    MarketplaceError::InvalidMetadata(format!(
                        "dynamic plugin '{}' version '{}' must declare dynamic_api",
                        plugin.id, self.version
                    ))
                })?;
                if !matches!(api, "0.1" | "0.2" | "0.3" | "0.4" | "0.5") {
                    return Err(MarketplaceError::InvalidMetadata(format!(
                        "dynamic plugin '{}' version '{}' declares unsupported dynamic_api '{}'",
                        plugin.id, self.version, api
                    )));
                }
                if self.assets.is_empty() {
                    return Err(MarketplaceError::InvalidMetadata(format!(
                        "dynamic plugin '{}' version '{}' must declare at least one release asset",
                        plugin.id, self.version
                    )));
                }
            }
            PluginKind::Static => {
                if self.dynamic_api.is_some() || !self.assets.is_empty() {
                    return Err(MarketplaceError::InvalidMetadata(format!(
                        "static plugin '{}' version '{}' cannot declare dynamic_api or installable assets",
                        plugin.id, self.version
                    )));
                }
            }
        }
        let mut targets = HashSet::new();
        let mut names = HashSet::new();
        for asset in &self.assets {
            asset.validate(&plugin.id)?;
            if !targets.insert(asset.target.as_str()) {
                return Err(MarketplaceError::InvalidMetadata(format!(
                    "plugin '{}' version '{}' declares target '{}' more than once",
                    plugin.id, self.version, asset.target
                )));
            }
            if !names.insert(asset.asset_name.as_str()) {
                return Err(MarketplaceError::InvalidMetadata(format!(
                    "plugin '{}' version '{}' declares asset '{}' more than once",
                    plugin.id, self.version, asset.asset_name
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogPlugin {
    #[serde(flatten)]
    pub manifest: PluginManifest,
    pub versions: Vec<VersionManifest>,
}

impl CatalogPlugin {
    pub fn validate(&self) -> Result<()> {
        self.manifest.validate()?;
        if self.versions.is_empty() {
            return Err(MarketplaceError::InvalidMetadata(format!(
                "plugin '{}' has no registered versions",
                self.manifest.id
            )));
        }
        let mut versions = HashSet::new();
        let mut precedence_versions = Vec::<(String, Version)>::new();
        for version in &self.versions {
            version.validate(&self.manifest)?;
            if !versions.insert(version.version.as_str()) {
                return Err(MarketplaceError::InvalidMetadata(format!(
                    "plugin '{}' registers version '{}' more than once",
                    self.manifest.id, version.version
                )));
            }
            let parsed = version.parsed_version()?;
            if let Some((existing, _)) = precedence_versions
                .iter()
                .find(|(_, item)| item.cmp_precedence(&parsed).is_eq())
            {
                return Err(MarketplaceError::InvalidMetadata(format!(
                    "plugin '{}' versions '{}' and '{}' have the same SemVer precedence; publish a higher prerelease or patch version instead",
                    self.manifest.id, existing, version.version
                )));
            }
            precedence_versions.push((version.version.clone(), parsed));
        }
        Ok(())
    }

    pub fn version(&self, version: &str) -> Option<&VersionManifest> {
        self.versions.iter().find(|item| item.version == version)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogIndex {
    pub schema_version: u32,
    pub plugins: Vec<CatalogPlugin>,
}

impl CatalogIndex {
    pub fn empty() -> Self {
        Self {
            schema_version: CATALOG_SCHEMA_VERSION,
            plugins: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != CATALOG_SCHEMA_VERSION {
            return Err(MarketplaceError::InvalidMetadata(format!(
                "catalog uses unsupported schema_version {}",
                self.schema_version
            )));
        }
        let mut ids = HashSet::new();
        for plugin in &self.plugins {
            plugin.validate()?;
            if !ids.insert(plugin.manifest.id.as_str()) {
                return Err(MarketplaceError::InvalidMetadata(format!(
                    "catalog contains duplicate plugin ID '{}'",
                    plugin.manifest.id
                )));
            }
        }
        Ok(())
    }

    pub fn plugin(&self, id: &str) -> Option<&CatalogPlugin> {
        self.plugins.iter().find(|plugin| plugin.manifest.id == id)
    }
}

pub fn validate_plugin_id(id: &str) -> Result<()> {
    let valid = (2..=64).contains(&id.len())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && id.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
        && id.as_bytes().last().is_some_and(u8::is_ascii_alphanumeric)
        && !id.contains("--");
    if !valid {
        return Err(MarketplaceError::InvalidMetadata(format!(
            "plugin ID '{id}' must be 2-64 lowercase ASCII letters, digits, or single hyphens"
        )));
    }
    Ok(())
}

pub fn validate_file_name(name: &str) -> Result<()> {
    let path = Path::new(name);
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let windows_reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'));
    if name.is_empty()
        || name.len() > 180
        || path.components().count() != 1
        || !matches!(path.components().next(), Some(Component::Normal(_)))
        || !name
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        || name.ends_with('.')
        || name.contains("..")
        || windows_reserved
    {
        return Err(MarketplaceError::InvalidMetadata(format!(
            "file name '{name}' must be a portable ASCII file name"
        )));
    }
    Ok(())
}

pub fn dynamic_library_extension(target: &str) -> Option<&'static str> {
    if target.contains("windows-msvc") {
        Some("dll")
    } else if target.contains("apple-darwin") {
        Some("dylib")
    } else if target.ends_with("linux-gnu") {
        Some("so")
    } else {
        None
    }
}

pub fn marketplace_asset_name(plugin_id: &str, target: &str) -> Result<String> {
    validate_plugin_id(plugin_id)?;
    validate_target(target)?;
    let extension = dynamic_library_extension(target).ok_or_else(|| {
        MarketplaceError::InvalidMetadata(format!(
            "target '{target}' cannot host a marketplace dynamic library"
        ))
    })?;
    let normalized = plugin_id.replace('-', "_");
    let prefix = if extension == "dll" { "" } else { "lib" };
    Ok(format!(
        "{prefix}qimen_dynamic_plugin_{normalized}-{target}.{extension}"
    ))
}

pub fn parse_short_version(value: &str, field: &str) -> Result<Version> {
    let normalized = match value.matches('.').count() {
        1 => format!("{value}.0"),
        2 => value.to_string(),
        _ => {
            return Err(MarketplaceError::InvalidMetadata(format!(
                "{field} '{value}' must use major.minor or major.minor.patch"
            )));
        }
    };
    Version::parse(&normalized).map_err(|error| {
        MarketplaceError::InvalidMetadata(format!("invalid {field} '{value}': {error}"))
    })
}

fn validate_target(target: &str) -> Result<()> {
    const TARGETS: &[&str] = &[
        "x86_64-pc-windows-msvc",
        "aarch64-pc-windows-msvc",
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
    ];
    if !TARGETS.contains(&target) {
        return Err(MarketplaceError::InvalidMetadata(format!(
            "unsupported dynamic plugin target '{target}'"
        )));
    }
    Ok(())
}

fn validate_repository(repository: &str) -> Result<()> {
    let mut parts = repository.split('/');
    let owner = parts.next().unwrap_or_default();
    let name = parts.next().unwrap_or_default();
    let valid_part = |part: &str| {
        !part.is_empty()
            && part.len() <= 100
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
            && part != "."
            && part != ".."
    };
    if parts.next().is_some() || !valid_part(owner) || !valid_part(name) {
        return Err(MarketplaceError::InvalidMetadata(format!(
            "repository '{repository}' must use the GitHub owner/name form"
        )));
    }
    Ok(())
}

fn validate_release_tag(tag: &str) -> Result<()> {
    if tag.is_empty()
        || tag.len() > 120
        || tag.starts_with('.')
        || tag.ends_with('.')
        || tag.contains("..")
        || !tag
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(MarketplaceError::InvalidMetadata(format!(
            "release_tag '{tag}' may contain only ASCII letters, digits, '.', '_' and '-'"
        )));
    }
    Ok(())
}

fn validate_text(value: &str, field: &str, max_chars: usize) -> Result<()> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().count() > max_chars || trimmed.contains(['\r', '\n']) {
        return Err(MarketplaceError::InvalidMetadata(format!(
            "{field} must be one non-empty line no longer than {max_chars} characters"
        )));
    }
    Ok(())
}

fn validate_unique_labels(values: &[String], field: &str, max_items: usize) -> Result<()> {
    if values.len() > max_items {
        return Err(MarketplaceError::InvalidMetadata(format!(
            "{field} may contain at most {max_items} values"
        )));
    }
    let mut seen = HashSet::new();
    for value in values {
        let normalized = value.trim().to_ascii_lowercase();
        if normalized.is_empty()
            || normalized.len() > 40
            || !normalized
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            || !seen.insert(normalized)
        {
            return Err(MarketplaceError::InvalidMetadata(format!(
                "{field} contains an invalid or duplicate value '{value}'"
            )));
        }
    }
    Ok(())
}

fn validate_authors(values: &[String]) -> Result<()> {
    if values.len() > 16 {
        return Err(MarketplaceError::InvalidMetadata(
            "authors may contain at most 16 values".to_string(),
        ));
    }
    let mut seen = HashSet::new();
    for value in values {
        let normalized = value.trim().to_lowercase();
        if normalized.is_empty()
            || normalized.chars().count() > 80
            || normalized.contains(['\r', '\n'])
            || !seen.insert(normalized)
        {
            return Err(MarketplaceError::InvalidMetadata(format!(
                "authors contains an invalid or duplicate value '{value}'"
            )));
        }
    }
    Ok(())
}

fn validate_unique_values<T>(
    values: &[T],
    field: &str,
    plugin_id: &str,
    version: &str,
) -> Result<()>
where
    T: Copy + Eq + std::hash::Hash,
{
    let mut seen = HashSet::new();
    if values.iter().copied().all(|value| seen.insert(value)) {
        Ok(())
    } else {
        Err(MarketplaceError::InvalidMetadata(format!(
            "plugin '{plugin_id}' version '{version}' contains duplicate {field} values"
        )))
    }
}

fn validate_https_url(value: &str, field: &str) -> Result<()> {
    let url = reqwest::Url::parse(value).map_err(|error| {
        MarketplaceError::InvalidMetadata(format!("invalid {field} URL '{value}': {error}"))
    })?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || url.username() != ""
        || url.password().is_some()
    {
        return Err(MarketplaceError::InvalidMetadata(format!(
            "{field} URL must use HTTPS without embedded credentials"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plugin(kind: PluginKind) -> PluginManifest {
        PluginManifest {
            schema_version: 1,
            id: "status-tools".into(),
            name: "Status Tools".into(),
            summary: "Reports runtime status.".into(),
            description: String::new(),
            kind,
            repository: "example/status-tools".into(),
            repository_id: 42,
            license: "MIT".into(),
            authors: vec!["Example".into()],
            categories: vec!["operations".into()],
            keywords: vec!["status".into()],
            homepage: None,
            trust: TrustLevel::Community,
        }
    }

    fn dynamic_version() -> VersionManifest {
        VersionManifest {
            schema_version: 1,
            version: "1.2.3".into(),
            released_at: "2026-08-03T00:00:00Z".into(),
            release_tag: "v1.2.3".into(),
            channel: ReleaseChannel::Stable,
            qimenbot: ">=0.1.16, <0.2.0".into(),
            dynamic_api: Some("0.5".into()),
            yanked: false,
            data_schema_version: 1,
            rollback_safe: true,
            changelog: String::new(),
            drivers: vec![
                DriverSupport {
                    driver: PluginDriver::OneBot11,
                    scenes: vec![MessageScene::Private, MessageScene::Group],
                    events: vec![DriverEventKind::Message, DriverEventKind::Notice],
                    outbound: vec![OutboundCapability::Reply, OutboundCapability::Proactive],
                },
                DriverSupport {
                    driver: PluginDriver::QqOfficial,
                    scenes: vec![MessageScene::Private, MessageScene::GroupAt],
                    events: vec![DriverEventKind::Message],
                    outbound: vec![OutboundCapability::Reply],
                },
            ],
            assets: vec![PluginAsset {
                target: "x86_64-unknown-linux-gnu".into(),
                asset_name: "libqimen_dynamic_plugin_status_tools-x86_64-unknown-linux-gnu.so"
                    .into(),
                sha256: "a".repeat(64),
                size_bytes: 123,
                min_glibc: Some("2.31".into()),
                github_attestation: true,
            }],
        }
    }

    #[test]
    fn accepts_a_complete_dynamic_release() {
        dynamic_version()
            .validate(&plugin(PluginKind::Dynamic))
            .unwrap();
    }

    #[test]
    fn rejects_musl_and_path_assets() {
        let mut version = dynamic_version();
        version.assets[0].target = "x86_64-unknown-linux-musl".into();
        assert!(version.validate(&plugin(PluginKind::Dynamic)).is_err());
        version.assets[0].target = "x86_64-unknown-linux-gnu".into();
        version.assets[0].asset_name = "../plugin.so".into();
        assert!(version.validate(&plugin(PluginKind::Dynamic)).is_err());
    }

    #[test]
    fn file_names_reject_windows_devices_and_alternate_streams() {
        for invalid in ["CON.dll", "lpt1.so", "plugin.dll:payload", "../plugin.so"] {
            assert!(validate_file_name(invalid).is_err(), "{invalid}");
        }
        assert!(validate_file_name("libqimen_plugin-status_1.so").is_ok());
    }

    #[test]
    fn static_releases_cannot_claim_hot_install_assets() {
        let mut version = dynamic_version();
        version.dynamic_api = None;
        assert!(version.validate(&plugin(PluginKind::Static)).is_err());
        version.assets.clear();
        version.validate(&plugin(PluginKind::Static)).unwrap();
    }

    #[test]
    fn authors_are_bounded_and_unique_case_insensitively() {
        let mut manifest = plugin(PluginKind::Static);
        manifest.authors = vec!["Example Team".into(), "example team".into()];
        assert!(manifest.validate().is_err());

        manifest.authors = (0..17).map(|index| format!("author-{index}")).collect();
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn build_metadata_cannot_create_a_second_release_at_the_same_precedence() {
        let mut first = dynamic_version();
        first.version = "1.2.3+build.1".into();
        let mut second = dynamic_version();
        second.version = "1.2.3+build.2".into();
        let catalog = CatalogPlugin {
            manifest: plugin(PluginKind::Dynamic),
            versions: vec![first, second],
        };
        assert!(catalog.validate().is_err());
    }

    #[test]
    fn driver_matrix_rejects_duplicate_and_driver_specific_scenes() {
        let mut version = dynamic_version();
        version.drivers.push(version.drivers[0].clone());
        assert!(version.validate(&plugin(PluginKind::Dynamic)).is_err());

        version = dynamic_version();
        version.drivers[0].scenes.push(MessageScene::GroupAt);
        assert!(version.validate(&plugin(PluginKind::Dynamic)).is_err());
    }
}
