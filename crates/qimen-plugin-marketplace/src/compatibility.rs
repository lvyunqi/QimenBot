use crate::model::parse_short_version;
use crate::{
    CatalogPlugin, MarketplaceError, PluginAsset, PluginKind, ReleaseChannel, Result,
    VersionManifest,
};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostProfile {
    pub qimenbot_version: String,
    pub target: String,
    pub os: String,
    pub arch: String,
    pub environment: String,
    pub glibc: Option<String>,
    pub dynamic_loading: bool,
    pub supported_dynamic_apis: Vec<String>,
}

impl HostProfile {
    pub fn current(qimenbot_version: impl Into<String>) -> Self {
        let os = std::env::consts::OS.to_string();
        let arch = std::env::consts::ARCH.to_string();
        let environment = current_environment().to_string();
        let target = target_triple(&os, &arch, &environment)
            .unwrap_or_else(|| format!("{arch}-unknown-{os}-{environment}"));
        let dynamic_loading = !cfg!(all(target_os = "linux", target_env = "musl"));
        Self {
            qimenbot_version: qimenbot_version.into(),
            target,
            os,
            arch,
            environment,
            glibc: current_glibc_version(),
            dynamic_loading,
            supported_dynamic_apis: ["0.1", "0.2", "0.3", "0.4", "0.5", "0.6"]
                .into_iter()
                .map(str::to_string)
                .collect(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        Version::parse(&self.qimenbot_version).map_err(|error| {
            MarketplaceError::InvalidMetadata(format!(
                "host QimenBot version '{}' is not SemVer: {error}",
                self.qimenbot_version
            ))
        })?;
        if let Some(glibc) = &self.glibc {
            parse_short_version(glibc, "host glibc")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompatibilityIssue {
    pub code: String,
    pub message: String,
}

impl CompatibilityIssue {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionCompatibility {
    pub version: String,
    pub compatible: bool,
    pub installable: bool,
    pub asset: Option<PluginAsset>,
    pub issues: Vec<CompatibilityIssue>,
}

pub fn evaluate_version(
    plugin: &CatalogPlugin,
    version: &VersionManifest,
    host: &HostProfile,
    allow_prerelease: bool,
) -> VersionCompatibility {
    let mut issues = Vec::new();
    let host_version = Version::parse(&host.qimenbot_version);
    let plugin_version = version.parsed_version();

    if version.yanked {
        issues.push(CompatibilityIssue::new(
            "yanked",
            "该版本已被商城撤回，只保留历史记录。",
        ));
    }
    if matches!(version.channel, ReleaseChannel::Prerelease) && !allow_prerelease {
        issues.push(CompatibilityIssue::new(
            "prerelease_disabled",
            "当前商城配置不接收预发布版本。",
        ));
    }
    match (host_version, version.qimenbot_requirement()) {
        (Ok(host_version), Ok(requirement)) if !requirement.matches(&host_version) => {
            issues.push(CompatibilityIssue::new(
                "qimenbot_version",
                format!(
                    "需要 QimenBot {}，当前版本为 {}。",
                    version.qimenbot, host.qimenbot_version
                ),
            ));
        }
        (Err(error), _) => issues.push(CompatibilityIssue::new(
            "host_version_invalid",
            format!("无法识别当前 QimenBot 版本：{error}"),
        )),
        (_, Err(error)) => issues.push(CompatibilityIssue::new(
            "requirement_invalid",
            error.to_string(),
        )),
        _ => {}
    }
    if let Err(error) = plugin_version {
        issues.push(CompatibilityIssue::new(
            "version_invalid",
            error.to_string(),
        ));
    }

    if matches!(plugin.manifest.kind, PluginKind::Static) {
        issues.push(CompatibilityIssue::new(
            "static_rebuild_required",
            "静态插件需要加入源码并重新构建 qimenbotd，不能在线安装。",
        ));
        let compatible = issues
            .iter()
            .all(|issue| issue.code == "static_rebuild_required");
        return VersionCompatibility {
            version: version.version.clone(),
            compatible,
            installable: false,
            asset: None,
            issues,
        };
    }

    if !host.dynamic_loading {
        issues.push(CompatibilityIssue::new(
            "dynamic_loading_unavailable",
            "当前宿主是 Linux musl 静态包，不能加载动态插件。请改用 GNU 包或 Docker。",
        ));
    }
    if let Some(api) = &version.dynamic_api
        && !host
            .supported_dynamic_apis
            .iter()
            .any(|supported| supported == api)
    {
        issues.push(CompatibilityIssue::new(
            "dynamic_api",
            format!("插件使用动态 API {api}，当前宿主不支持。"),
        ));
    }

    let asset = version
        .assets
        .iter()
        .find(|asset| asset.target == host.target)
        .cloned();
    if asset.is_none() {
        issues.push(CompatibilityIssue::new(
            "target",
            format!("该版本没有提供 {} 构建。", host.target),
        ));
    }
    if let Some(asset) = &asset
        && let Some(required) = asset.min_glibc.as_deref()
    {
        match host.glibc.as_deref() {
            Some(actual) => match (
                parse_short_version(required, "min_glibc"),
                parse_short_version(actual, "host glibc"),
            ) {
                (Ok(required), Ok(actual)) if actual < required => {
                    issues.push(CompatibilityIssue::new(
                        "glibc",
                        format!(
                            "插件至少需要 glibc {}，当前系统为 glibc {}。",
                            asset.min_glibc.as_deref().unwrap_or_default(),
                            host.glibc.as_deref().unwrap_or_default()
                        ),
                    ));
                }
                (Err(error), _) | (_, Err(error)) => {
                    issues.push(CompatibilityIssue::new("glibc_invalid", error.to_string()))
                }
                _ => {}
            },
            None => issues.push(CompatibilityIssue::new(
                "glibc_unknown",
                "无法读取当前 glibc 版本，为避免安装后无法加载，已停止自动安装。",
            )),
        }
    }

    VersionCompatibility {
        version: version.version.clone(),
        compatible: issues.is_empty(),
        installable: issues.is_empty() && asset.is_some(),
        asset,
        issues,
    }
}

pub fn compatible_versions<'a>(
    plugin: &'a CatalogPlugin,
    host: &HostProfile,
    allow_prerelease: bool,
) -> Vec<(&'a VersionManifest, VersionCompatibility)> {
    let mut versions = plugin
        .versions
        .iter()
        .map(|version| {
            (
                version,
                evaluate_version(plugin, version, host, allow_prerelease),
            )
        })
        .collect::<Vec<_>>();
    versions.sort_by(|(left, _), (right, _)| compare_versions_desc(left, right));
    versions
}

pub fn select_latest_compatible<'a>(
    plugin: &'a CatalogPlugin,
    host: &HostProfile,
    allow_prerelease: bool,
) -> Option<(&'a VersionManifest, VersionCompatibility)> {
    compatible_versions(plugin, host, allow_prerelease)
        .into_iter()
        .find(|(_, compatibility)| compatibility.installable)
}

fn compare_versions_desc(left: &VersionManifest, right: &VersionManifest) -> Ordering {
    match (left.parsed_version(), right.parsed_version()) {
        (Ok(left), Ok(right)) => right.cmp_precedence(&left),
        _ => right.version.cmp(&left.version),
    }
}

fn current_environment() -> &'static str {
    if cfg!(target_env = "gnu") {
        "gnu"
    } else if cfg!(target_env = "musl") {
        "musl"
    } else if cfg!(target_env = "msvc") {
        "msvc"
    } else {
        "unknown"
    }
}

fn target_triple(os: &str, arch: &str, environment: &str) -> Option<String> {
    let target = match (os, arch, environment) {
        ("windows", "x86_64", "msvc") => "x86_64-pc-windows-msvc",
        ("windows", "aarch64", "msvc") => "aarch64-pc-windows-msvc",
        ("linux", "x86_64", "gnu") => "x86_64-unknown-linux-gnu",
        ("linux", "aarch64", "gnu") => "aarch64-unknown-linux-gnu",
        ("linux", "x86_64", "musl") => "x86_64-unknown-linux-musl",
        ("linux", "aarch64", "musl") => "aarch64-unknown-linux-musl",
        ("macos", "x86_64", _) => "x86_64-apple-darwin",
        ("macos", "aarch64", _) => "aarch64-apple-darwin",
        _ => return None,
    };
    Some(target.to_string())
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn current_glibc_version() -> Option<String> {
    use std::ffi::{CStr, c_char};

    unsafe extern "C" {
        fn gnu_get_libc_version() -> *const c_char;
    }

    let pointer = unsafe { gnu_get_libc_version() };
    if pointer.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(pointer) }
        .to_str()
        .ok()
        .map(str::to_string)
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn current_glibc_version() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DriverEventKind, DriverSupport, MessageScene, OutboundCapability, PluginDriver,
        PluginManifest, TrustLevel,
    };

    fn catalog() -> CatalogPlugin {
        let manifest = PluginManifest {
            schema_version: 1,
            id: "status-tools".into(),
            name: "Status Tools".into(),
            summary: "Reports status".into(),
            description: String::new(),
            kind: PluginKind::Dynamic,
            repository: "example/status-tools".into(),
            repository_id: 42,
            license: "MIT".into(),
            authors: Vec::new(),
            categories: Vec::new(),
            keywords: Vec::new(),
            homepage: None,
            trust: TrustLevel::Community,
        };
        let version = |value: &str, min_glibc: &str| VersionManifest {
            schema_version: 1,
            version: value.into(),
            released_at: "2026-08-03T00:00:00Z".into(),
            release_tag: format!("v{value}"),
            channel: ReleaseChannel::Stable,
            qimenbot: ">=0.1.16, <0.2.0".into(),
            dynamic_api: Some("0.5".into()),
            yanked: false,
            data_schema_version: 1,
            rollback_safe: true,
            changelog: String::new(),
            drivers: vec![DriverSupport {
                driver: PluginDriver::OneBot11,
                scenes: vec![MessageScene::Private, MessageScene::Group],
                events: vec![DriverEventKind::Message],
                outbound: vec![OutboundCapability::Reply],
            }],
            assets: vec![PluginAsset {
                target: "x86_64-unknown-linux-gnu".into(),
                asset_name: "libqimen_dynamic_plugin_status_tools-x86_64-unknown-linux-gnu.so"
                    .into(),
                sha256: "a".repeat(64),
                size_bytes: 10,
                min_glibc: Some(min_glibc.into()),
                github_attestation: false,
            }],
        };
        CatalogPlugin {
            manifest,
            versions: vec![version("1.0.0", "2.31"), version("1.1.0", "2.39")],
        }
    }

    fn host() -> HostProfile {
        HostProfile {
            qimenbot_version: "0.1.16".into(),
            target: "x86_64-unknown-linux-gnu".into(),
            os: "linux".into(),
            arch: "x86_64".into(),
            environment: "gnu".into(),
            glibc: Some("2.31".into()),
            dynamic_loading: true,
            supported_dynamic_apis: vec!["0.5".into()],
        }
    }

    #[test]
    fn selects_highest_version_after_compatibility_filtering() {
        let catalog = catalog();
        let selected = select_latest_compatible(&catalog, &host(), false).unwrap();
        assert_eq!(selected.0.version, "1.0.0");
        assert!(selected.1.installable);
    }

    #[test]
    fn current_host_accepts_online_config_api() {
        let mut catalog = catalog();
        catalog.versions[0].dynamic_api = Some("0.6".into());
        let mut host = host();
        host.supported_dynamic_apis.push("0.6".into());
        assert!(evaluate_version(&catalog, &catalog.versions[0], &host, false).compatible);
    }

    #[test]
    fn musl_host_never_claims_dynamic_install_support() {
        let catalog = catalog();
        let mut host = host();
        host.target = "x86_64-unknown-linux-musl".into();
        host.environment = "musl".into();
        host.dynamic_loading = false;
        assert!(select_latest_compatible(&catalog, &host, false).is_none());
    }

    #[test]
    fn prerelease_numeric_identifiers_follow_semver_precedence() {
        let mut catalog = catalog();
        catalog.versions[0].version = "1.0.0-alpha.2".into();
        catalog.versions[0].channel = ReleaseChannel::Prerelease;
        catalog.versions[1].version = "1.0.0-alpha.10".into();
        catalog.versions[1].channel = ReleaseChannel::Prerelease;
        let mut host = host();
        host.glibc = Some("2.39".into());

        let selected = select_latest_compatible(&catalog, &host, true).unwrap();
        assert_eq!(selected.0.version, "1.0.0-alpha.10");
    }
}
