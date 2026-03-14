use abi_stable_host_api::{PluginDescriptor, is_compatible_api_version};
use qimen_config::AppConfig;
use qimen_error::{QimenError, Result};
use qimen_framework::Runtime;
use qimen_host_types::{
    DynamicCommandEntry, DynamicInterceptorEntry, DynamicPluginReportEntry, DynamicRouteEntry,
    HostPluginReport, PluginState, load_plugin_state,
};
use qimen_mod_admin::AdminModule;
use qimen_mod_bridge::BridgeModule;
use qimen_mod_command::CommandModule;
use qimen_mod_scheduler::SchedulerModule;
use qimen_observability::init;
use qimen_plugin_api::Module;
use qimen_plugin_host::ModuleRegistry;
use std::fs;
use std::path::Path;

const HOST_PLUGIN_API_VERSION: &str = "0.1";
const HOST_FRAMEWORK_VERSION: &str = "0.1.1";

pub async fn run_official_host(config_path: &str) -> Result<()> {
    // First-start: auto-copy config template if config file is missing
    if !Path::new(config_path).exists() {
        let example_path = format!("{}.example", config_path);
        if Path::new(&example_path).exists() {
            fs::copy(&example_path, config_path).map_err(|e| {
                QimenError::Config(format!(
                    "failed to copy '{}' to '{}': {}",
                    example_path, config_path, e
                ))
            })?;
            eprintln!(
                "[QimenBot] Configuration file not found. \
                 Copied '{}' -> '{}'.\n\
                 Please edit '{}' with your settings and restart.",
                example_path, config_path, config_path
            );
            std::process::exit(0);
        } else {
            eprintln!(
                "[QimenBot] Configuration file '{}' not found.\n\
                 Please create it (you can copy from '{}.example' if available) and restart.",
                config_path, config_path
            );
            std::process::exit(1);
        }
    }

    let config = AppConfig::load_from_path(config_path)?;
    init(&config.observability.level, config.observability.json_logs)?;

    tracing::info!("starting qimen official host");

    let plugin_state = load_plugin_state(&config.official_host.plugin_state_path)?;
    let dynamic_descriptors = scan_dynamic_plugin_descriptors(&config.official_host.plugin_bin_dir)?;
    let dynamic_descriptors: Vec<_> = dynamic_descriptors
        .into_iter()
        .filter(|d| plugin_state.is_enabled(&d.plugin_id))
        .collect();

    let mut modules = ModuleRegistry::default();
    register_builtin_modules(&config, &mut modules);
    register_plugin_modules(&config, &plugin_state, &mut modules)?;
    let report = build_host_plugin_report(&config, &plugin_state, &dynamic_descriptors);
    print_host_startup_report(&report);
    modules.load_all().await?;

    let plugins = modules.collect_plugins();

    let runtime = Runtime::from_config_with_plugins(&config, plugins).with_host_plugin_report(report);
    runtime.boot().await?;
    tracing::info!(bots = runtime.bots().len(), "official host booted");
    Ok(())
}

fn register_builtin_modules(config: &AppConfig, modules: &mut ModuleRegistry) {
    for module in &config.official_host.builtin_modules {
        match module.as_str() {
            "command" => modules.register(Box::new(CommandModule)),
            "admin" => modules.register(Box::new(AdminModule::default())),
            "scheduler" => modules.register(Box::new(SchedulerModule::default())),
            "bridge" => modules.register(Box::new(BridgeModule)),
            other => {
                tracing::warn!(module = %other, "unknown builtin module configured, skipping")
            }
        }
    }
}

