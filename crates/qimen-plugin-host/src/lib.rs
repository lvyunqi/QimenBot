pub mod plugin_manager;

use qimen_error::Result;
use qimen_plugin_api::{Module, PluginBundle, PluginRegistrar, PluginRegistration};

#[derive(Default)]
pub struct ModuleRegistry {
    modules: Vec<Box<dyn Module>>,
}

impl ModuleRegistry {
    pub fn register(&mut self, module: Box<dyn Module>) {
        self.modules.push(module);
    }

    pub fn modules(&self) -> &[Box<dyn Module>] {
        &self.modules
    }

    pub async fn load_all(&self) -> Result<()> {
        for module in &self.modules {
            module.on_load().await?;
        }
        Ok(())
    }

    pub fn collect_plugins(&self) -> PluginBundle {
        let mut collector = PluginCollector::default();

        for module in &self.modules {
            module.register_plugins(&mut collector);
        }

        collector.bundle
    }
}

#[derive(Default)]
struct PluginCollector {
    bundle: PluginBundle,
}

impl PluginRegistrar for PluginCollector {
    fn register(&mut self, registration: PluginRegistration) {
        let _module_id = registration.module_id;
        self.bundle.command_plugins.extend(registration.command_plugins);
        self.bundle.system_plugins.extend(registration.system_plugins);
        self.bundle.interceptors.extend(registration.interceptors);
    }
}
