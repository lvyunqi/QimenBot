use qimen_command_registry::CommandRegistry;
use qimen_config::CommandConfig;
use qimen_host_types::DynamicCommandDescriptor;
use qimen_message::Message;
use qimen_mod_command::{
    CommandTrigger, CommandTriggerPolicy, match_command_input, strip_command_name_and_args,
};
use qimen_plugin_api::{
    BuiltinCommandAction, CommandDefinition, CommandInvocation, CommandPlugin,
    CommandPluginContext, CommandPluginSignal, CommandRole, CommandScope, RuntimeBotContext,
};
use qimen_protocol_core::NormalizedEvent;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::plugin_acl::PluginAclManager;

#[derive(Debug, Clone)]
pub enum CommandDispatchSignal {
    Reply(Message),
    Builtin(BuiltinCommandAction),
    Help {
        page: usize,
    },
    DynamicCommand {
        descriptor: DynamicCommandDescriptor,
        args: Vec<String>,
    },
}

#[derive(Debug, Clone)]
pub struct ParsedCommandInput {
    pub trigger: CommandTrigger,
    pub name: String,
    pub args: Vec<String>,
    pub source_text: String,
}

pub struct CommandDispatcher {
    plugins: Vec<Arc<dyn CommandPlugin>>,
    dynamic_command_descriptors: Vec<DynamicCommandDescriptor>,
    plugin_priorities: BTreeMap<String, u32>,
    command_config: CommandConfig,
    registry: CommandRegistry,
}

impl CommandDispatcher {
    pub fn new(command_config: CommandConfig) -> Self {
        Self::with_config(BTreeMap::new(), command_config)
    }

    pub fn with_plugin_priorities(plugin_priorities: BTreeMap<String, u32>) -> Self {
        Self::with_config(plugin_priorities, CommandConfig::default())
    }

    pub fn with_config(
        plugin_priorities: BTreeMap<String, u32>,
        command_config: CommandConfig,
    ) -> Self {
        let mut dispatcher = Self {
            plugins: Vec::new(),
            dynamic_command_descriptors: Vec::new(),
            registry: CommandRegistry::with_plugin_priorities(plugin_priorities.clone()),
            plugin_priorities,
            command_config,
        };
        dispatcher.rebuild_registry();
        dispatcher
    }

    pub fn register_plugin(&mut self, plugin: Arc<dyn CommandPlugin>) {
        self.plugins.push(plugin);
        self.rebuild_registry();
    }

    pub fn set_dynamic_command_descriptors(&mut self, descriptors: Vec<DynamicCommandDescriptor>) {
        self.dynamic_command_descriptors = descriptors;
        self.rebuild_registry();
    }

    pub fn registry(&self) -> &CommandRegistry {
        &self.registry
    }

    pub fn describe_commands(&self) -> Vec<(CommandDefinition, String)> {
        let mut effective_entries = BTreeSet::new();
        let mut descriptions = Vec::new();

        for (definition, _) in self.registry.describe() {
            for key in std::iter::once(definition.name).chain(definition.aliases.iter().copied()) {
                let Some(entry) = self.registry.match_command(key) else {
                    continue;
                };
                let identity = (
                    entry.source_label.clone(),
                    entry.definition.name.to_string(),
                );
                if effective_entries.insert(identity) {
                    descriptions.push((entry.definition.clone(), entry.source_label.clone()));
                }
            }
        }

        descriptions
    }