fn register_plugin_modules(
    config: &AppConfig,
    plugin_state: &PluginState,
    modules: &mut ModuleRegistry,
) -> Result<()> {
    let inventory_map: std::collections::HashMap<&str, &qimen_plugin_api::ModuleEntry> =
        inventory::iter::<qimen_plugin_api::ModuleEntry>
            .into_iter()
            .map(|e| (e.id, e))
            .collect();

    tracing::info!(
        count = inventory_map.len(),
        modules = %inventory_map.keys().copied().collect::<Vec<_>>().join(", "),
        "inventory plugin modules discovered"
    );

    for id in &config.official_host.plugin_modules {
        if !plugin_state.is_enabled(id) {
            tracing::info!(module = %id, "plugin disabled, skipping");
            continue;
        }
        match inventory_map.get(id.as_str()) {
            Some(entry) => {
                let module = (entry.factory)();
                validate_module_compatibility(module.as_ref())?;
                modules.register(module);
            }
            None => tracing::warn!(module = %id, "plugin not found in inventory, skipping"),
        }
    }
    Ok(())
}

fn validate_module_compatibility(module: &dyn Module) -> Result<()> {
    for plugin in module.command_plugins() {
        validate_plugin_metadata(&plugin.metadata())?;
    }

    for plugin in module.system_plugins() {
        validate_plugin_metadata(&plugin.metadata())?;
    }

    Ok(())
}

fn validate_plugin_metadata(metadata: &qimen_plugin_api::PluginMetadata) -> Result<()> {
    if metadata.api_version != HOST_PLUGIN_API_VERSION {
        return Err(QimenError::Module(format!(
            "plugin '{}' declares api_version='{}' but host expects '{}'",
            metadata.id, metadata.api_version, HOST_PLUGIN_API_VERSION
        )));
    }

    if metadata.id.trim().is_empty() {
        return Err(QimenError::Module(
            "plugin metadata id cannot be empty".to_string(),
        ));
    }

    tracing::info!(
        plugin = metadata.id,
        plugin_version = metadata.version,
        plugin_api = metadata.api_version,
        host_api = HOST_PLUGIN_API_VERSION,
        framework_min = metadata.compatibility.framework_min,
        framework_max = metadata.compatibility.framework_max,
        host_framework = HOST_FRAMEWORK_VERSION,
        "plugin compatibility report"
    );

    Ok(())
}

fn build_host_plugin_report(
    config: &AppConfig,
    plugin_state: &PluginState,
    dynamic_descriptors: &[DynamicPluginDescriptor],
) -> HostPluginReport {
    HostPluginReport {
        builtin_modules: config.official_host.builtin_modules.clone(),
        configured_plugins: config.official_host.plugin_modules.clone(),
        persisted_states: plugin_state.modules().clone(),
        dynamic_plugins: dynamic_descriptors
            .iter()
            .map(|descriptor| DynamicPluginReportEntry {
                path: descriptor.path.clone(),
                plugin_id: descriptor.plugin_id.clone(),
                plugin_version: descriptor.plugin_version.clone(),
                api_version: descriptor.api_version.clone(),
                commands: descriptor.commands.clone(),
                routes: descriptor.routes.clone(),
                interceptors: descriptor.interceptors.clone(),
                // Legacy fields
                command_name: descriptor.command_name.clone(),
                command_description: descriptor.command_description.clone(),
                callback_symbol: descriptor.callback_symbol.clone(),
                notice_route: descriptor.notice_route.clone(),
                notice_callback_symbol: descriptor.notice_callback_symbol.clone(),
                request_route: descriptor.request_route.clone(),
                request_callback_symbol: descriptor.request_callback_symbol.clone(),
                meta_route: descriptor.meta_route.clone(),
                meta_callback_symbol: descriptor.meta_callback_symbol.clone(),
            })
            .collect(),
    }
}

fn print_host_startup_report(report: &HostPluginReport) {
    tracing::info!(
        builtin_modules = %report.builtin_modules.join(","),
        plugin_modules = %report.configured_plugins.join(","),
        "official host startup report"
    );

    for (module, enabled) in &report.persisted_states {
        tracing::info!(module = %module, enabled = *enabled, "persisted plugin state");
    }

    for descriptor in &report.dynamic_plugins {
        let command_names: Vec<&str> = descriptor.commands.iter().map(|c| c.name.as_str()).collect();
        let route_names: Vec<String> = descriptor.routes.iter().map(|r| format!("{}:{}", r.kind, r.route)).collect();
        tracing::info!(
            path = %descriptor.path,
            plugin = %descriptor.plugin_id,
            version = %descriptor.plugin_version,
            api = %descriptor.api_version,
            commands = %command_names.join(","),
            routes = %route_names.join(","),
            legacy_command = %descriptor.command_name,
            "dynamic plugin descriptor discovered"
        );
    }
}

