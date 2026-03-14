use async_trait::async_trait;
use qimen_command_registry::CommandRegistry;
use qimen_host_types::{DynamicCommandDescriptor, DynamicRuntimeHealthEntry};
use qimen_message::Message;
use qimen_mod_command::{CommandTrigger, strip_command_name_and_args};
use qimen_plugin_api::{
    BuiltinCommandAction, CommandDefinition, CommandInvocation, CommandPlugin,
    CommandPluginContext, CommandPluginSignal, CommandRole, CommandScope, RuntimeBotContext,
};
use qimen_protocol_core::NormalizedEvent;
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::plugin_acl::PluginAclManager;

#[derive(Clone)]
pub struct CommandContext<'a> {
    pub bot_id: &'a str,
    #[allow(dead_code)]
    pub event: &'a NormalizedEvent,
    pub runtime: &'a dyn RuntimeBotContext,
    pub is_admin: bool,
    pub is_owner: bool,
}

impl std::fmt::Debug for CommandContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandContext")
            .field("bot_id", &self.bot_id)
            .field("is_admin", &self.is_admin)
            .field("is_owner", &self.is_owner)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone)]
pub enum CommandDispatchSignal {
    Reply(Message),
    Builtin(BuiltinCommandAction),
    DynamicCommand {
        descriptor: DynamicCommandDescriptor,
        args: Vec<String>,
    },
}

#[derive(Debug, Clone)]
pub struct PluginStatusEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub api_version: String,
    pub command_descriptions: Vec<String>,
    pub commands: Vec<String>,
    pub dynamic: bool,
    pub enabled: Option<bool>,
    pub callback_symbol: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedCommandInput {
    pub trigger: CommandTrigger,
    pub name: String,
    pub args: Vec<String>,
    pub source_text: String,
}

#[async_trait]
pub trait CommandHandler: Send + Sync {
    async fn on_command(
        &self,
        _ctx: &CommandContext<'_>,
        _parsed: &ParsedCommandInput,
    ) -> Option<CommandDispatchSignal> {
        None
    }
}

pub struct CommandDispatcher {
    handlers: Vec<Arc<dyn CommandHandler>>,
    plugins: Vec<Arc<dyn CommandPlugin>>,
    dynamic_status_entries: Vec<PluginStatusEntry>,
    dynamic_command_descriptors: Vec<DynamicCommandDescriptor>,
    registry: CommandRegistry,
}

impl CommandDispatcher {
    pub fn with_default_handlers() -> Self {
        let mut dispatcher = Self {
            handlers: vec![Arc::new(BuiltinCommandHandler)],
            plugins: Vec::new(),
            dynamic_status_entries: Vec::new(),
            dynamic_command_descriptors: Vec::new(),
            registry: CommandRegistry::new(),
        };
        dispatcher.rebuild_registry();
        dispatcher
    }

    #[allow(dead_code)]
    pub fn register_handler(&mut self, handler: Arc<dyn CommandHandler>) {
        self.handlers.push(handler);
    }

    pub fn register_plugin(&mut self, plugin: Arc<dyn CommandPlugin>) {
        self.plugins.push(plugin);
        self.rebuild_registry();
    }

    pub fn set_dynamic_status_entries(&mut self, entries: Vec<PluginStatusEntry>) {
        self.dynamic_status_entries = entries;
    }

    pub fn set_dynamic_command_descriptors(&mut self, descriptors: Vec<DynamicCommandDescriptor>) {
        self.dynamic_command_descriptors = descriptors;
        self.rebuild_registry();
    }

    pub fn registry(&self) -> &CommandRegistry {
        &self.registry
    }

    pub fn describe_commands(&self) -> Vec<(CommandDefinition, String)> {
        self.registry.describe()
    }

    pub fn plugin_status_entries(&self) -> Vec<PluginStatusEntry> {
        let mut entries: Vec<PluginStatusEntry> = self
            .plugins
            .iter()
            .map(|plugin| {
                let metadata = plugin.metadata();
                PluginStatusEntry {
                    id: metadata.id.to_string(),
                    name: metadata.name.to_string(),
                    version: metadata.version.to_string(),
                    api_version: metadata.api_version.to_string(),
                    command_descriptions: plugin
                        .commands()
                        .iter()
                        .map(|command| format!("{}: {}", command.name, command.description))
                        .collect(),
                    commands: plugin
                        .commands()
                        .into_iter()
                        .map(|command| command.name.to_string())
                        .collect(),
                    dynamic: plugin.is_dynamic(),
                    enabled: Some(true),
                    callback_symbol: None,
                }
            })
            .collect();

        entries.extend(self.dynamic_status_entries.clone());
        entries
    }

