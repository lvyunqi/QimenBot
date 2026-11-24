use qimen_host_types::{
    BUILTIN_PLUGIN_PRIORITY, DYNAMIC_PLUGIN_PRIORITY, DynamicCommandDescriptor,
    STATIC_PLUGIN_PRIORITY,
};
use qimen_plugin_api::{
    CommandDefinition, CommandPlugin, CommandRole, CommandScope, MessageFilter,
};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

const BUILTIN_DECLARED_PRIORITY: i32 = 10;
const DYNAMIC_DESCRIPTOR_DECLARED_PRIORITY: i32 = 200;

#[derive(Debug, Clone)]
pub struct CommandRegistryDiagnostic {
    pub key: String,
    pub incoming_source: String,
    pub existing_sources: Vec<String>,
}

#[derive(Clone)]
pub struct CommandRegistryEntry {
    definition: RegistryDefinition,
    pub plugin: Option<Arc<dyn CommandPlugin>>,
    pub source_label: String,
    pub priority: u32,
}

#[derive(Clone)]
enum RegistryDefinition {
    Static(CommandDefinition),
    Dynamic {
        descriptor: DynamicCommandDescriptor,
        required_role: CommandRole,
        scope: CommandScope,
    },
}

/// A command description borrowed from a registry entry or a static plugin definition.
#[derive(Debug, Clone)]
pub struct RegisteredCommandDefinition<'a> {
    pub name: &'a str,
    pub description: &'a str,
    pub aliases: Cow<'a, [&'a str]>,
    pub examples: &'static [&'static str],
    pub category: &'a str,
    pub hidden: bool,
    pub required_role: &'a CommandRole,
    pub scope: &'a CommandScope,
    pub filter: Option<&'a MessageFilter>,
}

impl<'a> From<&'a CommandDefinition> for RegisteredCommandDefinition<'a> {
    fn from(definition: &'a CommandDefinition) -> Self {
        Self {
            name: definition.name,
            description: definition.description,
            aliases: Cow::Borrowed(definition.aliases),
            examples: definition.examples,
            category: definition.category,
            hidden: definition.hidden,
            required_role: &definition.required_role,
            scope: &definition.scope,
            filter: definition.filter.as_ref(),
        }
    }
}

impl RegisteredCommandDefinition<'_> {
    pub fn keys(&self) -> impl Iterator<Item = &str> + '_ {
        std::iter::once(self.name).chain(self.aliases.iter().copied())
    }
}

impl CommandRegistryEntry {
    pub fn name(&self) -> &str {
        match &self.definition {
            RegistryDefinition::Static(definition) => definition.name,
            RegistryDefinition::Dynamic { descriptor, .. } => &descriptor.command_name,
        }
    }

    pub fn static_definition(&self) -> Option<&CommandDefinition> {
        match &self.definition {
            RegistryDefinition::Static(definition) => Some(definition),
            RegistryDefinition::Dynamic { .. } => None,
        }
    }

    pub fn dynamic_descriptor(&self) -> Option<&DynamicCommandDescriptor> {
        match &self.definition {
            RegistryDefinition::Static(_) => None,
            RegistryDefinition::Dynamic { descriptor, .. } => Some(descriptor),
        }
    }

    pub fn role_and_scope(&self) -> (&CommandRole, &CommandScope) {
        match &self.definition {
            RegistryDefinition::Static(definition) => {
                (&definition.required_role, &definition.scope)
            }
            RegistryDefinition::Dynamic {
                required_role,
                scope,
                ..
            } => (required_role, scope),
        }
    }

    pub fn definition(&self) -> RegisteredCommandDefinition<'_> {
        match &self.definition {
            RegistryDefinition::Static(definition) => definition.into(),
            RegistryDefinition::Dynamic {
                descriptor,
                required_role,
                scope,
            } => RegisteredCommandDefinition {
                name: &descriptor.command_name,
                description: &descriptor.command_description,
                aliases: Cow::Owned(descriptor.aliases.iter().map(String::as_str).collect()),
                examples: &[],
                category: if descriptor.category.is_empty() {
                    "dynamic"
                } else {
                    &descriptor.category
                },
                hidden: false,
                required_role,
                scope,
                filter: None,
            },
        }
    }
}

#[derive(Default, Clone)]
pub struct CommandRegistry {
    entries: Vec<CommandRegistryEntry>,
    index: HashMap<String, Vec<usize>>,
    key_lengths: BTreeSet<usize>,
    diagnostics: Vec<CommandRegistryDiagnostic>,
    plugin_priorities: BTreeMap<String, u32>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_plugin_priorities(plugin_priorities: BTreeMap<String, u32>) -> Self {
        Self {
            plugin_priorities,
            ..Self::default()
        }
    }

