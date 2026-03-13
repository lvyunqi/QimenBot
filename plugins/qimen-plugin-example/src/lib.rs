// ── QimenBot 示例插件 / QimenBot Example Plugin ──
//
// 全面展示 QimenBot 框架的主要功能，包括：
// Comprehensive showcase of QimenBot framework features, including:
//
//   - basic.rs:            基础命令 / Basic commands (ping, echo, whoami, ban, stop)
//   - message_demo.rs:     消息构建与提取 / Message building & extraction
//   - event_demo.rs:       系统事件处理 / System event handling (notice, request, meta)
//   - interceptor_demo.rs: 拦截器 / Interceptors (logging, cooldown)

mod basic;
mod event_demo;
mod interceptor_demo;
mod message_demo;

pub use basic::BasicModule;
pub use event_demo::EventDemoModule;
pub use interceptor_demo::{CooldownInterceptor, LoggingInterceptor};
pub use message_demo::MessageDemoModule;

#[cfg(test)]
mod tests {
    use qimen_plugin_api::Module;

    // ── BasicModule tests ────────────────────────────────────────────────

    #[test]
    fn basic_module_exposes_commands() {
        let module = super::BasicModule;
        let cmd_plugins = module.command_plugins();
        assert_eq!(cmd_plugins.len(), 1);

        let commands = cmd_plugins[0].commands();
        // ping, echo, whoami, group-info, ban, group-only, private-only, stop
        assert_eq!(commands.len(), 8);
        assert_eq!(commands[0].name, "ping");
        assert_eq!(commands[1].name, "echo");
        assert_eq!(commands[2].name, "whoami");
        assert_eq!(commands[3].name, "group-info");
        assert_eq!(commands[4].name, "ban");
        assert_eq!(commands[5].name, "group-only");
        assert_eq!(commands[6].name, "private-only");
        assert_eq!(commands[7].name, "stop");
    }

    // ── MessageDemoModule tests ──────────────────────────────────────────

    #[test]
    fn message_demo_exposes_commands() {
        let module = super::MessageDemoModule;
        let cmd_plugins = module.command_plugins();
        assert_eq!(cmd_plugins.len(), 1);

        let commands = cmd_plugins[0].commands();
        // rich, parse, card, reply-quote, keyboard
        assert_eq!(commands.len(), 5);
        assert_eq!(commands[0].name, "rich");
        assert_eq!(commands[1].name, "parse");
        assert_eq!(commands[2].name, "card");
        assert_eq!(commands[3].name, "reply-quote");
        assert_eq!(commands[4].name, "keyboard");
    }

    // ── EventDemoModule tests ────────────────────────────────────────────

    #[test]
    fn event_demo_has_system_plugins() {
        let module = super::EventDemoModule;
        let sys_plugins = module.system_plugins();
        assert_eq!(sys_plugins.len(), 1);
    }

    #[test]
    fn event_demo_has_no_command_plugins() {
        let module = super::EventDemoModule;
        let cmd_plugins = module.command_plugins();
        assert_eq!(cmd_plugins.len(), 0);
    }
}