    pub fn dispatch<'a>(
        &'a self,
        bot_id: &'a str,
        event: &'a NormalizedEvent,
        runtime: &'a dyn RuntimeBotContext,
    ) -> CommandDispatch<'a> {
        CommandDispatch {
            dispatcher: self,
            bot_id,
            event,
            runtime,
            is_admin: false,
            is_owner: false,
            plugin_acl: None,
        }
    }

    pub fn merge_dynamic_health(&mut self, health: &[DynamicRuntimeHealthEntry]) {
        for entry in &mut self.dynamic_status_entries {
            if !entry.dynamic {
                continue;
            }

            if let Some(health_entry) = health.iter().find(|item| {
                entry.callback_symbol.is_some() && item.path.contains(&entry.id)
            }) {
                let health_text = format!(
                    "health: failures={} isolated_until={}",
                    health_entry.failures,
                    health_entry
                        .isolated_until_epoch_ms
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string())
                );
                entry.command_descriptions.push(health_text);
            }
        }
    }

    fn rebuild_registry(&mut self) {
        let mut registry = CommandRegistry::new();
        for definition in builtin_command_definitions() {
            registry.add_builtin(definition);
        }
        let mut sorted_plugins: Vec<_> = self.plugins.iter().collect();
        sorted_plugins.sort_by_key(|plugin| plugin.priority());
        for plugin in sorted_plugins {
            for definition in plugin.commands() {
                registry.add_plugin(plugin.clone(), definition);
            }
        }
        for descriptor in &self.dynamic_command_descriptors {
            registry.add_dynamic_descriptor(descriptor.clone());
        }
        self.registry = registry;
    }
}

pub struct CommandDispatch<'a> {
    dispatcher: &'a CommandDispatcher,
    bot_id: &'a str,
    event: &'a NormalizedEvent,
    runtime: &'a dyn RuntimeBotContext,
    is_admin: bool,
    is_owner: bool,
    plugin_acl: Option<&'a PluginAclManager>,
}

impl<'a> CommandDispatch<'a> {
    pub fn with_roles(mut self, is_admin: bool, is_owner: bool) -> Self {
        self.is_admin = is_admin;
        self.is_owner = is_owner;
        self
    }

    pub fn with_plugin_acl(mut self, acl: &'a PluginAclManager) -> Self {
        self.plugin_acl = Some(acl);
        self
    }

