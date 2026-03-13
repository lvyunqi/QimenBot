//! QimenBot 动态插件模板 / Dynamic Plugin Template
//!
//! 将 `{{name}}` 替换为你的插件名称，然后开始开发！
//! Replace `{{name}}` with your plugin name and start developing!
//!
//! ## 编译 / Build
//!
//! ```bash
//! cargo build --release
//! ```
//!
//! ## 安装 / Install
//!
//! 将编译产物复制到 `plugins/bin/` 目录：
//! Copy the build artifact to the `plugins/bin/` directory:
//!
//! ```bash
//! cp target/release/libqimen_dynamic_plugin_{{name}}.so ../../plugins/bin/
//! ```

use abi_stable::std_types::RString;
use abi_stable_host_api::{
    CommandDescriptorEntry, CommandRequest, CommandResponse, DynamicActionResponse,
    PluginDescriptor,
};

// ─── Plugin Descriptor 插件描述符 ────────────────────────────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn qimen_plugin_descriptor() -> PluginDescriptor {
    PluginDescriptor::new("{{name}}", "0.1.0")
        .add_command_full(CommandDescriptorEntry {
            name: RString::from("hello"),
            description: RString::from("Say hello / 打招呼"),
            callback_symbol: RString::from("{{name}}_hello"),
            aliases: RString::from("hi"),
            category: RString::from("{{name}}"),
            required_role: RString::new(),
        })
    // 添加更多命令 / Add more commands:
    // .add_command_full(CommandDescriptorEntry { ... })
}

// ─── Command Callbacks 命令回调 ──────────────────────────────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn {{name}}_hello(req: &CommandRequest) -> CommandResponse {
    let sender = req.sender_id.as_str();
    let nickname = req.sender_nickname.as_str();
    let display = if nickname.is_empty() { sender } else { nickname };

    CommandResponse::text(&format!("Hello, {}! 你好！", display))
}

// ─── Optional Lifecycle Hooks 可选生命周期钩子 ───────────────────────────────
//
// 取消注释以启用 init/shutdown 钩子：
// Uncomment to enable init/shutdown hooks:
//
// use abi_stable_host_api::{PluginInitConfig, PluginInitResult};
//
// #[unsafe(no_mangle)]
// pub unsafe extern "C" fn qimen_plugin_init(config: PluginInitConfig) -> PluginInitResult {
//     // 在这里初始化你的插件（数据库连接、配置加载等）
//     // Initialize your plugin here (database connections, config loading, etc.)
//     let _config_json = config.config_json.as_str();
//     PluginInitResult::ok()
// }
//
// #[unsafe(no_mangle)]
// pub unsafe extern "C" fn qimen_plugin_shutdown() {
//     // 在这里清理资源
//     // Clean up resources here
// }
