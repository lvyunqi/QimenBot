use abi_stable::std_types::RString;
use abi_stable_host_api::{
    CommandRequest, CommandResponse, DynamicActionResponse, InterceptorRequest,
    InterceptorResponse, NoticeRequest, NoticeResponse, PluginDescriptor, PluginInitConfig,
    PluginInitResult, is_compatible_api_version, ACTION_APPROVE, ACTION_IGNORE, ACTION_REJECT,
    ACTION_REPLY,
};
use qimen_error::{QimenError, Result};
use qimen_host_types::{
    DynamicCommandDescriptor, DynamicCommandEntry, DynamicInterceptorDescriptor,
    DynamicInterceptorEntry, DynamicMetaDescriptor, DynamicNoticeDescriptor,
    DynamicPluginReportEntry, DynamicRequestDescriptor, DynamicRouteEntry,
    DynamicRuntimeHealthEntry,
};
use qimen_message::Message;
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MAX_ERROR_HISTORY: usize = 10;

struct LoadedLibrary {
    library: libloading::Library,
    last_used: Instant,
    failures: u32,
    tripped_until: Option<Instant>,
    last_error: Option<String>,
    recent_errors: VecDeque<String>,
}

#[derive(Default)]
pub struct DynamicPluginRuntime {
    libraries: HashMap<String, LoadedLibrary>,
}

/// Response from a dynamic plugin callback.
pub enum DynamicResponse {
    /// Reply with a Message (supports rich content).
    ReplyMessage(Message),
    /// Reply with plain text (backward compatible).
    Reply(String),
    /// Ignore the event.
    Ignore,
    /// Approve a friend/group request.
    Approve(Option<String>),
    /// Reject a friend/group request.
    Reject(Option<String>),
}

impl DynamicPluginRuntime {
    pub fn new() -> Self {
        Self {
            libraries: HashMap::new(),
        }
    }

    /// Execute a command callback with v0.2 context.
    pub fn execute_command(
        &mut self,
        descriptor: &DynamicCommandDescriptor,
        args: &[String],
        sender_id: &str,
        group_id: &str,
        raw_event_json: &str,
        sender_nickname: &str,
        message_id: &str,
        timestamp: i64,
    ) -> Result<DynamicResponse> {
        self.evict_idle_libraries(Duration::from_secs(300));
        let path = descriptor.library_path.clone();
        let callback = descriptor.callback_symbol.clone();
        let path_for_error = path.clone();
        let args_str = args.join(" ");
        let command_name = descriptor.command_name.clone();
        let sender = sender_id.to_string();
        let group = group_id.to_string();
        let raw_json = raw_event_json.to_string();
        let nickname = sender_nickname.to_string();
        let msg_id = message_id.to_string();

        self.with_library(&path, move |library| unsafe {
            let symbol: libloading::Symbol<
                unsafe extern "C" fn(CommandRequest) -> CommandResponse,
            > = library.get(callback.as_bytes()).map_err(|err| {
                QimenError::Runtime(format!(
                    "failed to load dynamic callback '{}' from '{}': {err}",
                    callback, path_for_error
                ))
            })?;

            let request = CommandRequest {
                args: RString::from(args_str),
                command_name: RString::from(command_name),
                sender_id: RString::from(sender),
                group_id: RString::from(group),
                raw_event_json: RString::from(raw_json),
                sender_nickname: RString::from(nickname),
                message_id: RString::from(msg_id),
                timestamp,
            };

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                symbol(request)
            }));

