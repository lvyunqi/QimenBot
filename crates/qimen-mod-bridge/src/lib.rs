use async_trait::async_trait;
use qimen_error::Result;
use qimen_plugin_api::Module;

#[derive(Default)]
pub struct BridgeModule;

#[async_trait]
impl Module for BridgeModule {
    fn id(&self) -> &'static str {
        "bridge"
    }

    async fn on_load(&self) -> Result<()> {
        Ok(())
    }
}
