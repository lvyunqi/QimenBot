use crate::AdminState;
use crate::error::AdminError;
use crate::types::{ApiEnvelope, MutationResult};
use axum::Json;
use axum::extract::{Path, Query, State};
use qimen_host_types::load_plugin_state;
use qimen_plugin_marketplace::{
    CatalogIndex, CatalogPlugin, CatalogSource, DriverEventKind, DriverSupport, HostProfile,
    InstalledPlugin, InstalledVersion, MarketplaceClient, MarketplaceError, MarketplaceLock,
    MarketplacePaths, MessageScene, OutboundCapability, PluginDriver, PluginKind, ReleaseChannel,
    TrustLevel, VersionCompatibility, compatible_versions, evaluate_version,
    select_latest_compatible, sha256_file,
};
use qimen_runtime::dynamic_runtime::scan_dynamic_plugins;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path as FsPath;
use std::time::Duration;

const DEFAULT_MARKETPLACE_PAGE_SIZE: usize = 20;
const MAX_MARKETPLACE_PAGE_SIZE: usize = 100;

#[derive(Debug, Serialize)]
pub struct MarketplaceView {
    enabled: bool,
    allow_prerelease: bool,
    auto_update: bool,
    source: Option<CatalogSource>,
    fetched_at: Option<String>,
    warning: Option<String>,
    host: HostProfile,
    counts: MarketplaceCountsView,
    pagination: MarketplacePaginationView,
    plugins: Vec<MarketplacePluginSummaryView>,
}

#[derive(Debug, Serialize, Clone)]
pub struct MarketplacePluginView {
    id: String,
    name: String,
    summary: String,
    description: String,
    kind: PluginKind,
    repository: String,
    repository_url: String,
    repository_id: u64,
    license: String,
    authors: Vec<String>,
    categories: Vec<String>,
    keywords: Vec<String>,
    trust: TrustLevel,
    catalog_listed: bool,
    latest_compatible: Option<String>,
    versions: Vec<MarketplaceVersionView>,
    installed: Option<MarketplaceInstalledView>,
    unmanaged: Option<UnmanagedPluginView>,
}

