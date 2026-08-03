use crate::{
    CatalogIndex, CatalogPlugin, MarketplaceError, PluginAsset, ReleaseChannel, Result,
    VersionManifest,
};
use chrono::{SecondsFormat, Utc};
use futures_util::StreamExt;
use reqwest::header::{
    ACCEPT, AUTHORIZATION, ETAG, HeaderMap, HeaderValue, IF_NONE_MATCH, USER_AGENT,
};
use reqwest::{StatusCode, Url};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncWriteExt;

const MAX_CATALOG_BYTES: usize = 8 * 1024 * 1024;
// Generated from marketplace/ in lvyunqi/QimenBot; Pages is only the read endpoint.
const OFFICIAL_CATALOG_URL: &str = "https://lvyunqi.github.io/QimenBot/marketplace/index.json";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CatalogSource {
    Network,
    Cache,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogLoad {
    pub index: CatalogIndex,
    pub source: CatalogSource,
    pub fetched_at: Option<String>,
    pub warning: Option<String>,
    #[serde(skip)]
    etag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceVerification {
    pub plugin_id: String,
    pub repository_id: u64,
    pub valid: bool,
    pub messages: Vec<String>,
}

#[derive(Clone)]
pub struct MarketplaceClient {
    client: reqwest::Client,
    catalog_url: Url,
    github_api: Url,
    authorization: Option<HeaderValue>,
}

impl MarketplaceClient {
    pub fn new(timeout: Duration) -> Result<Self> {
        let catalog_url = validate_catalog_url(OFFICIAL_CATALOG_URL)?;
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("QimenBot-Marketplace"));
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
            .transpose()
            .map_err(|error| MarketplaceError::Network(error.to_string()))?;
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= 5 {
                    attempt.error("too many marketplace redirects")
                } else if attempt.url().scheme() == "https" {
                    attempt.follow()
                } else {
                    attempt.stop()
                }
            }))
            .build()?;
        Ok(Self {
            client,
            catalog_url,
            github_api: Url::parse("https://api.github.com/").expect("valid GitHub API URL"),
            authorization,
        })
    }

    pub async fn load_catalog(&self, cache_dir: &Path, refresh: bool) -> Result<CatalogLoad> {
        let cache_path = cache_dir.join("catalog-index.json");
        let meta_path = cache_dir.join("catalog-meta.json");
        let cached = read_cached_catalog(&cache_path, &meta_path).await;
        if !refresh && let Ok(Some(cached)) = &cached {
            return Ok(cached.clone());
        }

        match self
            .fetch_catalog(cached.as_ref().ok().and_then(Option::as_ref))
            .await
        {
            Ok(FetchCatalog::NotModified) => {
                let mut cached = cached?.ok_or_else(|| {
                    MarketplaceError::Network(
                        "catalog returned not-modified without a local cache".to_string(),
                    )
                })?;
                cached.source = CatalogSource::Network;
                cached.warning = None;
                Ok(cached)
            }
            Ok(FetchCatalog::Updated { index, etag }) => {
                let fetched_at = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
                write_catalog_cache(&cache_path, &meta_path, &index, etag.clone(), &fetched_at)
                    .await?;
                Ok(CatalogLoad {
                    index,
                    source: CatalogSource::Network,
                    fetched_at: Some(fetched_at),
                    warning: None,
                    etag,
                })
            }
            Err(error) => match cached {
                Ok(Some(mut cached)) => {
                    cached.source = CatalogSource::Cache;
                    cached.warning = Some(format!("目录刷新失败，正在使用本地缓存：{error}"));
                    Ok(cached)
                }
                _ => Err(error),
            },
        }
    }

    pub async fn verify_catalog_sources(&self, index: &CatalogIndex) -> Vec<SourceVerification> {
        let mut results = Vec::with_capacity(index.plugins.len());
        for plugin in &index.plugins {
            results.push(self.verify_plugin_source(plugin).await);
        }
        results
    }

    pub async fn download_release_asset(
        &self,
        plugin: &CatalogPlugin,
        version: &VersionManifest,
        asset: &PluginAsset,
        destination: &Path,
    ) -> Result<PathBuf> {
        let repository = self.github_repository(&plugin.manifest.repository).await?;
        if repository.id != plugin.manifest.repository_id
            || !repository
                .full_name
                .eq_ignore_ascii_case(&plugin.manifest.repository)
        {
            return Err(MarketplaceError::Conflict(format!(
                "GitHub repository identity changed for '{}': expected ID {}, got {}",
                plugin.manifest.id, plugin.manifest.repository_id, repository.id
            )));
        }
        if repository.private || repository.archived {
            return Err(MarketplaceError::Conflict(format!(
                "plugin '{}' repository is private or archived",
                plugin.manifest.id
            )));
        }

        let release = self
            .github_release(&plugin.manifest.repository, &version.release_tag)
            .await?;
        if release.draft {
            return Err(MarketplaceError::Conflict(format!(
                "release '{}' is still a draft",
                version.release_tag
            )));
        }
        if release.prerelease != matches!(version.channel, ReleaseChannel::Prerelease) {
            return Err(MarketplaceError::Conflict(format!(
                "release '{}' prerelease state differs from the reviewed catalog metadata",
                version.release_tag
            )));
        }
        let release_asset = release
            .assets
            .iter()
            .find(|candidate| candidate.name == asset.asset_name)
            .ok_or_else(|| {
                MarketplaceError::NotFound(format!(
                    "GitHub release '{}' no longer contains asset '{}'",
                    version.release_tag, asset.asset_name
                ))
            })?;
        if release_asset.size != asset.size_bytes {
            return Err(MarketplaceError::Conflict(format!(
                "release asset '{}' size changed after review: expected {}, got {}",
                asset.asset_name, asset.size_bytes, release_asset.size
            )));
        }

        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let temporary = destination.with_extension(format!("download-{}", std::process::id()));
        if temporary.exists() {
            tokio::fs::remove_file(&temporary).await?;
        }
        let url = self.github_url(&format!(
            "repos/{}/releases/assets/{}",
            plugin.manifest.repository, release_asset.id
        ))?;
        let response = self
            .authorized(self.client.get(url))
            .header(ACCEPT, "application/octet-stream")
            .send()
            .await?
            .error_for_status()?;
        let mut stream = response.bytes_stream();
        let mut file = tokio::fs::File::create(&temporary).await?;
        let mut hasher = Sha256::new();
        let mut downloaded = 0_u64;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            downloaded = downloaded.saturating_add(chunk.len() as u64);
            if downloaded > asset.size_bytes {
                drop(file);
                let _ = tokio::fs::remove_file(&temporary).await;
                return Err(MarketplaceError::Conflict(format!(
                    "release asset '{}' exceeded its reviewed size",
                    asset.asset_name
                )));
            }
            hasher.update(&chunk);
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        drop(file);
        if downloaded != asset.size_bytes {
            let _ = tokio::fs::remove_file(&temporary).await;
            return Err(MarketplaceError::Conflict(format!(
                "release asset '{}' is incomplete: expected {} bytes, got {}",
                asset.asset_name, asset.size_bytes, downloaded
            )));
        }
        let actual = lower_hex(&hasher.finalize());
        let expected = asset.sha256.to_ascii_lowercase();
        if actual != expected {
            let _ = tokio::fs::remove_file(&temporary).await;
            return Err(MarketplaceError::ChecksumMismatch { expected, actual });
        }
        atomic_replace_async(&temporary, destination).await?;
        Ok(destination.to_path_buf())
    }

    async fn fetch_catalog(&self, cached: Option<&CatalogLoad>) -> Result<FetchCatalog> {
        let mut request = self.client.get(self.catalog_url.clone());
        if let Some(etag) = cached.and_then(|cached| cached.etag.as_deref()) {
            request = request.header(IF_NONE_MATCH, etag);
        }
        let response = request.send().await?;
        if response.status() == StatusCode::NOT_MODIFIED {
            return Ok(FetchCatalog::NotModified);
        }
        let response = response.error_for_status()?;
        if response
            .content_length()
            .is_some_and(|length| length > MAX_CATALOG_BYTES as u64)
        {
            return Err(MarketplaceError::Network(
                "catalog index exceeds the 8 MiB limit".to_string(),
            ));
        }
        let etag = response
            .headers()
            .get(ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let mut stream = response.bytes_stream();
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            if bytes.len().saturating_add(chunk.len()) > MAX_CATALOG_BYTES {
                return Err(MarketplaceError::Network(
                    "catalog index exceeds the 8 MiB limit".to_string(),
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        let index = serde_json::from_slice::<CatalogIndex>(&bytes)?;
        index.validate()?;
        Ok(FetchCatalog::Updated { index, etag })
    }

    async fn verify_plugin_source(&self, plugin: &CatalogPlugin) -> SourceVerification {
        let mut messages = Vec::new();
        let repository = match self.github_repository(&plugin.manifest.repository).await {
            Ok(repository) => repository,
            Err(error) => {
                return SourceVerification {
                    plugin_id: plugin.manifest.id.clone(),
                    repository_id: plugin.manifest.repository_id,
                    valid: false,
                    messages: vec![error.to_string()],
                };
            }
        };
        if repository.id != plugin.manifest.repository_id {
            messages.push(format!(
                "repository_id mismatch: catalog {}, GitHub {}",
                plugin.manifest.repository_id, repository.id
            ));
        }
        if !repository
            .full_name
            .eq_ignore_ascii_case(&plugin.manifest.repository)
        {
            messages.push(format!(
                "repository name mismatch: catalog {}, GitHub {}",
                plugin.manifest.repository, repository.full_name
            ));
        }
        if repository.private {
            messages.push("repository is private".to_string());
        }
        if repository.archived {
            messages.push("repository is archived".to_string());
        }
        if repository
            .license
            .as_ref()
            .and_then(|license| license.spdx_id.as_deref())
            .is_none_or(|license| license == "NOASSERTION")
        {
            messages.push("GitHub could not detect an open-source license".to_string());
        }
        for version in &plugin.versions {
            if plugin.manifest.kind == crate::PluginKind::Static || version.yanked {
                continue;
            }
            match self
                .github_release(&plugin.manifest.repository, &version.release_tag)
                .await
            {
                Ok(release) => {
                    if release.draft {
                        messages.push(format!("release {} is a draft", version.release_tag));
                    }
                    if release.prerelease != matches!(version.channel, ReleaseChannel::Prerelease) {
                        messages.push(format!(
                            "release {} prerelease state does not match channel {:?}",
                            version.release_tag, version.channel
                        ));
                    }
                    for asset in &version.assets {
                        match release
                            .assets
                            .iter()
                            .find(|candidate| candidate.name == asset.asset_name)
                        {
                            Some(found) if found.size != asset.size_bytes => {
                                messages.push(format!(
                                    "asset {} size mismatch: catalog {}, GitHub {}",
                                    asset.asset_name, asset.size_bytes, found.size
                                ))
                            }
                            Some(_) if asset.github_attestation => {
                                match self
                                    .github_attestations(
                                        &plugin.manifest.repository,
                                        &asset.sha256,
                                    )
                                    .await
                                {
                                    Ok(attestations) if !attestations.attestations.is_empty() => {}
                                    Ok(_) => messages.push(format!(
                                        "asset {} declares github_attestation but none was found for SHA256 {}",
                                        asset.asset_name, asset.sha256
                                    )),
                                    Err(error) => messages.push(format!(
                                        "asset {} attestation could not be verified: {error}",
                                        asset.asset_name
                                    )),
                                }
                            }
                            Some(_) => {}
                            None => messages.push(format!(
                                "release {} is missing asset {}",
                                version.release_tag, asset.asset_name
                            )),
                        }
                    }
                }
                Err(error) => messages.push(format!(
                    "release {} could not be verified: {error}",
                    version.release_tag
                )),
            }
        }
        SourceVerification {
            plugin_id: plugin.manifest.id.clone(),
            repository_id: plugin.manifest.repository_id,
            valid: messages.is_empty(),
            messages,
        }
    }

    async fn github_repository(&self, repository: &str) -> Result<GithubRepository> {
        let url = self.github_url(&format!("repos/{repository}"))?;
        self.authorized(self.client.get(url))
            .send()
            .await?
            .error_for_status()?
            .json::<GithubRepository>()
            .await
            .map_err(Into::into)
    }

    async fn github_release(&self, repository: &str, tag: &str) -> Result<GithubRelease> {
        let url = self.github_url(&format!("repos/{repository}/releases/tags/{tag}"))?;
        self.authorized(self.client.get(url))
            .send()
            .await?
            .error_for_status()?
            .json::<GithubRelease>()
            .await
            .map_err(Into::into)
    }

    async fn github_attestations(
        &self,
        repository: &str,
        sha256: &str,
    ) -> Result<GithubAttestations> {
        let url = self.github_url(&format!(
            "repos/{repository}/attestations/sha256:{}",
            sha256.to_ascii_lowercase()
        ))?;
        self.authorized(self.client.get(url))
            .send()
            .await?
            .error_for_status()?
            .json::<GithubAttestations>()
            .await
            .map_err(Into::into)
    }

    fn github_url(&self, path: &str) -> Result<Url> {
        self.github_api.join(path).map_err(|error| {
            MarketplaceError::Network(format!("failed to construct GitHub API URL: {error}"))
        })
    }

    fn authorized(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(authorization) = &self.authorization {
            request.header(AUTHORIZATION, authorization.clone())
        } else {
            request
        }
    }
}

#[derive(Debug, Deserialize)]
struct GithubRepository {
    id: u64,
    full_name: String,
    private: bool,
    archived: bool,
    license: Option<GithubLicense>,
}

#[derive(Debug, Deserialize)]
struct GithubLicense {
    spdx_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    draft: bool,
    prerelease: bool,
    assets: Vec<GithubReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubReleaseAsset {
    id: u64,
    name: String,
    size: u64,
}

#[derive(Debug, Deserialize)]
struct GithubAttestations {
    attestations: Vec<serde_json::Value>,
}

enum FetchCatalog {
    NotModified,
    Updated {
        index: CatalogIndex,
        etag: Option<String>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct CatalogCacheMeta {
    fetched_at: String,
    etag: Option<String>,
}

async fn read_cached_catalog(cache_path: &Path, meta_path: &Path) -> Result<Option<CatalogLoad>> {
    let raw = match tokio::fs::read(cache_path).await {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let index = serde_json::from_slice::<CatalogIndex>(&raw)?;
    index.validate()?;
    let meta = match tokio::fs::read(meta_path).await {
        Ok(raw) => serde_json::from_slice::<CatalogCacheMeta>(&raw).ok(),
        Err(_) => None,
    };
    Ok(Some(CatalogLoad {
        index,
        source: CatalogSource::Cache,
        fetched_at: meta.as_ref().map(|meta| meta.fetched_at.clone()),
        warning: None,
        etag: meta.and_then(|meta| meta.etag),
    }))
}

async fn write_catalog_cache(
    cache_path: &Path,
    meta_path: &Path,
    index: &CatalogIndex,
    etag: Option<String>,
    fetched_at: &str,
) -> Result<()> {
    if let Some(parent) = cache_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let index_temporary = cache_path.with_extension(format!("json.{}.tmp", std::process::id()));
    let meta_temporary = meta_path.with_extension(format!("json.{}.tmp", std::process::id()));
    tokio::fs::write(&index_temporary, serde_json::to_vec_pretty(index)?).await?;
    tokio::fs::write(
        &meta_temporary,
        serde_json::to_vec_pretty(&CatalogCacheMeta {
            fetched_at: fetched_at.to_string(),
            etag,
        })?,
    )
    .await?;
    atomic_replace_async(&index_temporary, cache_path).await?;
    atomic_replace_async(&meta_temporary, meta_path).await?;
    Ok(())
}

async fn atomic_replace_async(source: &Path, destination: &Path) -> Result<()> {
    if let Err(first_error) = tokio::fs::rename(source, destination).await {
        if destination.exists() {
            tokio::fs::remove_file(destination).await?;
            tokio::fs::rename(source, destination).await?;
        } else {
            return Err(first_error.into());
        }
    }
    Ok(())
}

fn validate_catalog_url(value: &str) -> Result<Url> {
    let url = Url::parse(value).map_err(|error| {
        MarketplaceError::InvalidMetadata(format!("invalid catalog URL '{value}': {error}"))
    })?;
    let loopback_http = url.scheme() == "http"
        && url
            .host_str()
            .is_some_and(|host| matches!(host, "127.0.0.1" | "localhost" | "[::1]" | "::1"));
    if url.username() != ""
        || url.password().is_some()
        || url.host_str().is_none()
        || (url.scheme() != "https" && !loopback_http)
    {
        return Err(MarketplaceError::InvalidMetadata(
            "catalog URL must use HTTPS; HTTP is allowed only for loopback development".to_string(),
        ));
    }
    Ok(url)
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
    fn catalog_url_rejects_remote_plain_http_and_credentials() {
        assert!(validate_catalog_url(OFFICIAL_CATALOG_URL).is_ok());
        assert!(validate_catalog_url("http://example.com/index.json").is_err());
        assert!(validate_catalog_url("https://user@example.com/index.json").is_err());
        assert!(validate_catalog_url("http://127.0.0.1:8080/index.json").is_ok());
        assert!(validate_catalog_url("https://example.com/index.json").is_ok());
    }
}