#[derive(Debug, Clone)]
struct DynamicPluginDescriptor {
    path: String,
    plugin_id: String,
    plugin_version: String,
    api_version: String,
    /// v0.2 multi-command entries.
    commands: Vec<DynamicCommandEntry>,
    /// v0.2 multi-route entries.
    routes: Vec<DynamicRouteEntry>,
    /// Interceptor entries.
    interceptors: Vec<DynamicInterceptorEntry>,
    // Legacy v0.1 fields
    command_name: String,
    command_description: String,
    callback_symbol: String,
    notice_route: String,
    notice_callback_symbol: String,
    request_route: String,
    request_callback_symbol: String,
    meta_route: String,
    meta_callback_symbol: String,
}

fn scan_dynamic_plugin_descriptors(dir: &str) -> Result<Vec<DynamicPluginDescriptor>> {
    if !Path::new(dir).exists() {
        return Ok(Vec::new());
    }

    let mut discovered = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !is_dynamic_library_path(&path) {
            continue;
        }

        match load_dynamic_descriptor(&path) {
            Ok(descriptor) => discovered.push(descriptor),
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "failed to load dynamic plugin");
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

fn load_dynamic_descriptor(path: &Path) -> Result<DynamicPluginDescriptor> {
    unsafe {
        let library = libloading::Library::new(path)
            .map_err(|err| QimenError::Module(format!("failed to load library '{}': {err}", path.display())))?;

        // Try v0.2 symbol name first, then fallback to v0.1 legacy name
        let descriptor: PluginDescriptor = if let Ok(symbol) = library
            .get::<unsafe extern "C" fn() -> PluginDescriptor>(b"qimen_plugin_descriptor")
        {
            symbol()
        } else {
            let symbol: libloading::Symbol<unsafe extern "C" fn() -> PluginDescriptor> = library
                .get(b"qimen_demo_plugin_descriptor")
                .map_err(|err| QimenError::Module(format!(
                    "failed to load descriptor symbol '{}': {err}",
                    path.display()
                )))?;
            symbol()
        };

        if !is_compatible_api_version(descriptor.api_version.as_str()) {
            return Err(QimenError::Module(format!(
                "dynamic plugin '{}' api version '{}' is not compatible (expected 0.1, 0.2 or 0.3)",
                descriptor.plugin_id,
                descriptor.api_version,
            )));
        }

        let is_v2_plus = descriptor.api_version.as_str() == "0.2"
            || descriptor.api_version.as_str() == "0.3";

        // Parse v0.2+ multi-command entries
        let commands: Vec<DynamicCommandEntry> = if is_v2_plus && !descriptor.commands.is_empty() {
            descriptor.commands.iter().map(|entry| {
                let aliases = if entry.aliases.is_empty() {
                    Vec::new()
                } else {
                    entry.aliases.as_str().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
                };
                DynamicCommandEntry {
                    name: entry.name.to_string(),
                    description: entry.description.to_string(),
                    callback_symbol: entry.callback_symbol.to_string(),
                    aliases,
                    category: if entry.category.is_empty() { "dynamic".to_string() } else { entry.category.to_string() },
                    required_role: entry.required_role.to_string(),
                    scope: entry.scope.to_string(),
                }
            }).collect()
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

        // Parse v0.2+ multi-route entries
        let routes: Vec<DynamicRouteEntry> = if is_v2_plus && !descriptor.routes.is_empty() {
            descriptor.routes.iter().map(|entry| DynamicRouteEntry {
                kind: entry.kind.to_string(),
                route: entry.route.to_string(),
                callback_symbol: entry.callback_symbol.to_string(),
            }).collect()
        } else {
            // Legacy v0.1: build routes from individual fields
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

        Ok(DynamicPluginDescriptor {
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