            match result {
                Ok(response) => Ok(map_action_response(response.action)),
                Err(panic_info) => {
                    let panic_msg = panic_info
                        .downcast_ref::<String>()
                        .map(|s| s.as_str())
                        .or_else(|| panic_info.downcast_ref::<&str>().copied())
                        .unwrap_or("unknown panic");
                    tracing::error!(
                        callback = %callback,
                        path = %path_for_error,
                        panic = %panic_msg,
                        "dynamic plugin callback panicked"
                    );
                    Err(QimenError::Runtime(format!(
                        "dynamic plugin callback '{}' panicked: {}",
                        callback, panic_msg
                    )))
                }
            }
        })
    }

    /// Legacy execute_command for backward compatibility (no context).
    pub fn execute_command_legacy(
        &mut self,
        descriptor: &DynamicCommandDescriptor,
        args: &[String],
    ) -> Result<DynamicResponse> {
        self.execute_command(descriptor, args, "", "", "{}", "", "", 0)
    }

    pub fn execute_notice(
        &mut self,
        descriptor: &DynamicNoticeDescriptor,
        raw_event_json: &str,
    ) -> Result<DynamicResponse> {
        self.execute_route_callback(
            &descriptor.library_path,
            &descriptor.callback_symbol,
            descriptor.notice_route.clone(),
            "notice",
            raw_event_json,
        )
    }

    pub fn execute_request(
        &mut self,
        descriptor: &DynamicRequestDescriptor,
        raw_event_json: &str,
    ) -> Result<DynamicResponse> {
        self.execute_route_callback(
            &descriptor.library_path,
            &descriptor.callback_symbol,
            descriptor.request_route.clone(),
            "request",
            raw_event_json,
        )
    }

    pub fn execute_meta(
        &mut self,
        descriptor: &DynamicMetaDescriptor,
        raw_event_json: &str,
    ) -> Result<DynamicResponse> {
        self.execute_route_callback(
            &descriptor.library_path,
            &descriptor.callback_symbol,
            descriptor.meta_route.clone(),
            "meta",
            raw_event_json,
        )
    }

    pub fn health_entries(&self) -> Vec<DynamicRuntimeHealthEntry> {
        self.libraries
            .iter()
            .map(|(path, entry)| DynamicRuntimeHealthEntry {
                path: path.clone(),
                failures: entry.failures,
                isolated_until_epoch_ms: entry.tripped_until.map(instant_to_epoch_ms),
                last_error: entry.last_error.clone(),
                recent_errors: entry.recent_errors.iter().cloned().collect(),
            })
            .collect()
    }

    pub fn clear_errors(&mut self) {
        for entry in self.libraries.values_mut() {
            entry.failures = 0;
            entry.tripped_until = None;
            entry.last_error = None;
            entry.recent_errors.clear();
        }
    }

    /// Call the optional `qimen_plugin_init` lifecycle hook for a loaded plugin.
    /// Returns Ok(()) if the symbol doesn't exist (it's optional) or if init succeeds.
    pub fn call_plugin_init(
        &mut self,
        library_path: &str,
        plugin_id: &str,
        config_json: &str,
        plugin_dir: &str,
        data_dir: &str,
    ) -> Result<()> {
        let plugin_id_owned = plugin_id.to_string();
        let config_json_owned = config_json.to_string();
        let plugin_dir_owned = plugin_dir.to_string();
        let data_dir_owned = data_dir.to_string();
        let path_for_error = library_path.to_string();

        self.with_library(library_path, move |library| unsafe {
            // Try to load the optional init symbol
            let symbol: std::result::Result<
                libloading::Symbol<unsafe extern "C" fn(PluginInitConfig) -> PluginInitResult>,
                _,
            > = library.get(b"qimen_plugin_init");

            match symbol {
                Ok(init_fn) => {
                    let config = PluginInitConfig {
                        plugin_id: RString::from(plugin_id_owned),
                        config_json: RString::from(config_json_owned),
                        plugin_dir: RString::from(plugin_dir_owned),
                        data_dir: RString::from(data_dir_owned),
                    };

                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        init_fn(config)
                    }));

                    match result {
                        Ok(init_result) => {
                            if init_result.code != 0 {
                                Err(QimenError::Runtime(format!(
                                    "plugin '{}' init failed: {}",
                                    path_for_error, init_result.error_message
                                )))
                            } else {
                                Ok(())
                            }
                        }
                        Err(_) => Err(QimenError::Runtime(format!(
                            "plugin '{}' panicked during init",
                            path_for_error
                        ))),
                    }
                }
                Err(_) => {
                    // No init symbol — that's fine, it's optional.
                    Ok(())
                }
            }
        })
    }

    /// Call the optional `qimen_plugin_shutdown` lifecycle hook before unloading.
    pub fn call_plugin_shutdown(&mut self, library_path: &str) {
        if let Some(entry) = self.libraries.get(library_path) {
            unsafe {
                if let Ok(symbol) = entry.library.get::<unsafe extern "C" fn()>(b"qimen_plugin_shutdown") {
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        symbol();
                    }));
                }
            }
        }
    }

    /// Execute a dynamic interceptor's pre_handle callback.
    /// Returns `true` to allow, `false` to block.
    pub fn execute_pre_handle(
        &mut self,
        descriptor: &DynamicInterceptorDescriptor,
        request: InterceptorRequest,
    ) -> Result<bool> {
        let path = descriptor.library_path.clone();
        let symbol_name = descriptor.pre_handle_symbol.clone();

        if symbol_name.is_empty() {
            return Ok(true);
        }

        let path_for_error = path.clone();

        self.with_library(&path, move |library| unsafe {
            let symbol: libloading::Symbol<
                unsafe extern "C" fn(&InterceptorRequest) -> InterceptorResponse,
            > = library.get(symbol_name.as_bytes()).map_err(|err| {
                QimenError::Runtime(format!(
                    "failed to load interceptor pre_handle '{}' from '{}': {err}",
                    symbol_name, path_for_error
                ))
            })?;

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                symbol(&request)
            }));

            match result {
                Ok(response) => Ok(response.allow != 0),
                Err(panic_info) => {
                    let panic_msg = panic_info
                        .downcast_ref::<String>()
                        .map(|s| s.as_str())
                        .or_else(|| panic_info.downcast_ref::<&str>().copied())
                        .unwrap_or("unknown panic");
                    tracing::error!(
                        symbol = %symbol_name,
                        path = %path_for_error,
                        panic = %panic_msg,
                        "dynamic interceptor pre_handle panicked"
                    );
                    Err(QimenError::Runtime(format!(
                        "dynamic interceptor pre_handle '{}' panicked: {}",
                        symbol_name, panic_msg
                    )))
                }
            }
        })
    }

    /// Execute a dynamic interceptor's after_completion callback.
    pub fn execute_after_completion(
        &mut self,
        descriptor: &DynamicInterceptorDescriptor,
        request: InterceptorRequest,
    ) -> Result<()> {
        let path = descriptor.library_path.clone();
        let symbol_name = descriptor.after_completion_symbol.clone();

        if symbol_name.is_empty() {
            return Ok(());
        }

        let path_for_error = path.clone();

        self.with_library(&path, move |library| unsafe {
            let symbol: libloading::Symbol<
                unsafe extern "C" fn(&InterceptorRequest),
            > = library.get(symbol_name.as_bytes()).map_err(|err| {
                QimenError::Runtime(format!(
                    "failed to load interceptor after_completion '{}' from '{}': {err}",
                    symbol_name, path_for_error
                ))
            })?;

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                symbol(&request)
            }));

            match result {
                Ok(()) => Ok(()),
                Err(panic_info) => {
                    let panic_msg = panic_info
                        .downcast_ref::<String>()
                        .map(|s| s.as_str())
                        .or_else(|| panic_info.downcast_ref::<&str>().copied())
                        .unwrap_or("unknown panic");
                    tracing::error!(
                        symbol = %symbol_name,
                        path = %path_for_error,
                        panic = %panic_msg,
                        "dynamic interceptor after_completion panicked"
                    );
                    Err(QimenError::Runtime(format!(
                        "dynamic interceptor after_completion '{}' panicked: {}",
                        symbol_name, panic_msg
                    )))
                }
            }
        })
    }

    /// Unload a specific library by path (for hot reload).
    pub fn unload_library(&mut self, path: &str) {
        self.call_plugin_shutdown(path);
        self.libraries.remove(path);
    }

    /// Unload all libraries (for reload).
    pub fn unload_all(&mut self) {
        let paths: Vec<String> = self.libraries.keys().cloned().collect();
        for path in &paths {
            self.call_plugin_shutdown(path);
        }
        self.libraries.clear();
    }

    fn execute_route_callback(
        &mut self,
        path: &str,
        callback: &str,
        route: String,
        kind: &str,
        raw_event_json: &str,
    ) -> Result<DynamicResponse> {
        self.evict_idle_libraries(Duration::from_secs(300));
        let path_owned = path.to_string();
        let callback_owned = callback.to_string();
        let path_for_error = path_owned.clone();
        let raw_json = raw_event_json.to_string();

        self.with_library(&path_owned, move |library| unsafe {
            let symbol: libloading::Symbol<unsafe extern "C" fn(NoticeRequest) -> NoticeResponse> =
                library.get(callback_owned.as_bytes()).map_err(|err| {
                    QimenError::Runtime(format!(
                        "failed to load dynamic {} callback '{}' from '{}': {err}",
                        kind, callback_owned, path_for_error
                    ))
                })?;

            let request = NoticeRequest {
                route: RString::from(route),
                raw_event_json: RString::from(raw_json),
            };

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                symbol(request)
            }));

            match result {
                Ok(response) => Ok(map_action_response(response.action)),
                Err(panic_info) => {
                    let panic_msg = panic_info
                        .downcast_ref::<String>()
                        .map(|s| s.as_str())
                        .or_else(|| panic_info.downcast_ref::<&str>().copied())
                        .unwrap_or("unknown panic");
                    tracing::error!(
                        callback = %callback_owned,
                        path = %path_for_error,
                        panic = %panic_msg,
                        "dynamic plugin {} callback panicked",
                        kind
                    );
                    Err(QimenError::Runtime(format!(
                        "dynamic plugin {} callback '{}' panicked: {}",
                        kind, callback_owned, panic_msg
                    )))
                }
            }
        })
    }

    fn with_library<T, F>(&mut self, path: &str, operation: F) -> Result<T>
    where
        F: FnOnce(&libloading::Library) -> Result<T>,
    {
        let entry = self.ensure_library(path)?;

        if let Some(until) = entry.tripped_until {
            if Instant::now() < until {
                return Err(QimenError::Runtime(format!(
                    "dynamic plugin '{}' is temporarily isolated after repeated failures",
                    path
                )));
            }
            entry.tripped_until = None;
        }

        entry.last_used = Instant::now();

        match operation(&entry.library) {
            Ok(value) => {
                entry.failures = 0;
                entry.last_error = None;
                Ok(value)
            }
            Err(err) => {
                entry.failures += 1;
                let error_text = err.to_string();
                entry.last_error = Some(error_text.clone());
                entry.recent_errors.push_back(error_text);
                while entry.recent_errors.len() > MAX_ERROR_HISTORY {
                    entry.recent_errors.pop_front();
                }
                if entry.failures >= 3 {
                    entry.tripped_until = Some(Instant::now() + Duration::from_secs(60));
                }
                Err(err)
            }
        }
    }

    fn ensure_library(&mut self, path: &str) -> Result<&mut LoadedLibrary> {
        if !self.libraries.contains_key(path) {
            let library = unsafe {
                libloading::Library::new(path).map_err(|err| {
                    QimenError::Runtime(format!(
                        "failed to load dynamic plugin library '{}': {err}",
                        path
                    ))
                })?
            };
            self.libraries.insert(
                path.to_string(),
                LoadedLibrary {
                    library,
                    last_used: Instant::now(),
                    failures: 0,
                    tripped_until: None,
                    last_error: None,
                    recent_errors: VecDeque::new(),
                },
            );
        }

        self.libraries
            .get_mut(path)
            .ok_or_else(|| QimenError::Runtime(format!("library '{}' missing after load", path)))
    }

    fn evict_idle_libraries(&mut self, max_idle: Duration) {
        let now = Instant::now();
        self.libraries.retain(|_, entry| {
            now.duration_since(entry.last_used) <= max_idle || entry.tripped_until.is_some()
        });
    }
}