    pub fn add_builtin(&mut self, definition: CommandDefinition) {
        self.insert_entry(CommandRegistryEntry {
            definition: RegistryDefinition::Static(definition),
            plugin: None,
            source_label: "builtin".to_string(),
            priority: BUILTIN_PLUGIN_PRIORITY,
        });
    }

    pub fn add_plugin(&mut self, plugin: Arc<dyn CommandPlugin>, definition: CommandDefinition) {
        let metadata = plugin.metadata();
        let source_label = if plugin.is_dynamic() {
            format!("dynamic-plugin:{}", metadata.id)
        } else {
            format!("static-plugin:{}", metadata.id)
        };
        let default_priority = if plugin.is_dynamic() {
            DYNAMIC_PLUGIN_PRIORITY
        } else {
            STATIC_PLUGIN_PRIORITY
        };
        self.insert_entry(CommandRegistryEntry {
            definition: RegistryDefinition::Static(definition),
            plugin: Some(plugin),
            source_label,
            priority: self
                .plugin_priorities
                .get(metadata.id)
                .copied()
                .unwrap_or(default_priority),
        });
    }

    pub fn add_dynamic_descriptor(&mut self, descriptor: DynamicCommandDescriptor) {
        let required_role = match descriptor.required_role.as_str() {
            "admin" => CommandRole::Admin,
            "owner" => CommandRole::Owner,
            _ => CommandRole::Anyone,
        };
        let scope = match descriptor.scope.as_str() {
            "group" => CommandScope::Group,
            "private" => CommandScope::Private,
            _ => CommandScope::All,
        };
        let priority = self
            .plugin_priorities
            .get(&descriptor.plugin_id)
            .copied()
            .unwrap_or(DYNAMIC_PLUGIN_PRIORITY);
        let source_label = format!("dynamic-descriptor:{}", descriptor.plugin_id);
        self.insert_entry(CommandRegistryEntry {
            definition: RegistryDefinition::Dynamic {
                descriptor,
                required_role,
                scope,
            },
            plugin: None,
            source_label,
            priority,
        });
    }

    pub fn describe(&self) -> Vec<(RegisteredCommandDefinition<'_>, String)> {
        self.entries
            .iter()
            .map(|entry| (entry.definition(), entry.source_label.clone()))
            .collect()
    }

    pub fn match_command(&self, name: &str) -> Option<&CommandRegistryEntry> {
        let positions = self.index.get(name)?;
        positions.first().and_then(|index| self.entries.get(*index))
    }

    /// 前缀匹配：当输入没有空格分隔命令名和参数时，尝试将已注册的命令名/别名
    /// 作为输入文本的前缀进行匹配。返回匹配的条目和剩余文本（参数部分）。
    ///
    /// Prefix match: when input has no whitespace between command and args,
    /// try registered command names/aliases as prefixes. Returns the matched
    /// entry and the remaining text (args portion).
    ///
    /// 优先匹配最长的命令名，避免短命令误匹配长输入。
    /// Prefers the longest matching command name to avoid short commands
    /// accidentally matching longer input.
    pub fn prefix_match_command<'a>(
        &self,
        input: &'a str,
    ) -> Option<(&CommandRegistryEntry, &'a str)> {
        // Try registered byte lengths in descending order and reuse the exact-match index.
        self.key_lengths
            .range(..input.len())
            .rev()
            .find_map(|&len| {
                let (key, rest) = input.split_at_checked(len)?;
                if rest.starts_with(char::is_whitespace) {
                    return None;
                }
                self.match_command(key).map(|entry| (entry, rest))
            })
    }

    pub fn grouped_describe(
        &self,
    ) -> BTreeMap<String, Vec<(RegisteredCommandDefinition<'_>, String)>> {
        let mut groups = BTreeMap::<_, Vec<_>>::new();
        for (definition, source) in self.describe() {
            groups
                .entry(definition.category.to_string())
                .or_default()
                .push((definition, source));
        }
        groups
    }

    pub fn diagnostics(&self) -> &[CommandRegistryDiagnostic] {
        &self.diagnostics
    }

    pub fn precedence_report(&self) -> Vec<(String, Vec<(String, u32)>)> {
        let mut report = Vec::new();
        for (key, positions) in &self.index {
            let entries = positions
                .iter()
                .filter_map(|index| self.entries.get(*index))
                .map(|entry| (entry.source_label.clone(), entry.priority))
                .collect::<Vec<_>>();
            report.push((key.clone(), entries));
        }
        report.sort_by(|a, b| a.0.cmp(&b.0));
        report
    }

    fn insert_entry(&mut self, entry: CommandRegistryEntry) {
        let position = self.entries.len();
        let keys = entry
            .definition()
            .keys()
            .map(str::to_owned)
            .collect::<Vec<_>>();

        for key in &keys {
            if let Some(indices) = self.index.get(key)
                && !indices.is_empty()
            {
                let existing_sources = indices
                    .iter()
                    .filter_map(|index| self.entries.get(*index))
                    .map(|existing| existing.source_label.clone())
                    .collect::<Vec<_>>();
                self.diagnostics.push(CommandRegistryDiagnostic {
                    key: key.clone(),
                    incoming_source: entry.source_label.clone(),
                    existing_sources,
                });
            }
        }

        self.entries.push(entry);

        for key in keys {
            self.key_lengths.insert(key.len());
            let bucket = self.index.entry(key).or_default();
            bucket.push(position);
            bucket.sort_by(|left_index, right_index| {
                let left = &self.entries[*left_index];
                let right = &self.entries[*right_index];
                right
                    .priority
                    .cmp(&left.priority)
                    .then_with(|| declared_priority(left).cmp(&declared_priority(right)))
                    .then_with(|| left.source_label.cmp(&right.source_label))
            });
        }
    }
}