    pub fn render_help(&self, page: usize) -> String {
        render_help_page(
            &self.describe_commands(),
            page,
            self.command_config.help_page_size,
            &self.command_config.prefixes,
        )
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

    fn rebuild_registry(&mut self) {
        let mut registry = CommandRegistry::with_plugin_priorities(self.plugin_priorities.clone());
        for definition in builtin_command_definitions(&self.command_config) {
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
        let parsed = parse_command_input(self.event, &self.dispatcher.command_config)?;

        // 先精确匹配；若失败且命令名无空格分隔参数，尝试前缀匹配
        // Exact match first; if it fails and the name has no space-separated args,
        // try prefix matching (e.g. "创建角色小明-男" → command="创建角色", args=["小明-男"])
        let (matched_entry, parsed) =
            if let Some(entry) = self.dispatcher.registry.match_command(&parsed.name) {
                (Some(entry), parsed)
            } else if let Some((entry, rest)) =
                self.dispatcher.registry.prefix_match_command(&parsed.name)
            {
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
            if !role_allowed(
                &entry.definition.required_role,
                self.is_admin,
                self.is_owner,
            ) {
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
                            tracing::info!(
                                plugin = plugin.metadata().id,
                                "command plugin produced reply"
                            );
                            return Some(CommandDispatchSignal::Reply(message));
                        }
                        CommandPluginSignal::Block(message) => {
                            tracing::info!(
                                plugin = plugin.metadata().id,
                                "command plugin blocked chain with reply"
                            );
                            return Some(CommandDispatchSignal::Reply(message));
                        }
                        CommandPluginSignal::Ignore => {
                            tracing::info!(
                                plugin = plugin.metadata().id,
                                "command plugin blocked chain silently"
                            );
                            return None;
                        }
                        CommandPluginSignal::Continue => {}
                    }
                }
            }

            if let Some(descriptor) = &entry.dynamic_descriptor {
                tracing::info!(
                    bot_id = %self.bot_id,
                    plugin = %descriptor.plugin_id,
                    command = %descriptor.command_name,
                    "matched dynamic command"
                );
                return Some(CommandDispatchSignal::DynamicCommand {
                    descriptor: descriptor.clone(),
                    args: parsed.args.clone(),
                });
            }

            if entry.source_label == "builtin" {
                return dispatch_builtin_action(entry.definition.name, &parsed.args);
            }
        }

        if self.dispatcher.command_config.help_enabled && is_help_command(&parsed.name) {
            return Some(CommandDispatchSignal::Help {
                page: parse_help_page(&parsed.args),
            });
        }

        None
    }
}

fn parse_command_input(
    event: &NormalizedEvent,
    command_config: &CommandConfig,
) -> Option<ParsedCommandInput> {
    let matched = match_command_input(
        event,
        CommandTriggerPolicy {
            prefixes: &command_config.prefixes,
            private_bare_enabled: command_config.private_bare_enabled,
            mention_enabled: command_config.mention_enabled,
            reply_enabled: command_config.reply_enabled,
        },
    )?;
    let (name, args) = strip_command_name_and_args(&matched.command_text)?;
    Some(ParsedCommandInput {
        trigger: matched.trigger,
        name: name.to_string(),
        args,
        source_text: matched.source_text,
    })
}

fn is_help_command(name: &str) -> bool {
    name.eq_ignore_ascii_case("help") || name.eq_ignore_ascii_case("h")
}

fn parse_help_page(args: &[String]) -> usize {
    args.first()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|page| *page > 0)
        .unwrap_or(1)
}

fn dispatch_builtin_action(command: &str, args: &[String]) -> Option<CommandDispatchSignal> {
    let action = match command {
        "plugins" => match args.first().map(String::as_str) {
            Some("enable") => BuiltinCommandAction::PluginsEnable {
                plugin_id: args.get(1).cloned().unwrap_or_default(),
            },
            Some("disable") => BuiltinCommandAction::PluginsDisable {
                plugin_id: args.get(1).cloned().unwrap_or_default(),
            },
            Some("reload") => BuiltinCommandAction::PluginsReload,
            _ => BuiltinCommandAction::PluginsShow,
        },
        "registry" => match args.first().map(String::as_str) {
            Some("conflicts") => BuiltinCommandAction::RegistryConflicts,
            _ => BuiltinCommandAction::RegistryReport,
        },
        "dynamic-errors" => match args.first().map(String::as_str) {
            Some("clear") => BuiltinCommandAction::DynamicErrorsClear,
            _ => BuiltinCommandAction::DynamicErrors,
        },
        _ => return None,
    };
    Some(CommandDispatchSignal::Builtin(action))
}

fn role_allowed(role: &CommandRole, is_admin: bool, is_owner: bool) -> bool {
    match role {
        CommandRole::Anyone => true,
        CommandRole::Admin => is_admin || is_owner,
        CommandRole::Owner => is_owner,
    }
}

