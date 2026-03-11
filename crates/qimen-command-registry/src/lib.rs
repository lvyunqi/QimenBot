use qimen_host_types::DynamicCommandDescriptor;
use qimen_plugin_api::{CommandDefinition, CommandPlugin, CommandRole};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct CommandRegistryDiagnostic {
    pub key: String,
    pub incoming_source: String,
    pub existing_sources: Vec<String>,
}

#[derive(Clone)]
pub struct CommandRegistryEntry {
    pub definition: CommandDefinition,
    pub plugin: Option<Arc<dyn CommandPlugin>>,
    pub dynamic_descriptor: Option<DynamicCommandDescriptor>,
    pub source_label: String,
    pub priority: u32,
}

#[derive(Default)]
pub struct CommandRegistry {
    entries: Vec<CommandRegistryEntry>,
    index: HashMap<String, Vec<usize>>,
    diagnostics: Vec<CommandRegistryDiagnostic>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_builtin(&mut self, definition: CommandDefinition) {
        self.insert_entry(CommandRegistryEntry {
            definition,
            plugin: None,
            dynamic_descriptor: None,
            source_label: "builtin".to_string(),
            priority: 10,
        });
    }

    pub fn add_plugin(&mut self, plugin: Arc<dyn CommandPlugin>, definition: CommandDefinition) {
        let source_label = if plugin.is_dynamic() {
            format!("dynamic-plugin:{}", plugin.metadata().id)
        } else {
            format!("static-plugin:{}", plugin.metadata().id)
        };
        self.insert_entry(CommandRegistryEntry {
            definition,
            plugin: Some(plugin),
            dynamic_descriptor: None,
            source_label,
            priority: 30,
        });
    }

    pub fn add_dynamic_descriptor(&mut self, descriptor: DynamicCommandDescriptor) {
        let role = match descriptor.required_role.as_str() {
            "admin" => CommandRole::Admin,
            "owner" => CommandRole::Owner,
            _ => CommandRole::Anyone,
        };
        let category = if descriptor.category.is_empty() {
            "dynamic"
        } else {
            Box::leak(descriptor.category.clone().into_boxed_str())
        };
        let aliases: &'static [&'static str] = if descriptor.aliases.is_empty() {
            &[]
        } else {
            let leaked: Vec<&'static str> = descriptor.aliases.iter()
                .map(|a| &*Box::leak(a.clone().into_boxed_str()))
                .collect();
            Box::leak(leaked.into_boxed_slice())
        };
        self.insert_entry(CommandRegistryEntry {
            definition: CommandDefinition {
                name: Box::leak(descriptor.command_name.clone().into_boxed_str()),
                description: Box::leak(descriptor.command_description.clone().into_boxed_str()),
                aliases,
                examples: &[],
                category,
                hidden: false,
                required_role: role,
                filter: None,
            },
            plugin: None,
            dynamic_descriptor: Some(descriptor.clone()),
            source_label: format!("dynamic-descriptor:{}", descriptor.plugin_id),
            priority: 20,
        });
    }

    pub fn describe(&self) -> Vec<(CommandDefinition, String)> {
        self.entries
            .iter()
            .map(|entry| (entry.definition.clone(), entry.source_label.clone()))
            .collect()
    }

    pub fn match_command(&self, name: &str) -> Option<&CommandRegistryEntry> {
        let positions = self.index.get(name)?;
        positions.first().and_then(|index| self.entries.get(*index))
    }

    pub fn grouped_describe(&self) -> BTreeMap<String, Vec<(CommandDefinition, String)>> {
        let mut groups: BTreeMap<String, Vec<(CommandDefinition, String)>> = BTreeMap::new();
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
        let keys = std::iter::once(entry.definition.name.to_string())
            .chain(
                entry
                    .definition
                    .aliases
                    .iter()
                    .map(|alias| alias.to_string()),
            )
            .collect::<Vec<_>>();

        for key in &keys {
            if let Some(indices) = self.index.get(key) {
                if !indices.is_empty() {
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
        }

        self.entries.push(entry.clone());

        for key in keys {
            let bucket = self.index.entry(key).or_default();
            bucket.push(position);
            bucket
                .sort_by_key(|entry_index| std::cmp::Reverse(self.entries[*entry_index].priority));
        }
    }
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
        assert_eq!(entry.definition.name, "ping");
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
        assert_eq!(by_alias.definition.name, "help");
    }

    #[test]
    fn priority_ordering() {
        let mut registry = CommandRegistry::new();

        // Builtin has priority 10, plugin has priority 30
        // Higher priority wins (sorted by Reverse)
        registry.add_builtin(make_definition("echo", &[], "general"));

        // Insert a higher-priority entry manually via insert_entry
        registry.insert_entry(CommandRegistryEntry {
            definition: make_definition("echo", &[], "plugin"),
            plugin: None,
            dynamic_descriptor: None,
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
            definition: make_definition("cmd", &[], "general"),
            plugin: None,
            dynamic_descriptor: None,
            source_label: "high".to_string(),
            priority: 100,
        });

        // Then low priority
        registry.add_builtin(make_definition("cmd", &[], "general"));

        let entry = registry.match_command("cmd").unwrap();
        assert_eq!(entry.source_label, "high");
    }
}