fn declared_priority(entry: &CommandRegistryEntry) -> i32 {
    entry
        .plugin
        .as_ref()
        .map(|plugin| plugin.priority())
        .unwrap_or_else(|| {
            if entry.dynamic_descriptor().is_some() {
                DYNAMIC_DESCRIPTOR_DECLARED_PRIORITY
            } else {
                BUILTIN_DECLARED_PRIORITY
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use qimen_plugin_api::{CommandDefinition, CommandRole};

    fn make_definition(
        name: &'static str,
        aliases: &'static [&'static str],
        category: &'static str,
    ) -> CommandDefinition {
        CommandDefinition {
            name,
            description: "test command",
            aliases,
            examples: &[],
            category,
            hidden: false,
            required_role: CommandRole::Anyone,
            scope: CommandScope::All,
            filter: None,
        }
    }

    #[test]
    fn register_and_lookup_command() {
        let mut registry = CommandRegistry::new();
        registry.add_builtin(make_definition("ping", &[], "general"));

        let entry = registry.match_command("ping");
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.name(), "ping");
        assert_eq!(entry.source_label, "builtin");
    }

    #[test]
    fn lookup_nonexistent_command_returns_none() {
        let registry = CommandRegistry::new();
        assert!(registry.match_command("nonexistent").is_none());
    }

    #[test]
    fn alias_lookup() {
        let mut registry = CommandRegistry::new();
        registry.add_builtin(make_definition("help", &["h", "?"], "general"));

        assert!(registry.match_command("help").is_some());
        assert!(registry.match_command("h").is_some());
        assert!(registry.match_command("?").is_some());

        let by_alias = registry.match_command("h").unwrap();
        assert_eq!(by_alias.name(), "help");
    }

    #[test]
    fn priority_ordering() {
        let mut registry = CommandRegistry::new();

        // Builtin has priority 10, plugin has priority 30
        // Higher priority wins (sorted by Reverse)
        registry.add_builtin(make_definition("echo", &[], "general"));

        // Insert a higher-priority entry manually via insert_entry
        registry.insert_entry(CommandRegistryEntry {
            definition: RegistryDefinition::Static(make_definition("echo", &[], "plugin")),
            plugin: None,
            source_label: "override".to_string(),
            priority: 50,
        });

        let entry = registry.match_command("echo").unwrap();
        // The higher priority (50) entry should come first
        assert_eq!(entry.source_label, "override");
        assert_eq!(entry.priority, 50);
    }

    #[test]
    fn duplicate_registration_creates_diagnostic() {
        let mut registry = CommandRegistry::new();
        registry.add_builtin(make_definition("status", &[], "general"));
        registry.add_builtin(make_definition("status", &[], "general"));

        let diagnostics = registry.diagnostics();
        assert!(!diagnostics.is_empty());
        assert_eq!(diagnostics[0].key, "status");
        assert_eq!(diagnostics[0].incoming_source, "builtin");
        assert_eq!(diagnostics[0].existing_sources, vec!["builtin"]);
    }

    #[test]
    fn describe_returns_all_entries() {
        let mut registry = CommandRegistry::new();
        registry.add_builtin(make_definition("ping", &[], "general"));
        registry.add_builtin(make_definition("help", &["h"], "general"));

        let descriptions = registry.describe();
        assert_eq!(descriptions.len(), 2);
    }

    #[test]
    fn grouped_describe_groups_by_category() {
        let mut registry = CommandRegistry::new();
        registry.add_builtin(make_definition("ping", &[], "general"));
        registry.add_builtin(make_definition("ban", &[], "admin"));
        registry.add_builtin(make_definition("kick", &[], "admin"));

        let groups = registry.grouped_describe();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups["general"].len(), 1);
        assert_eq!(groups["admin"].len(), 2);
    }

    #[test]
    fn precedence_report_lists_all_keys() {
        let mut registry = CommandRegistry::new();
        registry.add_builtin(make_definition("test", &["t"], "general"));

        let report = registry.precedence_report();
        let keys: Vec<&str> = report.iter().map(|(k, _)| k.as_str()).collect();
        assert!(keys.contains(&"test"));
        assert!(keys.contains(&"t"));
    }

    #[test]
    fn lower_priority_entry_does_not_shadow_higher() {
        let mut registry = CommandRegistry::new();

        // Insert high priority first
        registry.insert_entry(CommandRegistryEntry {
            definition: RegistryDefinition::Static(make_definition("cmd", &[], "general")),
            plugin: None,
            source_label: "high".to_string(),
            priority: 100,
        });

        // Then low priority
        registry.add_builtin(make_definition("cmd", &[], "general"));

        let entry = registry.match_command("cmd").unwrap();
        assert_eq!(entry.source_label, "high");
    }

    #[test]
    fn configured_plugin_priority_overrides_source_defaults() {
        let mut priorities = BTreeMap::new();
        priorities.insert("example-plugin".to_string(), 99);
        let mut registry = CommandRegistry::with_plugin_priorities(priorities);
        registry.add_dynamic_descriptor(DynamicCommandDescriptor {
            plugin_id: "example-plugin".to_string(),
            command_name: "status".to_string(),
            command_description: "configured priority".to_string(),
            callback_symbol: "handle_status".to_string(),
            library_path: "example.so".to_string(),
            aliases: Vec::new(),
            category: "general".to_string(),
            required_role: String::new(),
            scope: String::new(),
        });
        registry.add_builtin(make_definition("status", &[], "general"));

        let entry = registry.match_command("status").unwrap();
        assert_eq!(entry.source_label, "dynamic-descriptor:example-plugin");
        assert_eq!(entry.priority, 99);
    }

    #[test]
    fn equal_priorities_are_deterministic_by_source_label() {
        let mut priorities = BTreeMap::new();
        priorities.insert("z-plugin".to_string(), 50);
        priorities.insert("a-plugin".to_string(), 50);
        let mut registry = CommandRegistry::with_plugin_priorities(priorities);
        for plugin_id in ["z-plugin", "a-plugin"] {
            registry.add_dynamic_descriptor(DynamicCommandDescriptor {
                plugin_id: plugin_id.to_string(),
                command_name: "same".to_string(),
                command_description: plugin_id.to_string(),
                callback_symbol: "handle".to_string(),
                library_path: format!("{plugin_id}.so"),
                aliases: Vec::new(),
                category: "general".to_string(),
                required_role: String::new(),
                scope: String::new(),
            });
        }

        assert_eq!(
            registry.match_command("same").unwrap().source_label,
            "dynamic-descriptor:a-plugin"
        );
    }

    #[test]
    fn prefix_match_chinese_command_without_space() {
        let mut registry = CommandRegistry::new();
        registry.add_builtin(make_definition("创建角色", &["新建角色"], "game"));

        // 精确匹配仍然有效
        assert!(registry.match_command("创建角色").is_some());

        // 无空格时前缀匹配: "创建角色小明-男" → command="创建角色", rest="小明-男"
        let result = registry.prefix_match_command("创建角色小明-男");
        assert!(result.is_some());
        let (entry, rest) = result.unwrap();
        assert_eq!(entry.name(), "创建角色");
        assert_eq!(rest, "小明-男");

        // 别名也能前缀匹配
        let result = registry.prefix_match_command("新建角色小红-女");
        assert!(result.is_some());
        let (entry, rest) = result.unwrap();
        assert_eq!(entry.name(), "创建角色");
        assert_eq!(rest, "小红-女");
    }

    #[test]
    fn prefix_match_prefers_longest() {
        let mut registry = CommandRegistry::new();
        registry.add_builtin(make_definition("创建", &[], "game"));
        registry.add_builtin(make_definition("创建角色", &[], "game"));

        // 应匹配最长的 "创建角色" 而非 "创建"
        let result = registry.prefix_match_command("创建角色小明-男");
        assert!(result.is_some());
        let (entry, rest) = result.unwrap();
        assert_eq!(entry.name(), "创建角色");
        assert_eq!(rest, "小明-男");
    }

    #[test]
    fn prefix_match_returns_none_for_exact() {
        let mut registry = CommandRegistry::new();
        registry.add_builtin(make_definition("创建角色", &[], "game"));

        // 完全匹配时不应触发前缀匹配（长度相等）
        assert!(registry.prefix_match_command("创建角色").is_none());
    }

    #[test]
    fn prefix_match_ignores_whitespace_separated() {
        let mut registry = CommandRegistry::new();
        registry.add_builtin(make_definition("echo", &[], "general"));

        // "echo hello" 有空格分隔，不应触发前缀匹配
        assert!(registry.prefix_match_command("echo hello").is_none());
    }
}