fn builtin_command_definitions(config: &CommandConfig) -> Vec<CommandDefinition> {
    let mut definitions = Vec::new();
    if config.plugins_enabled {
        definitions.push(CommandDefinition {
            name: "plugins",
            description: "Show or manage plugin status",
            aliases: &["pl"],
            examples: &[
                "/plugins",
                "/plugins enable example-plugin",
                "/plugins disable example-plugin",
                "/plugins reload",
            ],
            category: "host-management",
            hidden: false,
            required_role: CommandRole::Admin,
            scope: CommandScope::All,
            filter: None,
        });
    }
    if config.registry_enabled {
        definitions.push(CommandDefinition {
            name: "registry",
            description: "Show command conflicts and precedence",
            aliases: &["reg"],
            examples: &["/registry", "/registry conflicts"],
            category: "host-management",
            hidden: false,
            required_role: CommandRole::Admin,
            scope: CommandScope::All,
            filter: None,
        });
    }
    if config.dynamic_errors_enabled {
        definitions.push(CommandDefinition {
            name: "dynamic-errors",
            description: "Show or clear dynamic plugin runtime errors",
            aliases: &["derr"],
            examples: &["/dynamic-errors", "/dynamic-errors clear"],
            category: "host-management",
            hidden: false,
            required_role: CommandRole::Admin,
            scope: CommandScope::All,
            filter: None,
        });
    }
    definitions
}

pub fn render_help_page(
    registry_entries: &[(CommandDefinition, String)],
    requested_page: usize,
    page_size: usize,
    prefixes: &[String],
) -> String {
    let mut entries = registry_entries
        .iter()
        .filter(|(definition, _)| !definition.hidden)
        .cloned()
        .collect::<Vec<_>>();
    entries.sort_by(
        |(left_definition, left_source), (right_definition, right_source)| {
            source_rank(left_source)
                .cmp(&source_rank(right_source))
                .then(left_source.cmp(right_source))
                .then(left_definition.category.cmp(right_definition.category))
                .then(left_definition.name.cmp(right_definition.name))
        },
    );

    let page_size = page_size.max(1);
    let total_items = entries.len();
    let total_pages = total_items.div_ceil(page_size).max(1);
    let page = requested_page.max(1).min(total_pages);
    let start = (page - 1) * page_size;
    let end = (start + page_size).min(total_items);
    let prefix = prefixes.first().map(String::as_str).unwrap_or("");

    let mut lines = vec![format!("[help {page}/{total_pages}]")];
    if total_items == 0 {
        lines.push("No commands are currently registered.".to_string());
    } else {
        lines.extend(render_command_groups(&entries[start..end], prefix));
    }
    lines.push(format!(
        "[page {page}/{total_pages} · {total_items} commands]"
    ));
    if page > 1 {
        lines.push(format!("prev: {prefix}help {}", page - 1));
    }
    if page < total_pages {
        lines.push(format!("next: {prefix}help {}", page + 1));
    }
    lines.join("\n")
}

fn render_command_groups(entries: &[(CommandDefinition, String)], prefix: &str) -> Vec<String> {
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
            let examples = match definition.examples {
                [] => "-".to_string(),
                [example] => (*example).to_string(),
                [example, rest @ ..] => format!("{example} (+{} more)", rest.len()),
            };
            let scope = match &definition.scope {
                CommandScope::Group => "group",
                CommandScope::Private => "private",
                _ => "all",
            };
            lines.push(format!(
                "  - {}{}\n    desc: {}\n    source: {} | role: {} | scope: {} | aliases: {}\n    example: {}",
                prefix,
                definition.name,
                definition.description,
                render_source(source),
                render_command_role(&definition.required_role),
                scope,
                aliases,
                examples
            ));
        }
    }

    lines
}

fn source_rank(source: &str) -> u8 {
    if source.starts_with("static-plugin:") {
        0
    } else if source.starts_with("dynamic-") {
        1
    } else {
        2
    }
}

