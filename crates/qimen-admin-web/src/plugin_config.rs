use crate::AdminState;
use crate::error::AdminError;
use crate::types::{ApiEnvelope, MutationResult};
use axum::Json;
use axum::extract::{Path, State};
use chrono::{SecondsFormat, Utc};
use qimen_error::QimenError;
use qimen_host_types::{DynamicPluginConfigEntry, DynamicPluginReportEntry};
use qimen_runtime::dynamic_runtime::{is_safe_plugin_config_id, scan_dynamic_plugins};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path as FsPath, PathBuf};
use std::sync::atomic::Ordering;

const MAX_SCHEMA_DEPTH: usize = 64;
const MAX_VALIDATION_ERRORS: usize = 20;
const MAX_CONFIG_REVISIONS: usize = 20;

#[derive(Debug, Serialize)]
pub struct PluginConfigView {
    plugin_id: String,
    plugin_version: String,
    config_version: u32,
    apply_mode: String,
    loaded: bool,
    validates_config: bool,
    applies_live: bool,
    exists: bool,
    revision: String,
    config_file: String,
    schema: Value,
    ui_schema: Value,
    values: Value,
    secrets: Vec<PluginSecretState>,
}

#[derive(Debug, Serialize)]
struct PluginSecretState {
    pointer: String,
    configured: bool,
}

#[derive(Debug, Deserialize)]
pub struct PluginConfigMutationRequest {
    revision: String,
    values: Value,
    /// JSON Pointer -> 新值；`null` 表示清除，未提交表示保留原值。
    #[serde(default)]
    secret_updates: BTreeMap<String, Option<String>>,
    /// 目标 JSON Pointer -> 当前 revision 中的来源 Pointer。
    ///
    /// 前端调整对象数组顺序时，用这个映射让密钥跟随原数组项移动，整个过程
    /// 不会把密钥原文返回浏览器。省略该字段时按相同 Pointer 保留，兼容旧客户端。
    #[serde(default)]
    secret_references: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Serialize)]
pub struct PluginConfigValidationView {
    valid: bool,
    message: String,
}

#[derive(Clone)]
struct ConfigurablePlugin {
    report: DynamicPluginReportEntry,
    descriptor: DynamicPluginConfigEntry,
    loaded: bool,
    config_dir: PathBuf,
}

struct PluginConfigSnapshot {
    path: PathBuf,
    raw: Option<String>,
    values: Value,
    revision: String,
}

#[derive(Clone)]
struct PluginConfigTransaction {
    plugin_id: String,
    path: PathBuf,
    revisions_dir: PathBuf,
    previous_raw: Option<String>,
    previous_revision: String,
    next_raw: String,
}

/// 返回插件导出的 Schema、脱敏后的当前值和保存 revision。
pub async fn get(
    State(state): State<AdminState>,
    Path(plugin_id): Path<String>,
) -> Result<Json<ApiEnvelope<PluginConfigView>>, AdminError> {
    let _operation = state.plugin_operations.lock().await;
    let plugin = configurable_plugin(&state, &plugin_id).await?;
    let schema = parse_schema(&plugin.descriptor)?;
    let ui_schema = parse_ui_schema(&plugin.descriptor)?;
    let store = PluginConfigStore::new(plugin.config_dir.clone());
    let snapshot = store.read(&plugin_id)?;
    let secret_patterns = collect_secret_patterns(&schema)?;
    let secrets = concrete_secret_values(&snapshot.values, &secret_patterns)
        .into_iter()
        .map(|(pointer, value)| PluginSecretState {
            pointer,
            configured: secret_is_configured(value),
        })
        .collect();
    let mut values = snapshot.values.clone();
    redact_secrets(&mut values, &secret_patterns)?;

    Ok(Json(ApiEnvelope::new(PluginConfigView {
        plugin_id: plugin.report.plugin_id,
        plugin_version: plugin.report.plugin_version,
        config_version: plugin.descriptor.config_version,
        apply_mode: plugin.descriptor.apply_mode,
        loaded: plugin.loaded,
        validates_config: plugin.descriptor.validates_config,
        applies_live: plugin.descriptor.applies_live,
        exists: snapshot.raw.is_some(),
        revision: snapshot.revision,
        config_file: snapshot.path.display().to_string(),
        schema,
        ui_schema,
        values,
        secrets,
    })))
}

