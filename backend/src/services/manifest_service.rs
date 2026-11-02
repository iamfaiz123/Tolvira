use crate::models::EngineManifest;
use crate::registry::EngineRegistry;

pub struct ManifestService<'a> {
    engine_registry: &'a EngineRegistry,
}

impl<'a> ManifestService<'a> {
    pub fn new(engine_registry: &'a EngineRegistry) -> Self {
        Self { engine_registry }
    }

    pub fn get_engine_manifest(&self, engine_id: &str) -> Option<EngineManifest> {
        self.engine_registry.get(engine_id).cloned()
    }

    pub fn list_available_engines(&self) -> Vec<EngineManifest> {
        self.engine_registry.list()
    }
}