fn render_source(source: &str) -> &str {
    source
        .strip_prefix("static-plugin:")
        .or_else(|| source.strip_prefix("dynamic-descriptor:"))
        .or_else(|| source.strip_prefix("dynamic-plugin:"))
        .unwrap_or(source)
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
    use super::{CommandDispatchSignal, CommandDispatcher, render_help_page};
    use async_trait::async_trait;
    use qimen_config::CommandConfig;
    use qimen_error::{QimenError, Result};
    use qimen_message::Message;
    use qimen_plugin_api::{
        BuiltinCommandAction, CommandDefinition, CommandInvocation, CommandPlugin,
        CommandPluginContext, CommandPluginSignal, CommandRole, CommandScope, OwnedTaskFuture,
        PluginCompatibility, PluginMetadata, RuntimeBotContext, TaskHandle,
    };
    use qimen_protocol_core::{
        ActionStatus, CapabilitySet, EventKind, NormalizedActionRequest, NormalizedActionResponse,
        NormalizedEvent, ProtocolId, TransportMode,
    };
    use serde_json::{Map, Value};
    use std::collections::BTreeMap;
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

        async fn send_action(
            &self,
            _req: NormalizedActionRequest,
        ) -> Result<NormalizedActionResponse> {
            Err(QimenError::Runtime(
                "test runtime does not send actions".to_string(),
            ))
        }

        async fn reply(
            &self,
            _event: &NormalizedEvent,
            _message: Message,
        ) -> Result<NormalizedActionResponse> {
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

    struct PluginHelp;

    #[async_trait]
    impl CommandPlugin for PluginHelp {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata {
                id: "plugin-help",
                name: "Plugin Help",
                version: "0.1.0",
                description: "Plugin-owned help command",
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
                name: "help",
                description: "Plugin help",
                aliases: &["h"],
                examples: &["/help"],
                category: "support",
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
            (invocation.definition.name == "help")
                .then(|| CommandPluginSignal::Reply(Message::text("plugin help")))
        }
    }

    struct PriorityPlugin {
        id: &'static str,
        declared_priority: i32,
    }

    #[async_trait]
    impl CommandPlugin for PriorityPlugin {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata {
                id: self.id,
                name: self.id,
                version: "0.1.0",
                description: "Priority routing test plugin",
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
                name: "shared",
                description: "Shared test command",
                aliases: &[],
                examples: &[],
                category: "tests",
                hidden: false,
                required_role: CommandRole::Anyone,
                scope: CommandScope::All,
                filter: None,
            }]
        }

        fn priority(&self) -> i32 {
            self.declared_priority
        }

        async fn on_command(
            &self,
            _ctx: &CommandPluginContext<'_>,
            _invocation: &CommandInvocation,
        ) -> Option<CommandPluginSignal> {
            Some(CommandPluginSignal::Continue)
        }
    }

    struct AliasPlugin {
        id: &'static str,
        command: &'static str,
        aliases: &'static [&'static str],
    }

    #[async_trait]
    impl CommandPlugin for AliasPlugin {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata {
                id: self.id,
                name: self.id,
                version: "0.1.0",
                description: "Alias precedence test plugin",
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
                name: self.command,
                description: "Alias precedence test command",
                aliases: self.aliases,
                examples: &[],
                category: "tests",
                hidden: false,
                required_role: CommandRole::Anyone,
                scope: CommandScope::All,
                filter: None,
            }]
        }

        async fn on_command(
            &self,
            _ctx: &CommandPluginContext<'_>,
            _invocation: &CommandInvocation,
        ) -> Option<CommandPluginSignal> {
            Some(CommandPluginSignal::Continue)
        }
    }

    #[test]
    fn admin_priority_precedes_declared_priority() {
        let mut priorities = BTreeMap::new();
        priorities.insert("admin-wins".to_string(), 51);
        priorities.insert("declared-wins".to_string(), 50);
        let mut dispatcher = CommandDispatcher::with_plugin_priorities(priorities);
        dispatcher.register_plugin(Arc::new(PriorityPlugin {
            id: "declared-wins",
            declared_priority: 0,
        }));
        dispatcher.register_plugin(Arc::new(PriorityPlugin {
            id: "admin-wins",
            declared_priority: 1_000,
        }));

        assert_eq!(
            dispatcher
                .registry()
                .match_command("shared")
                .unwrap()
                .source_label,
            "static-plugin:admin-wins"
        );
        let descriptions = dispatcher.describe_commands();
        let shared = descriptions
            .iter()
            .find(|(definition, _)| definition.name == "shared")
            .unwrap();
        assert_eq!(shared.1, "static-plugin:admin-wins");
    }

    #[test]
    fn declared_priority_breaks_equal_admin_priority_before_plugin_id() {
        let mut priorities = BTreeMap::new();
        priorities.insert("a-plugin".to_string(), 50);
        priorities.insert("z-plugin".to_string(), 50);
        let mut dispatcher = CommandDispatcher::with_plugin_priorities(priorities);
        dispatcher.register_plugin(Arc::new(PriorityPlugin {
            id: "a-plugin",
            declared_priority: 100,
        }));
        dispatcher.register_plugin(Arc::new(PriorityPlugin {
            id: "z-plugin",
            declared_priority: 10,
        }));

        assert_eq!(
            dispatcher
                .registry()
                .match_command("shared")
                .unwrap()
                .source_label,
            "static-plugin:z-plugin"
        );
    }

    #[test]
    fn help_descriptions_deduplicate_alias_winners() {
        let mut priorities = BTreeMap::new();
        priorities.insert("alias-winner".to_string(), 100);
        priorities.insert("canonical-loser".to_string(), 1);
        let mut dispatcher = CommandDispatcher::with_plugin_priorities(priorities);
        dispatcher.register_plugin(Arc::new(AliasPlugin {
            id: "alias-winner",
            command: "primary",
            aliases: &["shadowed"],
        }));
        dispatcher.register_plugin(Arc::new(AliasPlugin {
            id: "canonical-loser",
            command: "shadowed",
            aliases: &[],
        }));

        let descriptions = dispatcher.describe_commands();
        let plugin_descriptions = descriptions
            .iter()
            .filter(|(_, source)| source != "builtin")
            .collect::<Vec<_>>();
        assert_eq!(plugin_descriptions.len(), 1);
        assert_eq!(plugin_descriptions[0].0.name, "primary");
        assert_eq!(plugin_descriptions[0].1, "static-plugin:alias-winner");
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
    async fn runtime_does_not_claim_plugin_commands() {
        let dispatcher = CommandDispatcher::new(CommandConfig::default());
        for command in ["ping", "echo hello", "status"] {
            let event = sample_event(command);
            assert!(
                dispatcher
                    .dispatch("qq-main", &event, &TEST_RUNTIME)
                    .execute()
                    .await
                    .is_none(),
                "runtime unexpectedly claimed {command}"
            );
        }
    }

    #[tokio::test]
    async fn host_management_commands_are_configurable_and_require_admin() {
        let dispatcher = CommandDispatcher::new(CommandConfig::default());
        for command in ["plugins", "pl", "registry", "reg", "dynamic-errors", "derr"] {
            let event = sample_event(command);
            assert!(matches!(
                dispatcher
                    .dispatch("qq-main", &event, &TEST_RUNTIME)
                    .execute()
                    .await,
                Some(CommandDispatchSignal::Reply(message))
                    if message.plain_text().contains("permission denied")
            ));
        }

        let event = sample_event("plugins");
        assert!(matches!(
            dispatcher
                .dispatch("qq-main", &event, &TEST_RUNTIME)
                .with_roles(true, false)
                .execute()
                .await,
            Some(CommandDispatchSignal::Builtin(
                BuiltinCommandAction::PluginsShow
            ))
        ));

        let event = sample_event("registry");
        assert!(matches!(
            dispatcher
                .dispatch("qq-main", &event, &TEST_RUNTIME)
                .with_roles(false, true)
                .execute()
                .await,
            Some(CommandDispatchSignal::Builtin(
                BuiltinCommandAction::RegistryReport
            ))
        ));

        let disabled = CommandConfig {
            plugins_enabled: false,
            registry_enabled: false,
            dynamic_errors_enabled: false,
            ..CommandConfig::default()
        };
        let dispatcher = CommandDispatcher::new(disabled);
        for command in ["plugins", "registry", "dynamic-errors"] {
            let event = sample_event(command);
            assert!(
                dispatcher
                    .dispatch("qq-main", &event, &TEST_RUNTIME)
                    .with_roles(true, false)
                    .execute()
                    .await
                    .is_none(),
                "disabled host command unexpectedly claimed {command}"
            );
        }
    }

    #[test]
    fn disabled_host_command_does_not_reserve_its_name_or_alias() {
        let config = CommandConfig {
            plugins_enabled: false,
            ..CommandConfig::default()
        };
        let mut dispatcher = CommandDispatcher::new(config);
        dispatcher.register_plugin(Arc::new(AliasPlugin {
            id: "plugin-management",
            command: "plugins",
            aliases: &["pl"],
        }));

        for command in ["plugins", "pl"] {
            assert_eq!(
                dispatcher
                    .registry()
                    .match_command(command)
                    .unwrap()
                    .source_label,
                "static-plugin:plugin-management"
            );
        }
    }

    #[test]
    fn plugin_priority_can_override_host_management_commands() {
        let mut dispatcher = CommandDispatcher::new(CommandConfig::default());
        dispatcher.register_plugin(Arc::new(AliasPlugin {
            id: "plugin-management",
            command: "plugins",
            aliases: &[],
        }));

        assert_eq!(
            dispatcher
                .registry()
                .match_command("plugins")
                .unwrap()
                .source_label,
            "static-plugin:plugin-management"
        );
    }

    #[tokio::test]
    async fn command_plugin_can_be_registered() {
        let mut dispatcher = CommandDispatcher::new(CommandConfig::default());
        dispatcher.register_plugin(Arc::new(PluginEcho));

        let event = sample_event("status");
        let signal = dispatcher
            .dispatch("qq-main", &event, &TEST_RUNTIME)
            .execute()
            .await;

        match signal {
            Some(CommandDispatchSignal::Reply(message)) => {
                assert_eq!(message.plain_text(), "plugin status");
            }
            _ => panic!("expected command reply signal"),
        }
    }

    #[tokio::test]
    async fn help_is_optional_fallback_after_plugins() {
        let event = sample_event("help 2");
        let dispatcher = CommandDispatcher::new(CommandConfig::default());
        assert!(matches!(
            dispatcher
                .dispatch("qq-main", &event, &TEST_RUNTIME)
                .execute()
                .await,
            Some(CommandDispatchSignal::Help { page: 2 })
        ));

        let disabled = CommandConfig {
            help_enabled: false,
            ..CommandConfig::default()
        };
        let dispatcher = CommandDispatcher::new(disabled);
        assert!(
            dispatcher
                .dispatch("qq-main", &event, &TEST_RUNTIME)
                .execute()
                .await
                .is_none()
        );

        let mut dispatcher = CommandDispatcher::new(CommandConfig::default());
        dispatcher.register_plugin(Arc::new(PluginHelp));
        let event = sample_event("help");
        match dispatcher
            .dispatch("qq-main", &event, &TEST_RUNTIME)
            .execute()
            .await
        {
            Some(CommandDispatchSignal::Reply(message)) => {
                assert_eq!(message.plain_text(), "plugin help");
            }
            _ => panic!("plugin should own the help command"),
        }
    }

    #[test]
    fn help_output_is_paginated_without_builtin_entries() {
        let definition = |name| CommandDefinition {
            name,
            description: "Test command",
            aliases: &[],
            examples: &[],
            category: "tests",
            hidden: false,
            required_role: CommandRole::Anyone,
            scope: CommandScope::All,
            filter: None,
        };
        let entries = vec![
            (definition("alpha"), "static-plugin:sample".to_string()),
            (definition("beta"), "static-plugin:sample".to_string()),
            (
                definition("gamma"),
                "dynamic-descriptor:sample-dynamic".to_string(),
            ),
        ];

        let page = render_help_page(&entries, 2, 1, &["!".to_string()]);
        assert!(page.contains("[help 2/3]"));
        assert!(page.contains("!beta"));
        assert!(!page.contains("!alpha"));
        assert!(!page.contains("[builtin]"));
        assert!(page.contains("prev: !help 1"));
        assert!(page.contains("next: !help 3"));
    }
}