    pub async fn execute(&self) -> Option<CommandDispatchSignal> {
        let parsed = parse_command_input(self.event)?;
        let ctx = CommandContext {
            bot_id: self.bot_id,
            event: self.event,
            runtime: self.runtime,
            is_admin: self.is_admin,
            is_owner: self.is_owner,
        };

        if let Some(signal) = match_builtin(&parsed, self.is_admin, self.is_owner) {
            return Some(signal);
        }

        // 先精确匹配；若失败且命令名无空格分隔参数，尝试前缀匹配
        // Exact match first; if it fails and the name has no space-separated args,
        // try prefix matching (e.g. "开始穿越夜夜-男" → command="开始穿越", args=["夜夜-男"])
        let (matched_entry, parsed) = if let Some(entry) = self.dispatcher.registry.match_command(&parsed.name) {
            (Some(entry), parsed)
        } else if let Some((entry, rest)) = self.dispatcher.registry.prefix_match_command(&parsed.name) {
            let mut new_args = vec![rest.to_string()];
            new_args.extend(parsed.args);
            let new_parsed = ParsedCommandInput {
                trigger: parsed.trigger,
                name: entry.definition.name.to_string(),
                args: new_args,
                source_text: parsed.source_text,
            };
            (Some(entry), new_parsed)
        } else {
            (None, parsed)
        };

        if let Some(entry) = matched_entry {
            if !role_allowed(&entry.definition.required_role, self.is_admin, self.is_owner) {
                return Some(CommandDispatchSignal::Reply(Message::text(
                    "permission denied for this command",
                )));
            }

            match &entry.definition.scope {
                CommandScope::Group if !self.event.is_group() => return None,
                CommandScope::Private if !self.event.is_private() => return None,
                _ => {}
            }

            if let Some(filter) = &entry.definition.filter {
                let result = crate::message_filter::filter_matches(filter, self.event);
                if !result.matched {
                    return None;
                }
            }

            if let Some(plugin) = &entry.plugin {
                // Check plugin ACL before dispatching to static plugin
                if let Some(acl) = self.plugin_acl {
                    let plugin_id = plugin.metadata().id;
                    let user_id = self.event.user_id();
                    let group_id = self.event.group_id_i64();
                    if !acl.should_process(plugin_id, user_id, group_id).await {
                        tracing::debug!(plugin_id = %plugin_id, "event blocked by plugin ACL");
                        return None;
                    }
                }

                let invocation = CommandInvocation {
                    definition: entry.definition.clone(),
                    args: parsed.args.clone(),
                    source_text: parsed.source_text.clone(),
                };
                let plugin_ctx = CommandPluginContext {
                    bot_id: self.bot_id,
                    event: self.event,
                    runtime: self.runtime,
                };
                if let Some(signal) = plugin.on_command(&plugin_ctx, &invocation).await {
                    match signal {
                        CommandPluginSignal::Reply(message) => {
                            tracing::info!(plugin = plugin.metadata().id, "command plugin produced reply");
                            return Some(CommandDispatchSignal::Reply(message));
                        }
                        CommandPluginSignal::Block(message) => {
                            tracing::info!(plugin = plugin.metadata().id, "command plugin blocked chain with reply");
                            return Some(CommandDispatchSignal::Reply(message));
                        }
                        CommandPluginSignal::Ignore => {
                            tracing::info!(plugin = plugin.metadata().id, "command plugin blocked chain silently");
                            return None;
                        }
                        CommandPluginSignal::Continue => {}
                    }
                }
            }

            if let Some(descriptor) = &entry.dynamic_descriptor {
                return Some(CommandDispatchSignal::DynamicCommand {
                    descriptor: descriptor.clone(),
                    args: parsed.args.clone(),
                });
            }
        }

        for handler in &self.dispatcher.handlers {
            if let Some(signal) = handler.on_command(&ctx, &parsed).await {
                return Some(signal);
            }
        }

        None
    }
}

pub struct BuiltinCommandHandler;

#[async_trait]
impl CommandHandler for BuiltinCommandHandler {
    async fn on_command(
        &self,
        ctx: &CommandContext<'_>,
        parsed: &ParsedCommandInput,
    ) -> Option<CommandDispatchSignal> {
        tracing::info!(
            bot_id = %ctx.bot_id,
            trigger = ?parsed.trigger,
            command = %parsed.name,
            "matched builtin command"
        );

        match parsed.name.as_str() {
            "ping" => Some(CommandDispatchSignal::Reply(Message::text("pong"))),
            "echo" => Some(CommandDispatchSignal::Reply(Message::text(
                parsed.args.first().cloned().unwrap_or_default(),
            ))),
            "status" => Some(CommandDispatchSignal::Reply(Message::text(format!(
                "bot={} protocol=onebot11 transport=ws-forward status=ok",
                ctx.bot_id
            )))),
            _ => None,
        }
    }
}

fn parse_command_input(event: &NormalizedEvent) -> Option<ParsedCommandInput> {
    let message = event.message.as_ref()?;
    let text = message.plain_text();
    let trimmed = text.trim();
    let normalized = trimmed.strip_prefix('/').unwrap_or(trimmed);
    let (name, args) = strip_command_name_and_args(normalized)?;
    Some(ParsedCommandInput {
        trigger: CommandTrigger::Prefix,
        name: name.to_string(),
        args,
        source_text: text,
    })
}

fn match_builtin(
    parsed: &ParsedCommandInput,
    is_admin: bool,
    is_owner: bool,
) -> Option<CommandDispatchSignal> {
    match parsed.name.as_str() {
        "help" => Some(CommandDispatchSignal::Builtin(BuiltinCommandAction::Help)),
        "dynamic-errors" => Some(dispatch_dynamic_errors_action(&parsed.source_text)),
        "registry" => Some(dispatch_registry_action(&parsed.source_text)),
        "plugins" => {
            if !role_allowed(&CommandRole::Admin, is_admin, is_owner) {
                return Some(CommandDispatchSignal::Reply(Message::text(
                    "permission denied for this command",
                )));
            }
            Some(dispatch_plugins_action(&parsed.source_text))
        }
        _ => None,
    }
}