/// 只执行完整后端与插件语义校验，不写入配置文件。
pub async fn validate(
    State(state): State<AdminState>,
    Path(plugin_id): Path<String>,
    Json(request): Json<PluginConfigMutationRequest>,
) -> Result<Json<ApiEnvelope<PluginConfigValidationView>>, AdminError> {
    let _operation = state.plugin_operations.lock().await;
    let plugin = configurable_plugin(&state, &plugin_id).await?;
    let schema = parse_schema(&plugin.descriptor)?;
    let store = PluginConfigStore::new(plugin.config_dir.clone());
    let snapshot = store.read(&plugin_id)?;
    ensure_revision(&snapshot.revision, &request.revision)?;
    let values = prepare_values(
        &schema,
        &snapshot.values,
        request.values,
        &request.secret_updates,
        request.secret_references.as_ref(),
    )?;
    let config_json = serde_json::to_string(&values).map_err(AdminError::internal)?;
    let previous_json = snapshot_json(&snapshot)?;
    if plugin.loaded && plugin.descriptor.validates_config {
        state
            .runtime
            .validate_dynamic_plugin_config(
                &plugin.report.path,
                &plugin_id,
                &config_json,
                &previous_json,
            )
            .await?;
    }

    Ok(Json(ApiEnvelope::new(PluginConfigValidationView {
        valid: true,
        message: if plugin.loaded && plugin.descriptor.validates_config {
            "Schema 与插件语义校验均已通过".to_string()
        } else {
            "Schema 校验已通过".to_string()
        },
    })))
}

/// 保存配置并按插件声明执行即时应用、动态重载或重启标记。
pub async fn save(
    State(state): State<AdminState>,
    Path(plugin_id): Path<String>,
    Json(request): Json<PluginConfigMutationRequest>,
) -> Result<Json<ApiEnvelope<MutationResult>>, AdminError> {
    let _operation = state.plugin_operations.lock().await;
    let plugin = configurable_plugin(&state, &plugin_id).await?;
    let schema = parse_schema(&plugin.descriptor)?;
    let store = PluginConfigStore::new(plugin.config_dir.clone());
    let snapshot = store.read(&plugin_id)?;
    ensure_revision(&snapshot.revision, &request.revision)?;
    let values = prepare_values(
        &schema,
        &snapshot.values,
        request.values,
        &request.secret_updates,
        request.secret_references.as_ref(),
    )?;
    let config_json = serde_json::to_string(&values).map_err(AdminError::internal)?;
    let previous_json = snapshot_json(&snapshot)?;
    if plugin.loaded && plugin.descriptor.validates_config {
        state
            .runtime
            .validate_dynamic_plugin_config(
                &plugin.report.path,
                &plugin_id,
                &config_json,
                &previous_json,
            )
            .await?;
    }

    let next_raw = values_to_toml(&values)?;
    let transaction = store.prepare(&plugin_id, &snapshot, next_raw)?;
    let (restart_required, message) = match (plugin.loaded, plugin.descriptor.apply_mode.as_str()) {
        (true, "live") => {
            apply_live_config(
                &state,
                &plugin,
                &plugin_id,
                &transaction,
                &config_json,
                &previous_json,
            )
            .await?;
            (false, "配置已保存并即时应用".to_string())
        }
        (true, "reload") => {
            apply_reload_config(&state, &plugin, &plugin_id, &transaction).await?;
            (
                state.restart_required.load(Ordering::Relaxed),
                "配置已保存，动态插件已重新加载".to_string(),
            )
        }
        (true, "restart") => {
            transaction.apply()?;
            state.restart_required.store(true, Ordering::Relaxed);
            (true, "配置已保存，重启宿主后生效".to_string())
        }
        (false, _) => {
            transaction.apply()?;
            (false, "配置已保存，将在插件下次加载时生效".to_string())
        }
        (_, mode) => {
            return Err(AdminError::BadRequest(format!(
                "插件声明了未知配置生效模式 '{mode}'"
            )));
        }
    };

    let revision = revision_for(&transaction.next_raw);
    state.audit.record(
        "plugin.config.save",
        format!("plugin:{plugin_id}"),
        "success",
        format!(
            "configuration saved with apply mode {} and revision {}",
            plugin.descriptor.apply_mode, revision
        ),
    )?;
    Ok(Json(ApiEnvelope::new(MutationResult {
        revision: Some(revision),
        restart_required,
        message,
    })))
}

async fn configurable_plugin(
    state: &AdminState,
    plugin_id: &str,
) -> Result<ConfigurablePlugin, AdminError> {
    if !is_safe_plugin_config_id(plugin_id) {
        return Err(AdminError::BadRequest(
            "插件 ID 不能映射到配置文件".to_string(),
        ));
    }
    let stored = state.config_store.read().await?;
    let plugin_bin_dir = state
        .runtime
        .active_plugin_bin_dir()
        .unwrap_or(&stored.config.official_host.plugin_bin_dir);
    let mut matches = scan_dynamic_plugins(plugin_bin_dir)?
        .into_iter()
        .filter(|plugin| plugin.plugin_id == plugin_id)
        .collect::<Vec<_>>();
    if matches.len() > 1 {
        return Err(AdminError::Conflict(format!(
            "发现多个 ID 为 '{plugin_id}' 的动态库，请先清理重复文件"
        )));
    }
    let report = matches
        .pop()
        .ok_or_else(|| AdminError::NotFound(format!("没有发现动态插件 '{plugin_id}'")))?;
    let descriptor = report.config.clone().ok_or_else(|| {
        AdminError::BadRequest(format!("插件 '{plugin_id}' 没有声明在线配置 Schema"))
    })?;
    let loaded = state.runtime.host_plugin_report().is_some_and(|host| {
        host.dynamic_plugins
            .iter()
            .any(|plugin| plugin.plugin_id == plugin_id && plugin.path == report.path)
    });
    let config_dir = state
        .runtime
        .active_plugin_config_dir()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(&stored.config.official_host.plugin_config_dir));
    Ok(ConfigurablePlugin {
        report,
        descriptor,
        loaded,
        config_dir,
    })
}