fn map_action_response(response: DynamicActionResponse) -> DynamicResponse {
    // Check for rich-media segments first (v0.2)
    let segments_json = response.segments_json.to_string();
    let has_rich_content = !segments_json.trim().is_empty();

    match response.action_kind {
        ACTION_IGNORE => DynamicResponse::Ignore,
        ACTION_APPROVE => DynamicResponse::Approve(non_empty(response.message.to_string())),
        ACTION_REJECT => DynamicResponse::Reject(non_empty(response.message.to_string())),
        ACTION_REPLY => {
            if has_rich_content {
                // Parse JSON segments into a Message
                match parse_segments_json(&segments_json) {
                    Some(message) => DynamicResponse::ReplyMessage(message),
                    None => DynamicResponse::Reply(response.message.to_string()),
                }
            } else {
                DynamicResponse::Reply(response.message.to_string())
            }
        }
        _ => DynamicResponse::Reply(response.message.to_string()),
    }
}

/// Parse a JSON array of OneBot segments into a Message.
fn parse_segments_json(json_str: &str) -> Option<Message> {
    let segments: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let array = segments.as_array()?;
    if array.is_empty() {
        return None;
    }
    // Convert to Message using the same format as OneBot11
    Some(Message::from_onebot_value(&serde_json::Value::Array(array.clone())))
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Load plugin-specific configuration from `config/plugins/<plugin_id>.toml`.
/// Returns the config as a JSON string, or an empty string if the file doesn't exist.
pub fn load_plugin_config(plugin_id: &str) -> String {
    let config_path = format!("config/plugins/{}.toml", plugin_id);
    match std::fs::read_to_string(&config_path) {
        Ok(toml_str) => {
            // Parse TOML and convert to JSON string
            match toml_str.parse::<toml::Table>() {
                Ok(table) => serde_json::to_string(&table).unwrap_or_default(),
                Err(e) => {
                    tracing::warn!(plugin_id = %plugin_id, error = %e, "failed to parse plugin config TOML");
                    String::new()
                }
            }
        }
        Err(_) => String::new(), // No config file, that's fine
    }
}

fn instant_to_epoch_ms(instant: Instant) -> u128 {
    let now_instant = Instant::now();
    let now_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);

    if instant > now_instant {
        now_epoch + instant.duration_since(now_instant).as_millis()
    } else {
        now_epoch.saturating_sub(now_instant.duration_since(instant).as_millis())
    }
}