fn dispatch_plugins_action(source_text: &str) -> CommandDispatchSignal {
    let normalized = source_text.trim().trim_start_matches('/').trim();
    let mut parts = normalized.split_whitespace();
    let _root = parts.next();
    match parts.next() {
        Some("enable") => {
            let plugin_id = parts.next().unwrap_or_default().to_string();
            CommandDispatchSignal::Builtin(BuiltinCommandAction::PluginsEnable { plugin_id })
        }
        Some("disable") => {
            let plugin_id = parts.next().unwrap_or_default().to_string();
            CommandDispatchSignal::Builtin(BuiltinCommandAction::PluginsDisable { plugin_id })
        }
        Some("reload") => CommandDispatchSignal::Builtin(BuiltinCommandAction::PluginsReload),
        _ => CommandDispatchSignal::Builtin(BuiltinCommandAction::PluginsShow),
    }
}

fn dispatch_registry_action(source_text: &str) -> CommandDispatchSignal {
    let normalized = source_text.trim().trim_start_matches('/').trim();
    let mut parts = normalized.split_whitespace();
    let _root = parts.next();
    match parts.next() {
        Some("conflicts") => CommandDispatchSignal::Builtin(BuiltinCommandAction::RegistryConflicts),
        _ => CommandDispatchSignal::Builtin(BuiltinCommandAction::RegistryReport),
    }
}

fn dispatch_dynamic_errors_action(source_text: &str) -> CommandDispatchSignal {
    let normalized = source_text.trim().trim_start_matches('/').trim();
    let mut parts = normalized.split_whitespace();
    let _root = parts.next();
    match parts.next() {
        Some("clear") => CommandDispatchSignal::Builtin(BuiltinCommandAction::DynamicErrorsClear),
        _ => CommandDispatchSignal::Builtin(BuiltinCommandAction::DynamicErrors),
    }
}

fn role_allowed(role: &CommandRole, is_admin: bool, is_owner: bool) -> bool {
    match role {
        CommandRole::Anyone => true,
        CommandRole::Admin => is_admin || is_owner,
        CommandRole::Owner => is_owner,
    }
}

fn builtin_command_definitions() -> Vec<CommandDefinition> {
    vec![
        CommandDefinition {
            name: "ping",
            description: "Check whether the bot is alive",
            aliases: &["p"],
            examples: &["/ping", "ping"],
            category: "general",
            hidden: false,
            required_role: CommandRole::Anyone,
            scope: CommandScope::All,
            filter: None,
        },
        CommandDefinition {
            name: "echo",
            description: "Echo text back to the sender",
            aliases: &["e"],
            examples: &["/echo hello", "echo hello"],
            category: "general",
            hidden: false,
            required_role: CommandRole::Anyone,
            scope: CommandScope::All,
            filter: None,
        },
        CommandDefinition {
            name: "status",
            description: "Show runtime status",
            aliases: &["st"],
            examples: &["/status", "status"],
            category: "system",
            hidden: false,
            required_role: CommandRole::Anyone,
            scope: CommandScope::All,
            filter: None,
        },
        CommandDefinition {
            name: "help",
            description: "Show command help",
            aliases: &["h"],
            examples: &["/help", "help"],
            category: "system",
            hidden: false,
            required_role: CommandRole::Anyone,
            scope: CommandScope::All,
            filter: None,
        },
        CommandDefinition {
            name: "plugins",
            description: "Show or manage plugin status",
            aliases: &["pl"],
            examples: &[
                "/plugins",
                "/plugins show",
                "/plugins enable example-plugin",
                "/plugins disable example-plugin",
                "/plugins reload",
            ],
            category: "system",
            hidden: false,
            required_role: CommandRole::Admin,
            scope: CommandScope::All,
            filter: None,
        },
        CommandDefinition {
            name: "registry",
            description: "Show command registry diagnostics and precedence",
            aliases: &["reg"],
            examples: &["/registry", "/registry conflicts"],
            category: "system",
            hidden: false,
            required_role: CommandRole::Admin,
            scope: CommandScope::All,
            filter: None,
        },
        CommandDefinition {
            name: "dynamic-errors",
            description: "Show dynamic runtime errors and circuit-breaker state",
            aliases: &["derr"],
            examples: &["/dynamic-errors", "/dynamic-errors clear"],
            category: "system",
            hidden: false,
            required_role: CommandRole::Admin,
            scope: CommandScope::All,
            filter: None,
        },
    ]
}

