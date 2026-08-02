use crate::DynError;
use crate::config::UpdateChannel;
use futures_util::StreamExt;
use qimen_update_protocol::{UpdatePhase, UpdateStatus, write_status};
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    pub version: Version,
    pub tag: String,
    pub html_url: String,
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Clone)]
pub struct GithubClient {
    client: reqwest::Client,
    repository: String,
    channel: UpdateChannel,
    authorization: Option<HeaderValue>,
}

impl GithubClient {
    pub fn new(
        repository: impl Into<String>,
        channel: UpdateChannel,
        timeout: Duration,
    ) -> Result<Self, DynError> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("QimenBot-Updater"));
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );
        headers.insert(
            "x-github-api-version",
            HeaderValue::from_static("2022-11-28"),
        );
        let authorization = std::env::var("GITHUB_TOKEN")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(|token| HeaderValue::from_str(&format!("Bearer {}", token.trim())))
            .transpose()?;
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(timeout)
            .build()?;
        Ok(Self {
            client,
            repository: repository.into(),
            channel,
            authorization,
        })
    }

    /// 查询比当前版本更新的 GitHub Release。
    pub async fn find_update(&self, current: &Version) -> Result<Option<ReleaseInfo>, DynError> {
        let url = format!("https://api.github.com/repos/{}/releases", self.repository);
        let releases = self
            .request(&url)
            .query(&[("per_page", "20")])
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<GithubRelease>>()
            .await?;

        let mut candidates = releases
            .into_iter()
            .filter(|release| {
                !release.draft && (self.channel.allows_prerelease() || !release.prerelease)
            })
            .filter_map(|release| {
                let version = Version::parse(release.tag_name.trim_start_matches('v')).ok()?;
                (version > *current).then_some(ReleaseInfo {
                    version,
                    tag: release.tag_name,
                    html_url: release.html_url,
                    assets: release.assets,
                })
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| right.version.cmp(&left.version));
        Ok(candidates.into_iter().next())
    }

    /// 下载组件并校验同名 `.sha256` Release 资产。
    pub async fn download_daemon(
        &self,
        release: &ReleaseInfo,
        target: &str,
        destination: &Path,
        update_dir: &Path,
        status: &mut UpdateStatus,
    ) -> Result<PathBuf, DynError> {
        let component = "qimenbotd";
        let asset_name = executable_asset_name(component, &release.tag, target);
        let checksum_name = format!("{asset_name}.sha256");
        let asset = find_asset(&release.assets, &asset_name)?;
        let checksum_asset = find_asset(&release.assets, &checksum_name)?;
        let expected_checksum = self
            .request(&checksum_asset.browser_download_url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?
            .split_whitespace()
            .next()
            .filter(|value| value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit()))
            .ok_or("release checksum asset is invalid")?
            .to_ascii_lowercase();

        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let temporary = destination.with_extension("download");
        let response = self
            .request(&asset.browser_download_url)
            .send()
            .await?
            .error_for_status()?;
        let mut stream = response.bytes_stream();
        let mut file = tokio::fs::File::create(&temporary).await?;
        let mut hasher = Sha256::new();
        let mut downloaded = 0_u64;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            hasher.update(&chunk);
            downloaded = downloaded.saturating_add(chunk.len() as u64);
            if asset.size > 0 {
                let fraction = downloaded.min(asset.size) as f64 / asset.size as f64;
                status.progress_percent = Some((fraction * 100.0).round().clamp(0.0, 100.0) as u8);
                status.set_phase(UpdatePhase::Downloading, format!("正在下载 {component}"));
                write_status(update_dir, status)?;
            }
        }
        file.flush().await?;
        drop(file);

        let actual_checksum = lower_hex(&hasher.finalize());
        if actual_checksum != expected_checksum {
            let _ = tokio::fs::remove_file(&temporary).await;
            return Err(format!(
                "checksum mismatch for {asset_name}: expected {expected_checksum}, got {actual_checksum}"
            )
            .into());
        }
        if destination.exists() {
            tokio::fs::remove_file(destination).await?;
        }
        tokio::fs::rename(temporary, destination).await?;
        set_executable(destination).await?;
        Ok(destination.to_path_buf())
    }

    fn request(&self, url: &str) -> reqwest::RequestBuilder {
        let request = self.client.get(url);
        if let Some(authorization) = &self.authorization {
            request.header(AUTHORIZATION, authorization.clone())
        } else {
            request
        }
    }
}

fn find_asset<'a>(assets: &'a [ReleaseAsset], name: &str) -> Result<&'a ReleaseAsset, DynError> {
    assets
        .iter()
        .find(|asset| asset.name == name)
        .ok_or_else(|| format!("release asset '{name}' was not found").into())
}

fn executable_asset_name(component: &str, release_tag: &str, target: &str) -> String {
    let executable_suffix = if target.contains("windows") {
        ".exe"
    } else {
        ""
    };
    format!("{component}-{release_tag}-{target}{executable_suffix}")
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

#[cfg(unix)]
async fn set_executable(path: &Path) -> Result<(), DynError> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = tokio::fs::metadata(path).await?.permissions();
    permissions.set_mode(0o755);
    tokio::fs::set_permissions(path, permissions).await?;
    Ok(())
}

#[cfg(not(unix))]
async fn set_executable(_path: &Path) -> Result<(), DynError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_asset_names_match_workflow_output() {
        assert_eq!(
            executable_asset_name("qimenbotd", "v0.2.0", "x86_64-pc-windows-msvc"),
            "qimenbotd-v0.2.0-x86_64-pc-windows-msvc.exe"
        );
        assert_eq!(
            executable_asset_name("qimenbotd", "v0.2.0", "aarch64-unknown-linux-gnu"),
            "qimenbotd-v0.2.0-aarch64-unknown-linux-gnu"
        );
    }

    #[test]
    fn digest_bytes_are_rendered_as_lower_hex() {
        assert_eq!(lower_hex(&[0x00, 0xab, 0xff]), "00abff");
    }
}