#[derive(Debug, Serialize, Clone)]
struct MarketplaceVersionView {
    version: String,
    released_at: String,
    channel: ReleaseChannel,
    qimenbot: String,
    dynamic_api: Option<String>,
    yanked: bool,
    data_schema_version: u32,
    rollback_safe: bool,
    changelog: String,
    drivers: Vec<DriverSupport>,
    compatible: bool,
    installable: bool,
    asset_name: Option<String>,
    asset_target: Option<String>,
    asset_size_bytes: Option<u64>,
    asset_sha256: Option<String>,
    min_glibc: Option<String>,
    github_attestation: bool,
    issues: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
struct MarketplaceInstalledView {
    version: String,
    active_file: String,
    target: String,
    sha256: String,
    installed_at: String,
    pinned: bool,
    active: bool,
    loaded: bool,
    update_available: bool,
    can_rollback: bool,
    data_schema_version: u32,
}

#[derive(Debug, Serialize, Clone)]
struct UnmanagedPluginView {
    version: String,
    file_name: String,
    sha256: Option<String>,
    can_adopt: bool,
    reason: String,
}

#[derive(Debug, Serialize)]
struct MarketplacePluginSummaryView {
    id: String,
    name: String,
    summary: String,
    kind: PluginKind,
    license: String,
    trust: TrustLevel,
    catalog_listed: bool,
    latest_compatible: Option<String>,
    drivers: Vec<DriverSupport>,
    installed: Option<MarketplaceInstalledView>,
    unmanaged: Option<UnmanagedPluginView>,
}

#[derive(Debug, Default, Serialize, PartialEq, Eq)]
struct MarketplaceCountsView {
    all: usize,
    dynamic: usize,
    #[serde(rename = "static")]
    static_plugins: usize,
    installed: usize,
    updates: usize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct MarketplacePaginationView {
    page: usize,
    page_size: usize,
    total_items: usize,
    total_pages: usize,
}

#[derive(Debug)]
struct MarketplaceSnapshot {
    enabled: bool,
    allow_prerelease: bool,
    auto_update: bool,
    source: Option<CatalogSource>,
    fetched_at: Option<String>,
    warning: Option<String>,
    host: HostProfile,
    plugins: Vec<MarketplacePluginView>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum MarketplaceFilter {
    #[default]
    All,
    Dynamic,
    Static,
    Installed,
    Updates,
}

#[derive(Debug, Deserialize)]
/// 商城列表的服务端搜索、筛选和分页参数。
pub struct MarketplaceQuery {
    #[serde(default = "default_marketplace_page")]
    page: usize,
    #[serde(default = "default_marketplace_page_size")]
    page_size: usize,
    #[serde(default)]
    query: String,
    #[serde(default)]
    filter: MarketplaceFilter,
}

#[derive(Debug, Deserialize)]
pub struct InstallRequest {
    #[serde(default)]
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AdoptRequest {
    #[serde(default)]
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PinRequest {
    pinned: bool,
}

/// 从最近一次成功保存的目录中读取商城列表。
pub async fn catalog(
    State(state): State<AdminState>,
    Query(query): Query<MarketplaceQuery>,
) -> Result<Json<ApiEnvelope<MarketplaceView>>, AdminError> {
    let _operation = state.marketplace_operations.lock().await;
    let snapshot = build_snapshot(&state, false).await?;
    Ok(Json(ApiEnvelope::new(paginate_snapshot(snapshot, &query))))
}

/// 同步官方目录后返回当前筛选条件下的商城列表。
pub async fn refresh(
    State(state): State<AdminState>,
    Query(query): Query<MarketplaceQuery>,
) -> Result<Json<ApiEnvelope<MarketplaceView>>, AdminError> {
    let _operation = state.marketplace_operations.lock().await;
    let snapshot = build_snapshot(&state, true).await?;
    Ok(Json(ApiEnvelope::new(paginate_snapshot(snapshot, &query))))
}

/// 按插件 ID 读取完整元数据和版本记录。
pub async fn detail(
    State(state): State<AdminState>,
    Path(plugin_id): Path<String>,
) -> Result<Json<ApiEnvelope<MarketplacePluginView>>, AdminError> {
    let _operation = state.marketplace_operations.lock().await;
    let snapshot = build_snapshot(&state, false).await?;
    if !snapshot.enabled {
        return Err(AdminError::BadRequest("插件商城已在配置中关闭".to_string()));
    }
    let plugin = snapshot
        .plugins
        .into_iter()
        .find(|plugin| plugin.id == plugin_id)
        .ok_or_else(|| AdminError::NotFound(format!("商城中没有插件 '{plugin_id}'")))?;
    Ok(Json(ApiEnvelope::new(plugin)))
}

pub async fn install(
    State(state): State<AdminState>,
    Path(plugin_id): Path<String>,
    Json(request): Json<InstallRequest>,
) -> Result<Json<ApiEnvelope<MutationResult>>, AdminError> {
    let _operation = state.marketplace_operations.lock().await;
    let context = operation_context(&state).await?;
    let plugin = context
        .catalog
        .plugin(&plugin_id)
        .ok_or_else(|| AdminError::NotFound(format!("商城中没有插件 '{plugin_id}'")))?;
    if plugin.manifest.kind != PluginKind::Dynamic {
        return Err(AdminError::BadRequest(
            "静态插件需要加入源码并重新构建 qimenbotd，不能在线安装".to_string(),
        ));
    }
    let (version, compatibility) = select_requested_version(
        plugin,
        request.version.as_deref(),
        &context.host,
        context.allow_prerelease,
    )?;
    let asset = compatibility
        .asset
        .as_ref()
        .ok_or_else(|| AdminError::BadRequest("所选版本没有当前平台可用的资产".to_string()))?;

    let existing = context.lock.plugins.get(&plugin_id).cloned();
    let was_update = existing.is_some();
    if let Some(existing) = existing.as_ref()
        && existing.current.repository_id != plugin.manifest.repository_id
    {
        return Err(AdminError::Conflict(format!(
            "插件 '{plugin_id}' 的仓库数字 ID 与本地安装锁不一致，已停止更新"
        )));
    }
    let scanned = scan_dynamic_plugins(&context.plugin_bin_dir)?;
    let same_id = scanned
        .iter()
        .filter(|entry| entry.plugin_id == plugin_id)
        .collect::<Vec<_>>();
    if !was_update && !same_id.is_empty() {
        return Err(AdminError::Conflict(format!(
            "检测到手工放入的插件 '{plugin_id}'，请先在商城中执行“关联本地插件”"
        )));
    }
    if same_id.len() > 1 {
        return Err(AdminError::Conflict(format!(
            "插件目录中存在 {} 个 ID 为 '{plugin_id}' 的动态库，请先移走重复文件",
            same_id.len()
        )));
    }

    let prepared = context
        .paths
        .prepare_install(&context.client, plugin, version, asset, existing.as_ref())
        .await
        .map_err(marketplace_error)?;
    validate_staged_descriptor(&prepared, plugin, version)?;
    apply_transaction(&state, &prepared.transaction).await?;
    if let Err(error) = validate_active_plugin(
        &state,
        &context.plugin_bin_dir,
        &context.plugin_state_path,
        &plugin_id,
        &version.version,
        &prepared.installed.active_file,
    ) {
        restore_transaction(&state, &prepared.transaction).await?;
        return Err(error);
    }
    let plugin_enabled = load_plugin_state(&context.plugin_state_path)?.is_enabled(&plugin_id);

    let mut next_lock = context.lock;
    next_lock
        .plugins
        .insert(plugin_id.clone(), prepared.installed.clone());
    if let Err(error) = next_lock.save(&context.paths.lock_path) {
        restore_transaction(&state, &prepared.transaction).await?;
        return Err(marketplace_error(error));
    }
    if let Err(error) = prepared.transaction.finalize() {
        tracing::warn!(plugin_id = %plugin_id, error = %error, "failed to remove marketplace transaction files");
    }
    state.audit.record(
        if was_update {
            "marketplace.update"
        } else {
            "marketplace.install"
        },
        format!("plugin:{plugin_id}"),
        "success",
        format!(
            "installed version {} for {} with SHA256 {}",
            version.version, asset.target, asset.sha256
        ),
    )?;
    Ok(Json(ApiEnvelope::new(MutationResult {
        revision: None,
        restart_required: false,
        message: if was_update {
            if plugin_enabled {
                format!("插件 {plugin_id} 已更新到 {} 并重新加载", version.version)
            } else {
                format!(
                    "插件 {plugin_id} 已更新到 {}，当前保持停用",
                    version.version
                )
            }
        } else if plugin_enabled {
            format!("插件 {plugin_id} {} 已安装并加载", version.version)
        } else {
            format!("插件 {plugin_id} {} 已安装，当前保持停用", version.version)
        },
    })))
}

pub async fn adopt(
    State(state): State<AdminState>,
    Path(plugin_id): Path<String>,
    Json(request): Json<AdoptRequest>,
) -> Result<Json<ApiEnvelope<MutationResult>>, AdminError> {
    let _operation = state.marketplace_operations.lock().await;
    let mut context = operation_context(&state).await?;
    if context.lock.plugins.contains_key(&plugin_id) {
        return Err(AdminError::Conflict(format!(
            "插件 '{plugin_id}' 已由商城管理"
        )));
    }
    let plugin = context
        .catalog
        .plugin(&plugin_id)
        .ok_or_else(|| AdminError::NotFound(format!("商城中没有插件 '{plugin_id}'")))?;
    if plugin.manifest.kind != PluginKind::Dynamic {
        return Err(AdminError::BadRequest(
            "只有动态插件可以关联本地二进制".to_string(),
        ));
    }
    let scanned = scan_dynamic_plugins(&context.plugin_bin_dir)?;
    let matches = scanned
        .iter()
        .filter(|entry| entry.plugin_id == plugin_id)
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(AdminError::Conflict(format!(
            "关联要求插件目录中恰好有一个 ID 为 '{plugin_id}' 的动态库，当前发现 {} 个",
            matches.len()
        )));
    }
    let descriptor = matches[0];
    let requested_version = request
        .version
        .as_deref()
        .unwrap_or(&descriptor.plugin_version);
    if requested_version != descriptor.plugin_version {
        return Err(AdminError::Conflict(format!(
            "本地描述符版本为 {}，不能关联为 {}",
            descriptor.plugin_version, requested_version
        )));
    }
    let version = plugin.version(requested_version).ok_or_else(|| {
        AdminError::NotFound(format!(
            "商城没有登记插件 '{plugin_id}' 的版本 {requested_version}"
        ))
    })?;
    let compatibility = evaluate_version(plugin, version, &context.host, context.allow_prerelease);
    if !compatibility.installable {
        return Err(incompatible_error(&compatibility));
    }
    let asset = compatibility
        .asset
        .ok_or_else(|| AdminError::BadRequest("该版本没有当前平台的审核资产".to_string()))?;
    let path = FsPath::new(&descriptor.path);
    let metadata = std::fs::metadata(path)?;
    let actual_sha = sha256_file(path).map_err(marketplace_error)?;
    if metadata.len() != asset.size_bytes || !actual_sha.eq_ignore_ascii_case(&asset.sha256) {
        return Err(AdminError::Conflict(format!(
            "本地文件与商城审核资产不一致：期望 {} 字节 / {}，实际 {} 字节 / {}",
            asset.size_bytes,
            asset.sha256,
            metadata.len(),
            actual_sha
        )));
    }
    let active_file = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| AdminError::BadRequest("本地插件文件名不是有效 UTF-8".to_string()))?
        .to_string();
    let current = InstalledVersion {
        version: version.version.clone(),
        repository_id: plugin.manifest.repository_id,
        target: asset.target,
        sha256: actual_sha,
        channel: version.channel,
        data_schema_version: version.data_schema_version,
        rollback_safe: version.rollback_safe,
        installed_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    };
    context
        .paths
        .archive_existing(&plugin_id, &current, path)
        .map_err(marketplace_error)?;
    context.lock.plugins.insert(
        plugin_id.clone(),
        InstalledPlugin {
            active_file,
            pinned: true,
            current,
            previous: None,
        },
    );
    context
        .lock
        .save(&context.paths.lock_path)
        .map_err(marketplace_error)?;
    state.audit.record(
        "marketplace.adopt",
        format!("plugin:{plugin_id}"),
        "success",
        "associated a checksum-matching unmanaged plugin and pinned its version",
    )?;
    Ok(Json(ApiEnvelope::new(MutationResult {
        revision: None,
        restart_required: false,
        message: format!("插件 {plugin_id} 已关联并固定在 {requested_version}"),
    })))
}

pub async fn pin(
    State(state): State<AdminState>,
    Path(plugin_id): Path<String>,
    Json(request): Json<PinRequest>,
) -> Result<Json<ApiEnvelope<MutationResult>>, AdminError> {
    let _operation = state.marketplace_operations.lock().await;
    let stored = state.config_store.read().await?;
    ensure_enabled(&stored.config.marketplace)?;
    let mut lock = MarketplaceLock::load(FsPath::new(&stored.config.marketplace.lock_path))
        .map_err(marketplace_error)?;
    let installed = lock
        .plugins
        .get_mut(&plugin_id)
        .ok_or_else(|| AdminError::NotFound(format!("插件 '{plugin_id}' 不是商城管理的安装")))?;
    installed.pinned = request.pinned;
    lock.save(FsPath::new(&stored.config.marketplace.lock_path))
        .map_err(marketplace_error)?;
    state.audit.record(
        if request.pinned {
            "marketplace.pin"
        } else {
            "marketplace.unpin"
        },
        format!("plugin:{plugin_id}"),
        "success",
        "marketplace update pin changed",
    )?;
    Ok(Json(ApiEnvelope::new(MutationResult {
        revision: None,
        restart_required: false,
        message: if request.pinned {
            format!("插件 {plugin_id} 已固定当前版本")
        } else {
            format!("插件 {plugin_id} 已允许显示新版本更新")
        },
    })))
}

pub async fn rollback(
    State(state): State<AdminState>,
    Path(plugin_id): Path<String>,
) -> Result<Json<ApiEnvelope<MutationResult>>, AdminError> {
    let _operation = state.marketplace_operations.lock().await;
    let mut context = local_operation_context(&state).await?;
    let installed = context
        .lock
        .plugins
        .get(&plugin_id)
        .cloned()
        .ok_or_else(|| AdminError::NotFound(format!("插件 '{plugin_id}' 不是商城管理的安装")))?;
    let previous_version = installed
        .previous
        .as_ref()
        .map(|version| version.version.clone())
        .ok_or_else(|| AdminError::Conflict("没有可回滚的历史版本".to_string()))?;
    let (rolled_back, transaction) = context
        .paths
        .prepare_rollback(&plugin_id, &installed)
        .map_err(marketplace_error)?;
    apply_transaction(&state, &transaction).await?;
    if let Err(error) = validate_active_plugin(
        &state,
        &context.plugin_bin_dir,
        &context.plugin_state_path,
        &plugin_id,
        &previous_version,
        &rolled_back.active_file,
    ) {
        restore_transaction(&state, &transaction).await?;
        return Err(error);
    }
    context.lock.plugins.insert(plugin_id.clone(), rolled_back);
    if let Err(error) = context.lock.save(&context.paths.lock_path) {
        restore_transaction(&state, &transaction).await?;
        return Err(marketplace_error(error));
    }
    if let Err(error) = transaction.finalize() {
        tracing::warn!(plugin_id = %plugin_id, error = %error, "failed to remove rollback transaction files");
    }
    state.audit.record(
        "marketplace.rollback",
        format!("plugin:{plugin_id}"),
        "success",
        format!("restored version {previous_version}"),
    )?;
    Ok(Json(ApiEnvelope::new(MutationResult {
        revision: None,
        restart_required: false,
        message: format!("插件 {plugin_id} 已回滚到 {previous_version}"),
    })))
}

pub async fn uninstall(
    State(state): State<AdminState>,
    Path(plugin_id): Path<String>,
) -> Result<Json<ApiEnvelope<MutationResult>>, AdminError> {
    let _operation = state.marketplace_operations.lock().await;
    let mut context = local_operation_context(&state).await?;
    let installed = context
        .lock
        .plugins
        .get(&plugin_id)
        .cloned()
        .ok_or_else(|| AdminError::NotFound(format!("插件 '{plugin_id}' 不是商城管理的安装")))?;
    let transaction = context
        .paths
        .prepare_uninstall(&plugin_id, &installed)
        .map_err(marketplace_error)?;
    apply_transaction(&state, &transaction).await?;
    context.lock.plugins.remove(&plugin_id);
    if let Err(error) = context.lock.save(&context.paths.lock_path) {
        restore_transaction(&state, &transaction).await?;
        return Err(marketplace_error(error));
    }
    if let Err(error) = transaction.finalize() {
        tracing::warn!(plugin_id = %plugin_id, error = %error, "failed to remove uninstall transaction files");
    }
    state.audit.record(
        "marketplace.uninstall",
        format!("plugin:{plugin_id}"),
        "success",
        "removed active binary; plugin configuration and data were preserved",
    )?;
    Ok(Json(ApiEnvelope::new(MutationResult {
        revision: None,
        restart_required: false,
        message: format!("插件 {plugin_id} 已卸载，配置和数据仍然保留"),
    })))
}

struct OperationContext {
    client: MarketplaceClient,
    catalog: CatalogIndex,
    host: HostProfile,
    paths: MarketplacePaths,
    lock: MarketplaceLock,
    plugin_bin_dir: String,
    plugin_state_path: String,
    allow_prerelease: bool,
}

struct LocalOperationContext {
    paths: MarketplacePaths,
    lock: MarketplaceLock,
    plugin_bin_dir: String,
    plugin_state_path: String,
}

async fn operation_context(state: &AdminState) -> Result<OperationContext, AdminError> {
    let stored = state.config_store.read().await?;
    ensure_enabled(&stored.config.marketplace)?;
    let plugin_bin_dir = state
        .runtime
        .active_plugin_bin_dir()
        .ok_or_else(|| AdminError::Conflict("当前 Runtime 没有配置动态插件目录".to_string()))?
        .to_string();
    let plugin_state_path = state
        .runtime
        .active_plugin_state_path()
        .ok_or_else(|| AdminError::Conflict("当前 Runtime 没有配置插件状态文件".to_string()))?
        .to_string();
    let client = MarketplaceClient::new(Duration::from_secs(
        stored.config.marketplace.request_timeout_secs,
    ))
    .map_err(marketplace_error)?;
    let paths = MarketplacePaths::new(
        &stored.config.marketplace.cache_dir,
        &stored.config.marketplace.lock_path,
        &plugin_bin_dir,
    );
    let loaded = client
        .load_catalog(&paths.catalog_cache_dir(), true)
        .await
        .map_err(marketplace_error)?;
    if loaded.source != CatalogSource::Network {
        return Err(AdminError::Unavailable(
            loaded
                .warning
                .unwrap_or_else(|| "无法确认插件目录是否为最新版本".to_string()),
        ));
    }
    let catalog = loaded.index;
    let lock = MarketplaceLock::load(&paths.lock_path).map_err(marketplace_error)?;
    Ok(OperationContext {
        client,
        catalog,
        host: HostProfile::current(env!("CARGO_PKG_VERSION")),
        paths,
        lock,
        plugin_bin_dir,
        plugin_state_path,
        allow_prerelease: stored.config.marketplace.allow_prerelease,
    })
}

async fn local_operation_context(state: &AdminState) -> Result<LocalOperationContext, AdminError> {
    let stored = state.config_store.read().await?;
    ensure_enabled(&stored.config.marketplace)?;
    let plugin_bin_dir = state
        .runtime
        .active_plugin_bin_dir()
        .ok_or_else(|| AdminError::Conflict("当前 Runtime 没有配置动态插件目录".to_string()))?
        .to_string();
    let plugin_state_path = state
        .runtime
        .active_plugin_state_path()
        .ok_or_else(|| AdminError::Conflict("当前 Runtime 没有配置插件状态文件".to_string()))?
        .to_string();
    let paths = MarketplacePaths::new(
        &stored.config.marketplace.cache_dir,
        &stored.config.marketplace.lock_path,
        &plugin_bin_dir,
    );
    let lock = MarketplaceLock::load(&paths.lock_path).map_err(marketplace_error)?;
    Ok(LocalOperationContext {
        paths,
        lock,
        plugin_bin_dir,
        plugin_state_path,
    })
}

async fn build_snapshot(
    state: &AdminState,
    refresh: bool,
) -> Result<MarketplaceSnapshot, AdminError> {
    let stored = state.config_store.read().await?;
    let config = &stored.config.marketplace;
    let host = HostProfile::current(env!("CARGO_PKG_VERSION"));
    if !config.enabled {
        return Ok(MarketplaceSnapshot {
            enabled: false,
            allow_prerelease: config.allow_prerelease,
            auto_update: config.auto_update,
            source: None,
            fetched_at: None,
            warning: Some("插件商城已在配置中关闭。".to_string()),
            host,
            plugins: Vec::new(),
        });
    }
    let client = MarketplaceClient::new(Duration::from_secs(config.request_timeout_secs))
        .map_err(marketplace_error)?;
    let paths = MarketplacePaths::new(
        &config.cache_dir,
        &config.lock_path,
        state
            .runtime
            .active_plugin_bin_dir()
            .unwrap_or(&stored.config.official_host.plugin_bin_dir),
    );
    let (catalog, source, fetched_at, mut warning) = match client
        .load_catalog(&paths.catalog_cache_dir(), refresh)
        .await
    {
        Ok(loaded) => (
            loaded.index,
            Some(loaded.source),
            loaded.fetched_at,
            loaded.warning,
        ),
        Err(error) => (
            CatalogIndex::empty(),
            None,
            None,
            Some(format!("无法读取插件目录：{error}")),
        ),
    };
    let lock = match MarketplaceLock::load(&paths.lock_path) {
        Ok(lock) => lock,
        Err(error) => {
            warning = Some(match warning {
                Some(warning) => format!("{warning}；安装锁读取失败：{error}"),
                None => format!("安装锁读取失败：{error}"),
            });
            MarketplaceLock::default()
        }
    };
    let scanned = scan_dynamic_plugins(
        state
            .runtime
            .active_plugin_bin_dir()
            .unwrap_or(&stored.config.official_host.plugin_bin_dir),
    )?;
    let loaded_ids = state
        .runtime
        .host_plugin_report()
        .map(|report| {
            report
                .dynamic_plugins
                .into_iter()
                .map(|entry| entry.plugin_id)
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    let mut scanned_by_id = HashMap::<String, Vec<_>>::new();
    for entry in &scanned {
        scanned_by_id
            .entry(entry.plugin_id.clone())
            .or_default()
            .push(entry);
    }
    let mut plugins = catalog
        .plugins
        .iter()
        .map(|plugin| {
            plugin_view(
                plugin,
                lock.plugins.get(&plugin.manifest.id),
                scanned_by_id
                    .get(&plugin.manifest.id)
                    .map(Vec::as_slice)
                    .unwrap_or_default(),
                &loaded_ids,
                &host,
                config.allow_prerelease,
                &paths,
            )
        })
        .collect::<Vec<_>>();
    let listed = catalog
        .plugins
        .iter()
        .map(|plugin| plugin.manifest.id.as_str())
        .collect::<HashSet<_>>();
    for (plugin_id, installed) in &lock.plugins {
        if listed.contains(plugin_id.as_str()) {
            continue;
        }
        let active = paths
            .active_path(installed)
            .is_ok_and(|path| path.is_file());
        plugins.push(MarketplacePluginView {
            id: plugin_id.clone(),
            name: plugin_id.clone(),
            summary: "该插件仍在本地安装锁中，但当前目录已不再收录。".to_string(),
            description: String::new(),
            kind: PluginKind::Dynamic,
            repository: String::new(),
            repository_url: String::new(),
            repository_id: installed.current.repository_id,
            license: "未知".to_string(),
            authors: Vec::new(),
            categories: Vec::new(),
            keywords: Vec::new(),
            trust: TrustLevel::Community,
            catalog_listed: false,
            latest_compatible: None,
            versions: Vec::new(),
            installed: Some(installed_view(
                installed,
                active,
                loaded_ids.contains(plugin_id),
                false,
            )),
            unmanaged: None,
        });
    }
    plugins.sort_by(|left, right| {
        marketplace_priority(left)
            .cmp(&marketplace_priority(right))
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(MarketplaceSnapshot {
        enabled: true,
        allow_prerelease: config.allow_prerelease,
        auto_update: config.auto_update,
        source,
        fetched_at,
        warning,
        host,
        plugins,
    })
}

fn paginate_snapshot(snapshot: MarketplaceSnapshot, query: &MarketplaceQuery) -> MarketplaceView {
    // 徽标展示整份目录的分类规模，搜索后的实际数量由 pagination 返回。
    let counts = marketplace_counts(&snapshot.plugins);
    let needle = query.query.trim().to_lowercase();
    let filtered = snapshot
        .plugins
        .into_iter()
        .filter(|plugin| marketplace_filter_matches(plugin, query.filter))
        .filter(|plugin| needle.is_empty() || marketplace_query_matches(plugin, &needle))
        .collect::<Vec<_>>();
    let page_size = query.page_size.clamp(1, MAX_MARKETPLACE_PAGE_SIZE);
    let total_items = filtered.len();
    let total_pages = total_items.div_ceil(page_size).max(1);
    let page = query.page.max(1).min(total_pages);
    let offset = (page - 1).saturating_mul(page_size).min(total_items);
    let plugins = filtered
        .into_iter()
        .skip(offset)
        .take(page_size)
        .map(plugin_summary_view)
        .collect();

    MarketplaceView {
        enabled: snapshot.enabled,
        allow_prerelease: snapshot.allow_prerelease,
        auto_update: snapshot.auto_update,
        source: snapshot.source,
        fetched_at: snapshot.fetched_at,
        warning: snapshot.warning,
        host: snapshot.host,
        counts,
        pagination: MarketplacePaginationView {
            page,
            page_size,
            total_items,
            total_pages,
        },
        plugins,
    }
}

fn marketplace_counts(plugins: &[MarketplacePluginView]) -> MarketplaceCountsView {
    MarketplaceCountsView {
        all: plugins.len(),
        dynamic: plugins
            .iter()
            .filter(|plugin| plugin.kind == PluginKind::Dynamic)
            .count(),
        static_plugins: plugins
            .iter()
            .filter(|plugin| plugin.kind == PluginKind::Static)
            .count(),
        installed: plugins
            .iter()
            .filter(|plugin| plugin.installed.is_some())
            .count(),
        updates: plugins
            .iter()
            .filter(|plugin| {
                plugin
                    .installed
                    .as_ref()
                    .is_some_and(|installed| installed.update_available)
            })
            .count(),
    }
}

fn marketplace_filter_matches(plugin: &MarketplacePluginView, filter: MarketplaceFilter) -> bool {
    match filter {
        MarketplaceFilter::All => true,
        MarketplaceFilter::Dynamic => plugin.kind == PluginKind::Dynamic,
        MarketplaceFilter::Static => plugin.kind == PluginKind::Static,
        MarketplaceFilter::Installed => plugin.installed.is_some(),
        MarketplaceFilter::Updates => plugin
            .installed
            .as_ref()
            .is_some_and(|installed| installed.update_available),
    }
}

fn marketplace_query_matches(plugin: &MarketplacePluginView, needle: &str) -> bool {
    let mut terms = vec![
        plugin.id.as_str(),
        plugin.name.as_str(),
        plugin.summary.as_str(),
        plugin.description.as_str(),
        plugin.repository.as_str(),
        plugin.license.as_str(),
    ];
    terms.extend(plugin.authors.iter().map(String::as_str));
    terms.extend(plugin.categories.iter().map(String::as_str));
    terms.extend(plugin.keywords.iter().map(String::as_str));
    if terms
        .into_iter()
        .any(|term| term.to_lowercase().contains(needle))
    {
        return true;
    }

    plugin.versions.iter().any(|version| {
        version.drivers.iter().any(|support| {
            driver_search_terms(support)
                .into_iter()
                .any(|term| term.contains(needle))
        })
    })
}

fn driver_search_terms(support: &DriverSupport) -> Vec<&'static str> {
    let mut terms = match support.driver {
        PluginDriver::OneBot11 => vec!["onebot11", "onebot 11", "普通消息驱动"],
        PluginDriver::QqOfficial => vec!["qq-official", "官方 qq bot", "开放平台驱动"],
    };
    terms.extend(support.scenes.iter().map(|scene| match scene {
        MessageScene::Private => "私聊",
        MessageScene::Group => "群聊",
        MessageScene::GroupAt => "群内 @",
        MessageScene::Channel => "频道消息",
        MessageScene::ChannelAt => "频道 @",
        MessageScene::ChannelPrivate => "频道私信",
    }));
    terms.extend(support.events.iter().map(|event| match event {
        DriverEventKind::Message => "消息",
        DriverEventKind::Notice => "通知",
        DriverEventKind::Request => "请求",
        DriverEventKind::Meta => "元事件",
    }));
    terms.extend(support.outbound.iter().map(|capability| match capability {
        OutboundCapability::Reply => "回复",
        OutboundCapability::Proactive => "主动发送",
        OutboundCapability::RichMessage => "富媒体",
    }));
    terms
}

fn plugin_summary_view(plugin: MarketplacePluginView) -> MarketplacePluginSummaryView {
    // 列表不返回完整版本数组，只保留最能代表当前可用状态的一组驱动能力。
    let drivers = plugin
        .versions
        .iter()
        .find(|version| Some(version.version.as_str()) == plugin.latest_compatible.as_deref())
        .or_else(|| {
            plugin.versions.iter().find(|version| {
                plugin
                    .installed
                    .as_ref()
                    .is_some_and(|installed| installed.version == version.version)
            })
        })
        .or_else(|| plugin.versions.first())
        .map(|version| version.drivers.clone())
        .unwrap_or_default();
    MarketplacePluginSummaryView {
        id: plugin.id,
        name: plugin.name,
        summary: plugin.summary,
        kind: plugin.kind,
        license: plugin.license,
        trust: plugin.trust,
        catalog_listed: plugin.catalog_listed,
        latest_compatible: plugin.latest_compatible,
        drivers,
        installed: plugin.installed,
        unmanaged: plugin.unmanaged,
    }
}

fn default_marketplace_page() -> usize {
    1
}

fn default_marketplace_page_size() -> usize {
    DEFAULT_MARKETPLACE_PAGE_SIZE
}

fn marketplace_priority(plugin: &MarketplacePluginView) -> u8 {
    if plugin
        .installed
        .as_ref()
        .is_some_and(|item| item.update_available)
    {
        0
    } else if plugin.installed.is_some() {
        1
    } else if plugin.unmanaged.as_ref().is_some_and(|item| item.can_adopt) {
        2
    } else if plugin.latest_compatible.is_some() {
        3
    } else if plugin.kind == PluginKind::Static {
        4
    } else {
        5
    }
}

#[allow(clippy::too_many_arguments)]
fn plugin_view(
    plugin: &CatalogPlugin,
    installed: Option<&InstalledPlugin>,
    scanned: &[&qimen_host_types::DynamicPluginReportEntry],
    loaded_ids: &HashSet<String>,
    host: &HostProfile,
    allow_prerelease: bool,
    paths: &MarketplacePaths,
) -> MarketplacePluginView {
    let versions = compatible_versions(plugin, host, allow_prerelease);
    let latest = versions
        .iter()
        .find(|(_, compatibility)| compatibility.installable)
        .map(|(version, _)| version.version.clone());
    let update_available = installed
        .and_then(|installed| {
            let latest = latest.as_deref()?;
            let latest = Version::parse(latest).ok()?;
            let current = Version::parse(&installed.current.version).ok()?;
            Some(!installed.pinned && latest.cmp_precedence(&current).is_gt())
        })
        .unwrap_or(false);
    let active = installed
        .and_then(|installed| paths.active_path(installed).ok())
        .is_some_and(|path| path.is_file());
    let installed_view = installed.map(|installed| {
        installed_view(
            installed,
            active,
            loaded_ids.contains(&plugin.manifest.id),
            update_available,
        )
    });
    let unmanaged = if installed.is_none() && !scanned.is_empty() {
        Some(unmanaged_view(plugin, scanned, host, allow_prerelease))
    } else {
        None
    };
    MarketplacePluginView {
        id: plugin.manifest.id.clone(),
        name: plugin.manifest.name.clone(),
        summary: plugin.manifest.summary.clone(),
        description: plugin.manifest.description.clone(),
        kind: plugin.manifest.kind,
        repository: plugin.manifest.repository.clone(),
        repository_url: plugin.manifest.repository_url(),
        repository_id: plugin.manifest.repository_id,
        license: plugin.manifest.license.clone(),
        authors: plugin.manifest.authors.clone(),
        categories: plugin.manifest.categories.clone(),
        keywords: plugin.manifest.keywords.clone(),
        trust: plugin.manifest.trust,
        catalog_listed: true,
        latest_compatible: latest,
        versions: versions
            .into_iter()
            .map(|(version, compatibility)| version_view(version, compatibility))
            .collect(),
        installed: installed_view,
        unmanaged,
    }
}

fn version_view(
    version: &qimen_plugin_marketplace::VersionManifest,
    compatibility: VersionCompatibility,
) -> MarketplaceVersionView {
    let asset = compatibility.asset.as_ref();
    MarketplaceVersionView {
        version: version.version.clone(),
        released_at: version.released_at.clone(),
        channel: version.channel,
        qimenbot: version.qimenbot.clone(),
        dynamic_api: version.dynamic_api.clone(),
        yanked: version.yanked,
        data_schema_version: version.data_schema_version,
        rollback_safe: version.rollback_safe,
        changelog: version.changelog.clone(),
        drivers: version.drivers.clone(),
        compatible: compatibility.compatible,
        installable: compatibility.installable,
        asset_name: asset.map(|asset| asset.asset_name.clone()),
        asset_target: asset.map(|asset| asset.target.clone()),
        asset_size_bytes: asset.map(|asset| asset.size_bytes),
        asset_sha256: asset.map(|asset| asset.sha256.clone()),
        min_glibc: asset.and_then(|asset| asset.min_glibc.clone()),
        github_attestation: asset.is_some_and(|asset| asset.github_attestation),
        issues: compatibility
            .issues
            .into_iter()
            .map(|issue| issue.message)
            .collect(),
    }
}

fn installed_view(
    installed: &InstalledPlugin,
    active: bool,
    loaded: bool,
    update_available: bool,
) -> MarketplaceInstalledView {
    MarketplaceInstalledView {
        version: installed.current.version.clone(),
        active_file: installed.active_file.clone(),
        target: installed.current.target.clone(),
        sha256: installed.current.sha256.clone(),
        installed_at: installed.current.installed_at.clone(),
        pinned: installed.pinned,
        active,
        loaded,
        update_available,
        can_rollback: installed.can_rollback(),
        data_schema_version: installed.current.data_schema_version,
    }
}

fn unmanaged_view(
    plugin: &CatalogPlugin,
    scanned: &[&qimen_host_types::DynamicPluginReportEntry],
    host: &HostProfile,
    allow_prerelease: bool,
) -> UnmanagedPluginView {
    if scanned.len() != 1 {
        return UnmanagedPluginView {
            version: String::new(),
            file_name: String::new(),
            sha256: None,
            can_adopt: false,
            reason: format!(
                "发现 {} 个相同插件 ID 的文件，不能自动关联。",
                scanned.len()
            ),
        };
    }
    let descriptor = scanned[0];
    let file_name = FsPath::new(&descriptor.path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();
    let actual_sha = sha256_file(FsPath::new(&descriptor.path)).ok();
    let match_result = plugin
        .version(&descriptor.plugin_version)
        .map(|version| evaluate_version(plugin, version, host, allow_prerelease));
    let checksum_matches = match_result
        .as_ref()
        .and_then(|compatibility| compatibility.asset.as_ref())
        .zip(actual_sha.as_ref())
        .is_some_and(|(asset, actual)| asset.sha256.eq_ignore_ascii_case(actual));
    let can_adopt = match_result
        .as_ref()
        .is_some_and(|compatibility| compatibility.installable)
        && checksum_matches;
    let reason = if plugin.version(&descriptor.plugin_version).is_none() {
        "商城没有登记这个本地版本。".to_string()
    } else if !checksum_matches {
        "本地文件 SHA256 与商城审核资产不一致。".to_string()
    } else if !can_adopt {
        "本地版本与当前宿主不兼容。".to_string()
    } else {
        "版本、目标平台和 SHA256 均匹配，可以纳入商城管理。".to_string()
    };
    UnmanagedPluginView {
        version: descriptor.plugin_version.clone(),
        file_name,
        sha256: actual_sha,
        can_adopt,
        reason,
    }
}

fn select_requested_version<'a>(
    plugin: &'a CatalogPlugin,
    requested: Option<&str>,
    host: &HostProfile,
    allow_prerelease: bool,
) -> Result<
    (
        &'a qimen_plugin_marketplace::VersionManifest,
        VersionCompatibility,
    ),
    AdminError,
> {
    let selected = if let Some(requested) = requested {
        let version = plugin.version(requested).ok_or_else(|| {
            AdminError::NotFound(format!(
                "插件 '{}' 没有版本 {}",
                plugin.manifest.id, requested
            ))
        })?;
        (
            version,
            evaluate_version(plugin, version, host, allow_prerelease),
        )
    } else {
        select_latest_compatible(plugin, host, allow_prerelease).ok_or_else(|| {
            AdminError::BadRequest(format!(
                "插件 '{}' 没有适合当前宿主的可安装版本",
                plugin.manifest.id
            ))
        })?
    };
    if !selected.1.installable {
        return Err(incompatible_error(&selected.1));
    }
    Ok(selected)
}

fn validate_staged_descriptor(
    prepared: &qimen_plugin_marketplace::PreparedInstall,
    plugin: &CatalogPlugin,
    version: &qimen_plugin_marketplace::VersionManifest,
) -> Result<(), AdminError> {
    let directory = prepared
        .archive_path
        .parent()
        .ok_or_else(|| AdminError::BadRequest("暂存插件没有父目录".to_string()))?;
    let entries = scan_dynamic_plugins(&directory.to_string_lossy())?;
    if entries.len() != 1 {
        return Err(AdminError::BadRequest(format!(
            "审核资产应导出一个动态插件描述符，实际发现 {} 个",
            entries.len()
        )));
    }
    let descriptor = &entries[0];
    if descriptor.plugin_id != plugin.manifest.id
        || descriptor.plugin_version != version.version
        || version.dynamic_api.as_deref() != Some(descriptor.api_version.as_str())
    {
        return Err(AdminError::Conflict(format!(
            "Release 资产描述符与目录不一致：期望 {} {} API {}，实际 {} {} API {}",
            plugin.manifest.id,
            version.version,
            version.dynamic_api.as_deref().unwrap_or_default(),
            descriptor.plugin_id,
            descriptor.plugin_version,
            descriptor.api_version
        )));
    }
    Ok(())
}

async fn apply_transaction(
    state: &AdminState,
    transaction: &qimen_plugin_marketplace::ActiveFileTransaction,
) -> Result<(), AdminError> {
    state
        .runtime
        .reload_dynamic_plugins_transaction(
            || transaction.apply().map_err(runtime_marketplace_error),
            || transaction.rollback().map_err(runtime_marketplace_error),
        )
        .await?;
    Ok(())
}

async fn restore_transaction(
    state: &AdminState,
    transaction: &qimen_plugin_marketplace::ActiveFileTransaction,
) -> Result<(), AdminError> {
    state
        .runtime
        .reload_dynamic_plugins_transaction(
            || transaction.rollback().map_err(runtime_marketplace_error),
            || transaction.apply().map_err(runtime_marketplace_error),
        )
        .await?;
    Ok(())
}

fn validate_active_plugin(
    state: &AdminState,
    plugin_bin_dir: &str,
    plugin_state_path: &str,
    plugin_id: &str,
    version: &str,
    active_file: &str,
) -> Result<(), AdminError> {
    let matching = scan_dynamic_plugins(plugin_bin_dir)?
        .into_iter()
        .filter(|entry| entry.plugin_id == plugin_id)
        .collect::<Vec<_>>();
    let descriptor_ok = matching.len() == 1
        && matching[0].plugin_version == version
        && FsPath::new(&matching[0].path)
            .file_name()
            .and_then(|value| value.to_str())
            == Some(active_file);
    if !descriptor_ok {
        return Err(AdminError::Conflict(format!(
            "插件 '{plugin_id}' 文件替换后没有以版本 {version} 唯一出现"
        )));
    }
    let enabled = load_plugin_state(plugin_state_path)?.is_enabled(plugin_id);
    let loaded = state.runtime.host_plugin_report().is_some_and(|report| {
        report
            .dynamic_plugins
            .iter()
            .any(|entry| entry.plugin_id == plugin_id && entry.plugin_version == version)
    });
    if enabled && !loaded {
        return Err(AdminError::Conflict(format!(
            "插件 '{plugin_id}' 描述符有效，但初始化失败，已恢复原版本"
        )));
    }
    Ok(())
}

fn ensure_enabled(config: &qimen_config::MarketplaceConfig) -> Result<(), AdminError> {
    if config.enabled {
        Ok(())
    } else {
        Err(AdminError::BadRequest("插件商城已在配置中关闭".to_string()))
    }
}

fn incompatible_error(compatibility: &VersionCompatibility) -> AdminError {
    AdminError::BadRequest(
        compatibility
            .issues
            .iter()
            .map(|issue| issue.message.as_str())
            .collect::<Vec<_>>()
            .join("；"),
    )
}

fn marketplace_error(error: MarketplaceError) -> AdminError {
    match error {
        MarketplaceError::NotFound(message) => AdminError::NotFound(message),
        MarketplaceError::Conflict(message) => AdminError::Conflict(message),
        MarketplaceError::InvalidMetadata(message) | MarketplaceError::Incompatible(message) => {
            AdminError::BadRequest(message)
        }
        MarketplaceError::Disabled => AdminError::BadRequest("插件商城已在配置中关闭".to_string()),
        MarketplaceError::ChecksumMismatch { expected, actual } => AdminError::Conflict(format!(
            "插件校验失败：期望 SHA256 {expected}，实际为 {actual}"
        )),
        MarketplaceError::Network(message) => AdminError::Unavailable(message),
        MarketplaceError::Http(error) => AdminError::Unavailable(error.to_string()),
        other => AdminError::internal(other),
    }
}

fn runtime_marketplace_error(error: MarketplaceError) -> qimen_error::QimenError {
    qimen_error::QimenError::Runtime(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use qimen_plugin_marketplace::{
        DriverEventKind, DriverSupport, MessageScene, OutboundCapability, PluginAsset,
        PluginDriver, PluginManifest, VersionManifest,
    };

    fn test_snapshot(plugins: Vec<MarketplacePluginView>) -> MarketplaceSnapshot {
        MarketplaceSnapshot {
            enabled: true,
            allow_prerelease: false,
            auto_update: false,
            source: Some(CatalogSource::Cache),
            fetched_at: None,
            warning: None,
            host: HostProfile {
                qimenbot_version: "0.1.17".into(),
                target: "x86_64-pc-windows-msvc".into(),
                os: "windows".into(),
                arch: "x86_64".into(),
                environment: "msvc".into(),
                glibc: None,
                dynamic_loading: true,
                supported_dynamic_apis: vec!["0.5".into()],
            },
            plugins,
        }
    }

    fn test_plugin(id: &str, kind: PluginKind) -> MarketplacePluginView {
        MarketplacePluginView {
            id: id.into(),
            name: format!("Plugin {id}"),
            summary: "测试插件".into(),
            description: String::new(),
            kind,
            repository: format!("example/{id}"),
            repository_url: format!("https://github.com/example/{id}"),
            repository_id: 42,
            license: "MIT".into(),
            authors: vec!["developer".into()],
            categories: vec!["tools".into()],
            keywords: Vec::new(),
            trust: TrustLevel::Community,
            catalog_listed: true,
            latest_compatible: None,
            versions: Vec::new(),
            installed: None,
            unmanaged: None,
        }
    }

    #[test]
    fn marketplace_pagination_returns_only_the_requested_page() {
        let plugins = (0..25)
            .map(|index| test_plugin(&format!("plugin-{index:02}"), PluginKind::Dynamic))
            .collect();
        let query = MarketplaceQuery {
            page: 2,
            page_size: 10,
            query: String::new(),
            filter: MarketplaceFilter::All,
        };

        let view = paginate_snapshot(test_snapshot(plugins), &query);

        assert_eq!(view.pagination.page, 2);
        assert_eq!(view.pagination.page_size, 10);
        assert_eq!(view.pagination.total_items, 25);
        assert_eq!(view.pagination.total_pages, 3);
        assert_eq!(view.plugins.len(), 10);
        assert_eq!(view.plugins[0].id, "plugin-10");
        assert_eq!(view.plugins[9].id, "plugin-19");
    }

    #[test]
    fn marketplace_pagination_clamps_page_and_preserves_category_counts() {
        let mut installed = test_plugin("installed", PluginKind::Dynamic);
        installed.installed = Some(MarketplaceInstalledView {
            version: "1.0.0".into(),
            active_file: "plugin.dll".into(),
            target: "x86_64-pc-windows-msvc".into(),
            sha256: "a".repeat(64),
            installed_at: "2026-08-04T00:00:00Z".into(),
            pinned: false,
            active: true,
            loaded: true,
            update_available: true,
            can_rollback: false,
            data_schema_version: 1,
        });
        let query = MarketplaceQuery {
            page: usize::MAX,
            page_size: 1,
            query: String::new(),
            filter: MarketplaceFilter::Static,
        };

        let view = paginate_snapshot(
            test_snapshot(vec![
                installed,
                test_plugin("source-only", PluginKind::Static),
            ]),
            &query,
        );

        assert_eq!(view.counts.all, 2);
        assert_eq!(view.counts.dynamic, 1);
        assert_eq!(view.counts.static_plugins, 1);
        assert_eq!(view.counts.installed, 1);
        assert_eq!(view.counts.updates, 1);
        assert_eq!(view.pagination.page, 1);
        assert_eq!(view.pagination.total_items, 1);
        assert_eq!(view.plugins[0].id, "source-only");
        let serialized = serde_json::to_value(&view.counts).unwrap();
        assert_eq!(serialized["static"], 1);
    }

    #[test]
    fn marketplace_search_matches_localized_driver_capabilities() {
        let mut plugin = test_plugin("official-tools", PluginKind::Dynamic);
        plugin.versions.push(MarketplaceVersionView {
            version: "1.0.0".into(),
            released_at: "2026-08-04T00:00:00Z".into(),
            channel: ReleaseChannel::Stable,
            qimenbot: ">=0.1.17".into(),
            dynamic_api: Some("0.5".into()),
            yanked: false,
            data_schema_version: 1,
            rollback_safe: true,
            changelog: String::new(),
            drivers: vec![DriverSupport {
                driver: PluginDriver::QqOfficial,
                scenes: vec![MessageScene::GroupAt],
                events: vec![DriverEventKind::Message],
                outbound: vec![OutboundCapability::RichMessage],
            }],
            compatible: true,
            installable: true,
            asset_name: None,
            asset_target: None,
            asset_size_bytes: None,
            asset_sha256: None,
            min_glibc: None,
            github_attestation: false,
            issues: Vec::new(),
        });

        assert!(marketplace_query_matches(&plugin, "官方 qq bot"));
        assert!(marketplace_query_matches(&plugin, "群内 @"));
        assert!(marketplace_query_matches(&plugin, "富媒体"));
        assert!(!marketplace_query_matches(&plugin, "onebot 11"));
    }

    #[test]
    fn requested_version_reports_all_compatibility_reasons() {
        let plugin = CatalogPlugin {
            manifest: PluginManifest {
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
            },
            versions: vec![VersionManifest {
                schema_version: 1,
                version: "1.0.0".into(),
                released_at: "2026-08-03T00:00:00Z".into(),
                release_tag: "v1.0.0".into(),
                channel: ReleaseChannel::Stable,
                qimenbot: ">=9.0.0".into(),
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
                    min_glibc: Some("2.31".into()),
                    github_attestation: false,
                }],
            }],
        };
        let host = HostProfile {
            qimenbot_version: "0.1.16".into(),
            target: "x86_64-pc-windows-msvc".into(),
            os: "windows".into(),
            arch: "x86_64".into(),
            environment: "msvc".into(),
            glibc: None,
            dynamic_loading: true,
            supported_dynamic_apis: vec!["0.5".into()],
        };
        let error = select_requested_version(&plugin, Some("1.0.0"), &host, false).unwrap_err();
        let AdminError::BadRequest(message) = error else {
            panic!("unexpected error variant");
        };
        assert!(message.contains("QimenBot"));
        assert!(message.contains("x86_64-pc-windows-msvc"));
    }
}