fn parse_schema(descriptor: &DynamicPluginConfigEntry) -> Result<Value, AdminError> {
    let schema: Value =
        serde_json::from_str(&descriptor.schema_json).map_err(AdminError::internal)?;
    reject_external_references(&schema, 0)?;
    jsonschema::draft202012::options()
        .should_validate_formats(true)
        .should_ignore_unknown_formats(true)
        .build(&schema)
        .map_err(|error| AdminError::BadRequest(format!("插件配置 Schema 无效：{error}")))?;
    Ok(schema)
}

fn parse_ui_schema(descriptor: &DynamicPluginConfigEntry) -> Result<Value, AdminError> {
    if descriptor.ui_schema_json.trim().is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    let value: Value =
        serde_json::from_str(&descriptor.ui_schema_json).map_err(AdminError::internal)?;
    if value.is_object() {
        Ok(value)
    } else {
        Err(AdminError::BadRequest(
            "插件 UI Schema 根节点必须是对象".to_string(),
        ))
    }
}

fn reject_external_references(value: &Value, depth: usize) -> Result<(), AdminError> {
    if depth > MAX_SCHEMA_DEPTH {
        return Err(AdminError::BadRequest(
            "插件配置 Schema 嵌套过深".to_string(),
        ));
    }
    match value {
        Value::Object(object) => {
            for key in ["$ref", "$dynamicRef"] {
                if let Some(reference) = object.get(key).and_then(Value::as_str)
                    && !reference.starts_with('#')
                {
                    return Err(AdminError::BadRequest(format!(
                        "插件配置 Schema 不允许远程引用 '{reference}'"
                    )));
                }
            }
            for child in object.values() {
                reject_external_references(child, depth + 1)?;
            }
        }
        Value::Array(array) => {
            for child in array {
                reject_external_references(child, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn prepare_values(
    schema: &Value,
    current: &Value,
    mut submitted: Value,
    secret_updates: &BTreeMap<String, Option<String>>,
    secret_references: Option<&BTreeMap<String, String>>,
) -> Result<Value, AdminError> {
    if !submitted.is_object() {
        return Err(AdminError::BadRequest(
            "插件配置根节点必须是对象".to_string(),
        ));
    }
    let patterns = collect_secret_patterns(schema)?;
    reject_plaintext_secret_submission(&submitted, &patterns, secret_updates)?;

    let current_secrets = concrete_secret_values(current, &patterns)
        .into_iter()
        .filter(|(_, value)| secret_is_configured(value))
        .map(|(pointer, value)| (pointer, value.clone()))
        .collect::<BTreeMap<_, _>>();
    if let Some(references) = secret_references {
        let mut used_sources = BTreeSet::new();
        for (destination, source) in references {
            if secret_updates.contains_key(destination) {
                return Err(AdminError::BadRequest(format!(
                    "密钥字段 '{destination}' 不能同时引用旧值和提交新值"
                )));
            }
            let destination_segments = parse_pointer(destination)?;
            let source_segments = parse_pointer(source)?;
            if !patterns.iter().any(|pattern| {
                pattern_matches(pattern, &destination_segments)
                    && pattern_matches(pattern, &source_segments)
            }) {
                return Err(AdminError::BadRequest(format!(
                    "密钥引用 '{source}' -> '{destination}' 不属于同一 Schema 字段"
                )));
            }
            if !used_sources.insert(source.clone()) {
                return Err(AdminError::BadRequest(format!(
                    "密钥来源 '{source}' 不能被重复引用"
                )));
            }
            let value = current_secrets.get(source).ok_or_else(|| {
                AdminError::BadRequest(format!("当前配置中没有可保留的密钥来源 '{source}'"))
            })?;
            set_pointer(&mut submitted, destination, value.clone())?;
        }
    } else {
        for (pointer, value) in &current_secrets {
            if !secret_updates.contains_key(pointer) {
                set_pointer(&mut submitted, pointer, value.clone())?;
            }
        }
    }
    for (pointer, update) in secret_updates {
        let segments = parse_pointer(pointer)?;
        if !patterns
            .iter()
            .any(|pattern| pattern_matches(pattern, &segments))
        {
            return Err(AdminError::BadRequest(format!(
                "'{pointer}' 不是插件声明的密钥字段"
            )));
        }
        match update {
            Some(value) => set_pointer(&mut submitted, pointer, Value::String(value.clone()))?,
            None => remove_pointer(&mut submitted, pointer)?,
        }
    }
    remove_redacted_secret_nulls(&mut submitted, &patterns)?;
    normalize_for_toml(&mut submitted, false)?;
    validate_instance(schema, &submitted)?;
    Ok(submitted)
}

fn validate_instance(schema: &Value, values: &Value) -> Result<(), AdminError> {
    let validator = jsonschema::draft202012::options()
        .should_validate_formats(true)
        .should_ignore_unknown_formats(true)
        .build(schema)
        .map_err(|error| AdminError::BadRequest(format!("插件配置 Schema 无效：{error}")))?;
    let errors = validator
        .iter_errors(values)
        .take(MAX_VALIDATION_ERRORS)
        .map(|error| {
            let path = error.instance_path().as_str();
            if path.is_empty() {
                error.to_string()
            } else {
                format!("{path}: {error}")
            }
        })
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(AdminError::BadRequest(format!(
            "插件配置校验失败：{}",
            errors.join("；")
        )))
    }
}

async fn apply_live_config(
    state: &AdminState,
    plugin: &ConfigurablePlugin,
    plugin_id: &str,
    transaction: &PluginConfigTransaction,
    config_json: &str,
    previous_json: &str,
) -> Result<(), AdminError> {
    transaction.apply()?;
    if let Err(apply_error) = state
        .runtime
        .apply_dynamic_plugin_config(&plugin.report.path, plugin_id, config_json, previous_json)
        .await
    {
        let rollback_file = transaction.rollback();
        let rollback_runtime = if rollback_file.is_ok() {
            state
                .runtime
                .apply_dynamic_plugin_config(
                    &plugin.report.path,
                    plugin_id,
                    previous_json,
                    config_json,
                )
                .await
        } else {
            Err(QimenError::Runtime(
                "配置文件恢复失败，未尝试恢复插件内存状态".to_string(),
            ))
        };
        return Err(AdminError::BadRequest(format!(
            "插件拒绝即时配置：{apply_error}；文件恢复：{}；运行状态恢复：{}",
            result_summary(rollback_file),
            result_summary(rollback_runtime)
        )));
    }
    Ok(())
}

async fn apply_reload_config(
    state: &AdminState,
    plugin: &ConfigurablePlugin,
    plugin_id: &str,
    transaction: &PluginConfigTransaction,
) -> Result<(), AdminError> {
    let apply = transaction.clone();
    let rollback = transaction.clone();
    state
        .runtime
        .reload_dynamic_plugins_transaction(
            move || apply.apply_qimen(),
            move || rollback.rollback_qimen(),
        )
        .await?;
    if plugin_is_loaded(state, plugin_id, &plugin.report.path) {
        return Ok(());
    }

    // init 返回失败时运行时会跳过该插件，但整体扫描仍可能成功；这里显式恢复旧配置。
    let restore = transaction.clone();
    let restore_rollback = transaction.clone();
    let restore_result = state
        .runtime
        .reload_dynamic_plugins_transaction(
            move || restore.rollback_qimen(),
            move || restore_rollback.apply_qimen(),
        )
        .await;
    Err(AdminError::BadRequest(format!(
        "新配置导致插件初始化失败，已恢复旧配置：{}",
        result_summary(restore_result.map(|_| ()))
    )))
}

fn plugin_is_loaded(state: &AdminState, plugin_id: &str, path: &str) -> bool {
    state.runtime.host_plugin_report().is_some_and(|report| {
        report
            .dynamic_plugins
            .iter()
            .any(|plugin| plugin.plugin_id == plugin_id && plugin.path == path)
    })
}

fn snapshot_json(snapshot: &PluginConfigSnapshot) -> Result<String, AdminError> {
    if snapshot.raw.is_none() {
        Ok(String::new())
    } else {
        serde_json::to_string(&snapshot.values).map_err(AdminError::internal)
    }
}

fn values_to_toml(values: &Value) -> Result<String, AdminError> {
    let value = toml::Value::try_from(values.clone())
        .map_err(|error| AdminError::BadRequest(format!("配置不能转换为 TOML：{error}")))?;
    toml::to_string_pretty(&value)
        .map_err(|error| AdminError::BadRequest(format!("配置不能序列化为 TOML：{error}")))
}

fn normalize_for_toml(value: &mut Value, in_array: bool) -> Result<(), AdminError> {
    match value {
        Value::Null if in_array => Err(AdminError::BadRequest(
            "TOML 数组不能保存 null，请删除该数组项".to_string(),
        )),
        Value::Null => Ok(()),
        Value::Object(object) => {
            let null_keys = object
                .iter()
                .filter_map(|(key, value)| value.is_null().then_some(key.clone()))
                .collect::<Vec<_>>();
            for key in null_keys {
                object.remove(&key);
            }
            for child in object.values_mut() {
                normalize_for_toml(child, false)?;
            }
            Ok(())
        }
        Value::Array(array) => {
            for child in array {
                normalize_for_toml(child, true)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

struct PluginConfigStore {
    root: PathBuf,
}

impl PluginConfigStore {
    fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn read(&self, plugin_id: &str) -> Result<PluginConfigSnapshot, AdminError> {
        let path = self.path(plugin_id)?;
        let raw = match fs::read_to_string(&path) {
            Ok(raw) => Some(raw),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        let values = match raw.as_deref() {
            Some(raw) => {
                let value = raw.parse::<toml::Value>().map_err(|error| {
                    AdminError::BadRequest(format!(
                        "插件配置 '{}' 不是有效 TOML：{error}",
                        path.display()
                    ))
                })?;
                serde_json::to_value(value).map_err(AdminError::internal)?
            }
            None => Value::Object(Map::new()),
        };
        let revision = revision_for(raw.as_deref().unwrap_or_default());
        Ok(PluginConfigSnapshot {
            path,
            raw,
            values,
            revision,
        })
    }

    fn prepare(
        &self,
        plugin_id: &str,
        snapshot: &PluginConfigSnapshot,
        next_raw: String,
    ) -> Result<PluginConfigTransaction, AdminError> {
        let current = self.read(plugin_id)?;
        ensure_revision(&current.revision, &snapshot.revision)?;
        Ok(PluginConfigTransaction {
            plugin_id: plugin_id.to_string(),
            path: current.path,
            revisions_dir: self.root.join(".qimen-revisions").join(plugin_id),
            previous_raw: current.raw,
            previous_revision: current.revision,
            next_raw,
        })
    }

    fn path(&self, plugin_id: &str) -> Result<PathBuf, AdminError> {
        if !is_safe_plugin_config_id(plugin_id) {
            return Err(AdminError::BadRequest(
                "插件 ID 不能映射到配置文件".to_string(),
            ));
        }
        Ok(self.root.join(format!("{plugin_id}.toml")))
    }
}

impl PluginConfigTransaction {
    fn apply(&self) -> Result<(), AdminError> {
        if let Some(previous) = &self.previous_raw {
            self.backup(previous)?;
        }
        atomic_replace(&self.path, &self.next_raw).map_err(AdminError::from)
    }

    fn rollback(&self) -> Result<(), AdminError> {
        match &self.previous_raw {
            Some(previous) => atomic_replace(&self.path, previous).map_err(AdminError::from),
            None => match fs::remove_file(&self.path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error.into()),
            },
        }
    }

    fn apply_qimen(&self) -> qimen_error::Result<()> {
        self.apply().map_err(|error| {
            QimenError::Runtime(format!(
                "failed to save config for plugin '{}': {error:?}",
                self.plugin_id
            ))
        })
    }

    fn rollback_qimen(&self) -> qimen_error::Result<()> {
        self.rollback().map_err(|error| {
            QimenError::Runtime(format!(
                "failed to restore config revision {} for plugin '{}': {error:?}",
                self.previous_revision, self.plugin_id
            ))
        })
    }

    fn backup(&self, raw: &str) -> Result<(), AdminError> {
        fs::create_dir_all(&self.revisions_dir)?;
        let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let timestamp = timestamp.replace([':', '.'], "-");
        let path = self
            .revisions_dir
            .join(format!("{timestamp}-{}.toml", self.previous_revision));
        if !path.exists() {
            fs::write(path, raw)?;
        }
        let mut entries = fs::read_dir(&self.revisions_dir)?
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.path().extension().and_then(|value| value.to_str()) == Some("toml")
            })
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        let remove_count = entries.len().saturating_sub(MAX_CONFIG_REVISIONS);
        for entry in entries.into_iter().take(remove_count) {
            let _ = fs::remove_file(entry.path());
        }
        Ok(())
    }
}

fn atomic_replace(path: &FsPath, raw: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("plugin.toml");
    let temp = path.with_file_name(format!("{file_name}.{}.tmp", std::process::id()));
    fs::write(&temp, raw)?;
    if let Err(first_error) = fs::rename(&temp, path) {
        if path.exists() {
            fs::remove_file(path)?;
            fs::rename(&temp, path)?;
        } else {
            let _ = fs::remove_file(&temp);
            return Err(first_error);
        }
    }
    Ok(())
}

fn revision_for(raw: &str) -> String {
    let digest = Sha256::digest(raw.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn ensure_revision(current: &str, expected: &str) -> Result<(), AdminError> {
    if current == expected {
        Ok(())
    } else {
        Err(AdminError::Conflict(format!(
            "插件配置已被其他操作修改，当前 revision 为 {current}"
        )))
    }
}

fn collect_secret_patterns(schema: &Value) -> Result<Vec<Vec<String>>, AdminError> {
    let mut patterns = BTreeSet::new();
    let mut refs = BTreeSet::new();
    collect_secret_patterns_inner(schema, schema, &mut Vec::new(), &mut refs, &mut patterns, 0)?;
    Ok(patterns.into_iter().collect())
}

fn collect_secret_patterns_inner(
    schema: &Value,
    root: &Value,
    path: &mut Vec<String>,
    refs: &mut BTreeSet<String>,
    patterns: &mut BTreeSet<Vec<String>>,
    depth: usize,
) -> Result<(), AdminError> {
    if depth > MAX_SCHEMA_DEPTH {
        return Err(AdminError::BadRequest(
            "插件配置 Schema 嵌套过深".to_string(),
        ));
    }
    let Some(object) = schema.as_object() else {
        return Ok(());
    };
    if let Some(reference) = object.get("$ref").and_then(Value::as_str)
        && refs.insert(reference.to_string())
    {
        let target = resolve_local_reference(root, reference)?;
        collect_secret_patterns_inner(target, root, path, refs, patterns, depth + 1)?;
        refs.remove(reference);
    }
    let secret = object
        .get("x-qimen-secret")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || object.get("format").and_then(Value::as_str) == Some("password")
        || object.get("writeOnly").and_then(Value::as_bool) == Some(true);
    if secret {
        patterns.insert(path.clone());
    }
    for keyword in ["allOf", "anyOf", "oneOf"] {
        if let Some(branches) = object.get(keyword).and_then(Value::as_array) {
            for branch in branches {
                collect_secret_patterns_inner(branch, root, path, refs, patterns, depth + 1)?;
            }
        }
    }
    for keyword in ["if", "then", "else", "not"] {
        if let Some(branch) = object.get(keyword) {
            collect_secret_patterns_inner(branch, root, path, refs, patterns, depth + 1)?;
        }
    }
    if let Some(properties) = object.get("properties").and_then(Value::as_object) {
        for (name, child) in properties {
            path.push(name.clone());
            collect_secret_patterns_inner(child, root, path, refs, patterns, depth + 1)?;
            path.pop();
        }
    }
    if let Some(items) = object.get("items")
        && items.is_object()
    {
        path.push("*".to_string());
        collect_secret_patterns_inner(items, root, path, refs, patterns, depth + 1)?;
        path.pop();
    }
    if let Some(items) = object.get("prefixItems").and_then(Value::as_array) {
        for (index, item) in items.iter().enumerate() {
            path.push(index.to_string());
            collect_secret_patterns_inner(item, root, path, refs, patterns, depth + 1)?;
            path.pop();
        }
    }
    if let Some(additional) = object.get("additionalProperties")
        && additional.is_object()
    {
        path.push("*".to_string());
        collect_secret_patterns_inner(additional, root, path, refs, patterns, depth + 1)?;
        path.pop();
    }
    Ok(())
}

fn resolve_local_reference<'a>(root: &'a Value, reference: &str) -> Result<&'a Value, AdminError> {
    if reference == "#" {
        return Ok(root);
    }
    let pointer = reference
        .strip_prefix('#')
        .ok_or_else(|| AdminError::BadRequest(format!("不允许远程 Schema 引用 '{reference}'")))?;
    root.pointer(pointer)
        .ok_or_else(|| AdminError::BadRequest(format!("Schema 引用 '{reference}' 不存在")))
}

fn concrete_secret_values<'a>(
    values: &'a Value,
    patterns: &[Vec<String>],
) -> Vec<(String, &'a Value)> {
    let mut found = BTreeMap::<String, &'a Value>::new();
    for pattern in patterns {
        expand_secret_pattern(values, pattern, 0, &mut Vec::new(), &mut found);
    }
    found.into_iter().collect()
}

fn expand_secret_pattern<'a>(
    value: &'a Value,
    pattern: &[String],
    index: usize,
    path: &mut Vec<String>,
    found: &mut BTreeMap<String, &'a Value>,
) {
    if index == pattern.len() {
        found.insert(pointer_for(path), value);
        return;
    }
    let segment = &pattern[index];
    if segment == "*" {
        match value {
            Value::Object(object) => {
                for (key, child) in object {
                    path.push(key.clone());
                    expand_secret_pattern(child, pattern, index + 1, path, found);
                    path.pop();
                }
            }
            Value::Array(array) => {
                for (child_index, child) in array.iter().enumerate() {
                    path.push(child_index.to_string());
                    expand_secret_pattern(child, pattern, index + 1, path, found);
                    path.pop();
                }
            }
            _ => {}
        }
    } else if let Some(child) = child_at(value, segment) {
        path.push(segment.clone());
        expand_secret_pattern(child, pattern, index + 1, path, found);
        path.pop();
    }
}

fn child_at<'a>(value: &'a Value, segment: &str) -> Option<&'a Value> {
    match value {
        Value::Object(object) => object.get(segment),
        Value::Array(array) => segment
            .parse::<usize>()
            .ok()
            .and_then(|index| array.get(index)),
        _ => None,
    }
}

fn redact_secrets(values: &mut Value, patterns: &[Vec<String>]) -> Result<(), AdminError> {
    let pointers = concrete_secret_values(values, patterns)
        .into_iter()
        .map(|(pointer, _)| pointer)
        .collect::<Vec<_>>();
    for pointer in pointers {
        set_pointer(values, &pointer, Value::Null)?;
    }
    Ok(())
}

fn reject_plaintext_secret_submission(
    submitted: &Value,
    patterns: &[Vec<String>],
    secret_updates: &BTreeMap<String, Option<String>>,
) -> Result<(), AdminError> {
    for (pointer, value) in concrete_secret_values(submitted, patterns) {
        if !value.is_null() && !secret_updates.contains_key(&pointer) {
            return Err(AdminError::BadRequest(format!(
                "密钥字段 '{pointer}' 必须通过 secret_updates 提交"
            )));
        }
    }
    Ok(())
}

fn remove_redacted_secret_nulls(
    submitted: &mut Value,
    patterns: &[Vec<String>],
) -> Result<(), AdminError> {
    let pointers = concrete_secret_values(submitted, patterns)
        .into_iter()
        .filter_map(|(pointer, value)| value.is_null().then_some(pointer))
        .collect::<Vec<_>>();
    for pointer in pointers {
        remove_pointer(submitted, &pointer)?;
    }
    Ok(())
}

fn secret_is_configured(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(value) => !value.is_empty(),
        _ => true,
    }
}

fn parse_pointer(pointer: &str) -> Result<Vec<String>, AdminError> {
    if pointer.is_empty() {
        return Ok(Vec::new());
    }
    if !pointer.starts_with('/') {
        return Err(AdminError::BadRequest(format!(
            "'{pointer}' 不是有效 JSON Pointer"
        )));
    }
    pointer[1..]
        .split('/')
        .map(|segment| decode_pointer_segment(segment, pointer))
        .collect()
}

fn decode_pointer_segment(segment: &str, pointer: &str) -> Result<String, AdminError> {
    let mut output = String::new();
    let mut chars = segment.chars();
    while let Some(character) = chars.next() {
        if character != '~' {
            output.push(character);
            continue;
        }
        match chars.next() {
            Some('0') => output.push('~'),
            Some('1') => output.push('/'),
            _ => {
                return Err(AdminError::BadRequest(format!(
                    "'{pointer}' 包含无效 JSON Pointer 转义"
                )));
            }
        }
    }
    Ok(output)
}

fn pointer_for(segments: &[String]) -> String {
    segments.iter().fold(String::new(), |mut pointer, segment| {
        pointer.push('/');
        pointer.push_str(&segment.replace('~', "~0").replace('/', "~1"));
        pointer
    })
}

fn pattern_matches(pattern: &[String], pointer: &[String]) -> bool {
    pattern.len() == pointer.len()
        && pattern
            .iter()
            .zip(pointer)
            .all(|(expected, actual)| expected == "*" || expected == actual)
}

fn set_pointer(root: &mut Value, pointer: &str, value: Value) -> Result<(), AdminError> {
    let segments = parse_pointer(pointer)?;
    if segments.is_empty() {
        *root = value;
        return Ok(());
    }
    let mut current = root;
    for segment in &segments[..segments.len() - 1] {
        match current {
            Value::Object(object) => {
                current = object
                    .entry(segment.clone())
                    .or_insert_with(|| Value::Object(Map::new()));
            }
            Value::Array(array) => {
                let index = segment
                    .parse::<usize>()
                    .map_err(|_| AdminError::BadRequest(format!("'{pointer}' 的数组索引无效")))?;
                current = array
                    .get_mut(index)
                    .ok_or_else(|| AdminError::BadRequest(format!("'{pointer}' 的数组索引越界")))?;
            }
            _ => {
                return Err(AdminError::BadRequest(format!(
                    "'{pointer}' 的父节点不是对象或数组"
                )));
            }
        }
    }
    let last = segments.last().expect("non-empty pointer");
    match current {
        Value::Object(object) => {
            object.insert(last.clone(), value);
            Ok(())
        }
        Value::Array(array) => {
            let index = last
                .parse::<usize>()
                .map_err(|_| AdminError::BadRequest(format!("'{pointer}' 的数组索引无效")))?;
            let target = array
                .get_mut(index)
                .ok_or_else(|| AdminError::BadRequest(format!("'{pointer}' 的数组索引越界")))?;
            *target = value;
            Ok(())
        }
        _ => Err(AdminError::BadRequest(format!(
            "'{pointer}' 的父节点不是对象或数组"
        ))),
    }
}

fn remove_pointer(root: &mut Value, pointer: &str) -> Result<(), AdminError> {
    let segments = parse_pointer(pointer)?;
    if segments.is_empty() {
        *root = Value::Object(Map::new());
        return Ok(());
    }
    let mut current = root;
    for segment in &segments[..segments.len() - 1] {
        current = match current {
            Value::Object(object) => match object.get_mut(segment) {
                Some(value) => value,
                None => return Ok(()),
            },
            Value::Array(array) => {
                let Some(index) = segment.parse::<usize>().ok() else {
                    return Ok(());
                };
                let Some(value) = array.get_mut(index) else {
                    return Ok(());
                };
                value
            }
            _ => return Ok(()),
        };
    }
    let last = segments.last().expect("non-empty pointer");
    match current {
        Value::Object(object) => {
            object.remove(last);
        }
        Value::Array(array) => {
            if let Ok(index) = last.parse::<usize>()
                && index < array.len()
            {
                array.remove(index);
            }
        }
        _ => {}
    }
    Ok(())
}

fn result_summary(result: impl ResultStatus) -> String {
    result.summary()
}

trait ResultStatus {
    fn summary(self) -> String;
}

impl<E: std::fmt::Debug> ResultStatus for Result<(), E> {
    fn summary(self) -> String {
        match self {
            Ok(()) => "成功".to_string(),
            Err(error) => format!("失败（{error:?}）"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_schema() -> Value {
        serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "required": ["endpoint", "token"],
            "properties": {
                "endpoint": { "type": "string", "format": "uri" },
                "token": { "type": "string", "format": "password", "minLength": 1 },
                "connections": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" },
                            "secret": { "type": "string", "x-qimen-secret": true }
                        }
                    }
                }
            }
        })
    }

    #[test]
    fn secret_patterns_cover_nested_array_items() {
        let patterns = collect_secret_patterns(&test_schema()).unwrap();
        assert!(patterns.contains(&vec!["token".to_string()]));
        assert!(patterns.contains(&vec![
            "connections".to_string(),
            "*".to_string(),
            "secret".to_string()
        ]));
    }

    #[test]
    fn secret_values_are_preserved_without_returning_them_to_the_browser() {
        let schema = test_schema();
        let current = serde_json::json!({
            "endpoint": "https://example.com",
            "token": "existing-secret"
        });
        let mut visible = current.clone();
        let patterns = collect_secret_patterns(&schema).unwrap();
        redact_secrets(&mut visible, &patterns).unwrap();
        assert!(visible["token"].is_null());

        let submitted = serde_json::json!({
            "endpoint": "https://example.org",
            "token": null
        });
        let prepared =
            prepare_values(&schema, &current, submitted, &BTreeMap::new(), None).unwrap();
        assert_eq!(prepared["token"], "existing-secret");
        assert_eq!(prepared["endpoint"], "https://example.org");
    }

    #[test]
    fn plaintext_secret_must_use_the_separate_update_channel() {
        let schema = test_schema();
        let submitted = serde_json::json!({
            "endpoint": "https://example.com",
            "token": "do-not-log-me"
        });
        assert!(
            prepare_values(
                &schema,
                &Value::Object(Map::new()),
                submitted,
                &BTreeMap::new(),
                None,
            )
            .is_err()
        );
    }

    #[test]
    fn empty_plaintext_secret_still_requires_the_secret_channel() {
        let schema = test_schema();
        let submitted = serde_json::json!({
            "endpoint": "https://example.com",
            "token": ""
        });
        assert!(
            prepare_values(
                &schema,
                &Value::Object(Map::new()),
                submitted,
                &BTreeMap::new(),
                None,
            )
            .is_err()
        );
    }

    #[test]
    fn secret_references_keep_secrets_attached_when_array_items_move() {
        let schema = test_schema();
        let current = serde_json::json!({
            "endpoint": "https://example.com",
            "token": "root-secret",
            "connections": [
                { "name": "first", "secret": "first-secret" },
                { "name": "second", "secret": "second-secret" }
            ]
        });
        let submitted = serde_json::json!({
            "endpoint": "https://example.com",
            "token": null,
            "connections": [
                { "name": "second", "secret": null },
                { "name": "first", "secret": null }
            ]
        });
        let references = BTreeMap::from([
            ("/token".to_string(), "/token".to_string()),
            (
                "/connections/0/secret".to_string(),
                "/connections/1/secret".to_string(),
            ),
            (
                "/connections/1/secret".to_string(),
                "/connections/0/secret".to_string(),
            ),
        ]);
        let prepared = prepare_values(
            &schema,
            &current,
            submitted,
            &BTreeMap::new(),
            Some(&references),
        )
        .unwrap();
        assert_eq!(prepared["connections"][0]["secret"], "second-secret");
        assert_eq!(prepared["connections"][1]["secret"], "first-secret");
    }

    #[test]
    fn secret_references_cannot_copy_across_schema_fields() {
        let schema = test_schema();
        let current = serde_json::json!({
            "endpoint": "https://example.com",
            "token": "root-secret",
            "connections": [{ "name": "first", "secret": "item-secret" }]
        });
        let submitted = serde_json::json!({
            "endpoint": "https://example.com",
            "token": null,
            "connections": [{ "name": "first", "secret": null }]
        });
        let references =
            BTreeMap::from([("/token".to_string(), "/connections/0/secret".to_string())]);
        assert!(
            prepare_values(
                &schema,
                &current,
                submitted,
                &BTreeMap::new(),
                Some(&references),
            )
            .is_err()
        );
    }

    #[test]
    fn external_schema_references_are_rejected() {
        let schema = serde_json::json!({ "$ref": "https://example.com/schema.json" });
        assert!(reject_external_references(&schema, 0).is_err());
    }

    #[test]
    fn store_uses_revision_conflicts_and_restores_missing_files() {
        let root = std::env::temp_dir().join(format!(
            "qimen-plugin-config-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let store = PluginConfigStore::new(root.clone());
        let snapshot = store.read("test-plugin").unwrap();
        let transaction = store
            .prepare("test-plugin", &snapshot, "enabled = true\n".to_string())
            .unwrap();
        transaction.apply().unwrap();
        assert!(root.join("test-plugin.toml").is_file());
        assert!(
            store
                .prepare("test-plugin", &snapshot, String::new())
                .is_err()
        );
        transaction.rollback().unwrap();
        assert!(!root.join("test-plugin.toml").exists());
        let _ = fs::remove_dir_all(root);
    }
}