// ─── Dynamic plugin directory scanning ──────────────────────────────────

/// Scan a directory for dynamic plugin shared libraries and return report entries.
///
/// This replicates the scanning logic from `qimen-official-host` so that the
/// runtime can re-scan plugins at reload time without depending on the host crate.
pub fn scan_dynamic_plugins(dir: &str) -> Result<Vec<DynamicPluginReportEntry>> {
    let dir_path = Path::new(dir);
    if !dir_path.exists() {
        return Ok(Vec::new());
    }

    let mut discovered = Vec::new();
    for entry in std::fs::read_dir(dir_path)? {
        let entry = entry?;
        let path = entry.path();
        if !is_dynamic_library_path(&path) {
            continue;
        }

        match load_dynamic_report_entry(&path) {
            Ok(report_entry) => discovered.push(report_entry),
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "failed to load dynamic plugin during rescan");
            }
        }
    }

    Ok(discovered)
}

fn is_dynamic_library_path(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("dll") | Some("so") | Some("dylib")
    )
}

fn load_dynamic_report_entry(path: &Path) -> Result<DynamicPluginReportEntry> {
    unsafe {
        let library = libloading::Library::new(path).map_err(|err| {
            QimenError::Module(format!(
                "failed to load library '{}': {err}",
                path.display()
            ))
        })?;

        // Try v0.2 symbol name first, then fallback to v0.1 legacy name
        let descriptor: PluginDescriptor = if let Ok(symbol) = library
            .get::<unsafe extern "C" fn() -> PluginDescriptor>(b"qimen_plugin_descriptor")
        {
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| symbol())) {
                Ok(desc) => desc,
                Err(_) => {
                    return Err(QimenError::Module(format!(
                        "plugin '{}' panicked during descriptor loading",
                        path.display()
                    )));
                }
            }
        } else {
            let symbol: libloading::Symbol<unsafe extern "C" fn() -> PluginDescriptor> = library
                .get(b"qimen_demo_plugin_descriptor")
                .map_err(|err| {
                    QimenError::Module(format!(
                        "failed to load descriptor symbol '{}': {err}",
                        path.display()
                    ))
                })?;
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| symbol())) {
                Ok(desc) => desc,
                Err(_) => {
                    return Err(QimenError::Module(format!(
                        "plugin '{}' panicked during descriptor loading",
                        path.display()
                    )));
                }
            }
        };

        if !is_compatible_api_version(descriptor.api_version.as_str()) {
            return Err(QimenError::Module(format!(
                "dynamic plugin '{}' api version '{}' is not compatible (expected 0.1 or 0.2)",
                descriptor.plugin_id, descriptor.api_version,
            )));
        }

        let is_v2 = descriptor.api_version.as_str() == "0.2";

        // Parse v0.2 multi-command entries
        let commands: Vec<DynamicCommandEntry> = if is_v2 && !descriptor.commands.is_empty() {
            descriptor
                .commands
                .iter()
                .map(|entry| {
                    let aliases = if entry.aliases.is_empty() {
                        Vec::new()
                    } else {
                        entry
                            .aliases
                            .as_str()
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect()
                    };
                    DynamicCommandEntry {
                        name: entry.name.to_string(),
                        description: entry.description.to_string(),
                        callback_symbol: entry.callback_symbol.to_string(),
                        aliases,
                        category: if entry.category.is_empty() {
                            "dynamic".to_string()
                        } else {
                            entry.category.to_string()
                        },
                        required_role: entry.required_role.to_string(),
                        scope: entry.scope.to_string(),
                    }
                })
                .collect()
        } else if !descriptor.command_name.is_empty() {
            // Legacy v0.1: single command
            vec![DynamicCommandEntry {
                name: descriptor.command_name.to_string(),
                description: descriptor.command_description.to_string(),
                callback_symbol: "qimen_demo_plugin_handle_command".to_string(),
                aliases: Vec::new(),
                category: "dynamic".to_string(),
                required_role: String::new(),
                scope: String::new(),
            }]
        } else {
            Vec::new()
        };

        // Parse v0.2 multi-route entries
        let routes: Vec<DynamicRouteEntry> = if is_v2 && !descriptor.routes.is_empty() {
            descriptor
                .routes
                .iter()
                .map(|entry| DynamicRouteEntry {
                    kind: entry.kind.to_string(),
                    route: entry.route.to_string(),
                    callback_symbol: entry.callback_symbol.to_string(),
                })
                .collect()
        } else {
            let mut routes = Vec::new();
            if !descriptor.notice_route.is_empty() {
                routes.push(DynamicRouteEntry {
                    kind: "notice".to_string(),
                    route: descriptor.notice_route.to_string(),
                    callback_symbol: "qimen_demo_plugin_handle_notice".to_string(),
                });
            }
            if !descriptor.request_route.is_empty() {
                routes.push(DynamicRouteEntry {
                    kind: "request".to_string(),
                    route: descriptor.request_route.to_string(),
                    callback_symbol: "qimen_demo_plugin_handle_notice".to_string(),
                });
            }
            if !descriptor.meta_route.is_empty() {
                routes.push(DynamicRouteEntry {
                    kind: "meta".to_string(),
                    route: descriptor.meta_route.to_string(),
                    callback_symbol: "qimen_demo_plugin_handle_notice".to_string(),
                });
            }
            routes
        };

        // Parse interceptor entries
        let interceptors: Vec<DynamicInterceptorEntry> = descriptor
            .interceptors
            .iter()
            .map(|entry| DynamicInterceptorEntry {
                pre_handle_symbol: entry.pre_handle_symbol.to_string(),
                after_completion_symbol: entry.after_completion_symbol.to_string(),
            })
            .collect();

        Ok(DynamicPluginReportEntry {
            path: path.display().to_string(),
            plugin_id: descriptor.plugin_id.to_string(),
            plugin_version: descriptor.plugin_version.to_string(),
            api_version: descriptor.api_version.to_string(),
            commands,
            routes,
            interceptors,
            // Legacy fields
            command_name: descriptor.command_name.to_string(),
            command_description: descriptor.command_description.to_string(),
            callback_symbol: "qimen_demo_plugin_handle_command".to_string(),
            notice_route: descriptor.notice_route.to_string(),
            notice_callback_symbol: "qimen_demo_plugin_handle_notice".to_string(),
            request_route: descriptor.request_route.to_string(),
            request_callback_symbol: "qimen_demo_plugin_handle_notice".to_string(),
            meta_route: descriptor.meta_route.to_string(),
            meta_callback_symbol: "qimen_demo_plugin_handle_notice".to_string(),
        })
    }
}
