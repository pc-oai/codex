use codex_protocol::openai_models::ModelPreset;
use std::convert::Infallible;
use std::sync::RwLock;

#[derive(Debug)]
pub(crate) struct ModelCatalog {
    models: RwLock<Vec<ModelPreset>>,
}

impl ModelCatalog {
    pub(crate) fn new(models: Vec<ModelPreset>) -> Self {
        Self {
            models: RwLock::new(models),
        }
    }

    pub(crate) fn try_list_models(&self) -> Result<Vec<ModelPreset>, Infallible> {
        Ok(self
            .models
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone())
    }

    pub(crate) fn replace_models(&self, models: Vec<ModelPreset>) {
        *self
            .models
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = models;
    }
}