pub fn render_help_text(registry_entries: &[(CommandDefinition, String)]) -> String {
    let mut builtin = Vec::new();
    let mut static_plugins = Vec::new();
    let mut dynamic_descriptors = Vec::new();

    for (definition, source) in registry_entries {
        if source == "builtin" {
            builtin.push((definition.clone(), source.clone()));
        } else if source.starts_with("static-plugin:") {
            static_plugins.push((definition.clone(), source.clone()));
        } else if source.starts_with("dynamic-descriptor:") || source.starts_with("dynamic-plugin:") {
            dynamic_descriptors.push((definition.clone(), source.clone()));
        }
    }

    let mut lines = vec!["[help]".to_string()];
    lines.push("[builtin]".to_string());
    lines.extend(render_command_groups(&builtin));
    lines.push("[static plugins]".to_string());
    lines.extend(render_command_groups(&static_plugins));
    lines.push("[dynamic descriptors]".to_string());
    lines.extend(render_command_groups(&dynamic_descriptors));
    lines.join("\n")
}

fn render_command_groups(entries: &[(CommandDefinition, String)]) -> Vec<String> {
    let mut grouped = BTreeMap::<String, Vec<(&CommandDefinition, &String)>>::new();
    for (definition, source) in entries.iter().filter(|(definition, _)| !definition.hidden) {
        grouped
            .entry(definition.category.to_string())
            .or_default()
            .push((definition, source));
    }

    let mut lines = Vec::new();
    for (category, definitions) in grouped {
        lines.push(format!("  <{}>", category));
        for (definition, source) in definitions {
            let aliases = if definition.aliases.is_empty() {
                "-".to_string()
            } else {
                definition.aliases.join(",")
            };
            let examples = if definition.examples.is_empty() {
                "-".to_string()
            } else {
                definition.examples.join(" | ")
            };
            let scope_line = match &definition.scope {
                CommandScope::Group => "\n    scope: group",
                CommandScope::Private => "\n    scope: private",
                _ => "",
            };
            lines.push(format!(
                "  - /{}\n    desc: {}\n    source: {}\n    aliases: {}\n    role: {}{}\n    examples: {}",
                definition.name,
                definition.description,
                source,
                aliases,
                render_command_role(&definition.required_role),
                scope_line,
                examples
            ));
        }
    }

    lines
}

fn render_command_role(role: &CommandRole) -> &'static str {
    match role {
        CommandRole::Anyone => "anyone",
        CommandRole::Admin => "admin",
        CommandRole::Owner => "owner",
    }
}

#[cfg(test)]
mod tests {
    use super::{CommandDispatchSignal, CommandDispatcher, CommandHandler, CommandContext};
    use async_trait::async_trait;
    use qimen_error::{QimenError, Result};
    use qimen_message::Message;
    use qimen_plugin_api::{
        CommandDefinition, CommandInvocation, CommandPlugin, CommandPluginContext,
        CommandPluginSignal, CommandRole, CommandScope, OwnedTaskFuture, PluginCompatibility,
        PluginMetadata, RuntimeBotContext, TaskHandle,
    };
    use qimen_protocol_core::{
        ActionStatus, CapabilitySet, EventKind, NormalizedActionRequest, NormalizedActionResponse,
        NormalizedEvent, ProtocolId, TransportMode,
    };
    use serde_json::{Map, Value};
    use std::sync::Arc;

    struct TestRuntimeBotContext;

    #[async_trait]
    impl RuntimeBotContext for TestRuntimeBotContext {
        fn bot_instance(&self) -> &str {
            "qq-main"
        }

        fn protocol(&self) -> ProtocolId {
            ProtocolId::OneBot11
        }

        fn capabilities(&self) -> &CapabilitySet {
            static CAPABILITIES: std::sync::OnceLock<CapabilitySet> = std::sync::OnceLock::new();
            CAPABILITIES.get_or_init(CapabilitySet::default)
        }

        async fn send_action(&self, _req: NormalizedActionRequest) -> Result<NormalizedActionResponse> {
            Err(QimenError::Runtime("test runtime does not send actions".to_string()))
        }

        async fn reply(&self, _event: &NormalizedEvent, _message: Message) -> Result<NormalizedActionResponse> {
            Ok(NormalizedActionResponse {
                protocol: ProtocolId::OneBot11,
                bot_instance: "qq-main".to_string(),
                status: ActionStatus::Ok,
                retcode: 0,
                data: Value::Null,
                echo: None,
                latency_ms: 0,
                raw_json: serde_json::json!({
                    "status": "ok",
                    "retcode": 0,
                    "data": null
                }),
            })
        }

        fn spawn_owned(&self, name: &str, _fut: OwnedTaskFuture) -> TaskHandle {
            TaskHandle {
                name: name.to_string(),
            }
        }
    }

    static TEST_RUNTIME: TestRuntimeBotContext = TestRuntimeBotContext;

    struct CustomHandler;

    #[async_trait]
    impl CommandHandler for CustomHandler {
        async fn on_command(
            &self,
            _ctx: &CommandContext<'_>,
            parsed: &super::ParsedCommandInput,
        ) -> Option<CommandDispatchSignal> {
            if parsed.name == "ping" {
                Some(CommandDispatchSignal::Reply(Message::text("custom pong")))
            } else {
                None
            }
        }
    }

    struct PluginEcho;

    #[async_trait]
    impl CommandPlugin for PluginEcho {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata {
                id: "plugin-echo",
                name: "Plugin Echo",
                version: "0.1.0",
                description: "Example command plugin",
                api_version: "0.1",
                compatibility: PluginCompatibility {
                    host_api: "0.1",
                    framework_min: "0.1.0",
                    framework_max: "0.1.x",
                },
            }
        }

        fn commands(&self) -> Vec<CommandDefinition> {
            vec![CommandDefinition {
                name: "status",
                description: "Status from plugin",
                aliases: &["st"],
                examples: &["/status"],
                category: "examples",
                hidden: false,
                required_role: CommandRole::Anyone,
                scope: CommandScope::All,
                filter: None,
            }]
        }

        async fn on_command(
            &self,
            _ctx: &CommandPluginContext<'_>,
            invocation: &CommandInvocation,
        ) -> Option<CommandPluginSignal> {
            if invocation.definition.name == "status" {
                Some(CommandPluginSignal::Reply(Message::text("plugin status")))
            } else {
                Some(CommandPluginSignal::Continue)
            }
        }
    }

    fn sample_event(text: &str) -> NormalizedEvent {
        NormalizedEvent {
            protocol: ProtocolId::OneBot11,
            bot_instance: "qq-main".to_string(),
            transport_mode: TransportMode::WsForward,
            time: Some(1),
            kind: EventKind::Message,
            message: Some(Message::text(text)),
            actor: None,
            chat: Some(qimen_protocol_core::ChatRef {
                id: "10001".to_string(),
                kind: "private".to_string(),
            }),
            raw_json: serde_json::json!({
                "self_id": 123456,
                "post_type": "message",
                "message_type": "private",
                "user_id": 10001,
                "message": text,
            }),
            raw_bytes: None,
            extensions: Map::new(),
        }
    }

    #[tokio::test]
    async fn custom_command_handler_can_be_registered() {
        let mut dispatcher = CommandDispatcher::with_default_handlers();
        dispatcher.register_handler(Arc::new(CustomHandler));

        let event = sample_event("ping");
        let signal = dispatcher.dispatch("qq-main", &event, &TEST_RUNTIME).execute().await;

        match signal {
            Some(CommandDispatchSignal::Reply(message)) => {
                let text = message.to_onebot_value();
                assert!(matches!(text, Value::String(_)));
            }
            _ => panic!("expected command reply signal"),
        }
    }

    #[tokio::test]
    async fn command_plugin_can_be_registered() {
        let mut dispatcher = CommandDispatcher::with_default_handlers();
        dispatcher.register_plugin(Arc::new(PluginEcho));

        let event = sample_event("status");
        let signal = dispatcher.dispatch("qq-main", &event, &TEST_RUNTIME).execute().await;

        match signal {
            Some(CommandDispatchSignal::Reply(message)) => {
                assert_eq!(message.plain_text(), "plugin status");
            }
            _ => panic!("expected command reply signal"),
        }
    }
}
