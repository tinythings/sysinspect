use std::{path::PathBuf, sync::Arc};

use crate::cfg::mmconf::MinionConfig;

use super::{browse_types::*, browser::ModelBrowser};

/// A single model discovered by the catalog, successful or not.
#[derive(Debug)]
pub struct ModelCatalogEntry {
    pub id: String,
    pub path: PathBuf,
    pub result: Result<BrowsedModel, ModelBrowseError>,
}

/// Multi-model discovery layer.
///
/// Scans configured model roots for directories containing `model.cfg`,
/// loads each model independently, and preserves both successes and
/// failures so one broken model never hides the others.
#[derive(Debug)]
pub struct ModelCatalog {
    entries: Vec<ModelCatalogEntry>,
}

impl ModelCatalog {
    /// Scan all configured model roots and attempt to load every
    /// discovered model independently.
    pub fn scan(cfg: Arc<MinionConfig>) -> Self {
        let models_root = cfg.models_dir();
        let mut entries: Vec<ModelCatalogEntry> = Vec::new();

        if let Ok(read_dir) = std::fs::read_dir(&models_root) {
            for entry in read_dir.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                if !path.join("model.cfg").exists() {
                    continue;
                }

                let id = path.file_name().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();

                let result = ModelBrowser::load(cfg.clone(), &path).and_then(|browser| browser.summarize());

                entries.push(ModelCatalogEntry { id, path, result });
            }
        }

        // Stable order by id
        entries.sort_by(|a, b| a.id.cmp(&b.id));

        Self { entries }
    }

    /// Return all discovered entries (successes and failures).
    pub fn entries(&self) -> &[ModelCatalogEntry] {
        &self.entries
    }

    /// Return only models that loaded successfully.
    pub fn successes(&self) -> Vec<&BrowsedModel> {
        self.entries.iter().filter_map(|e| e.result.as_ref().ok()).collect()
    }

    /// Return entries whose model failed to load.
    pub fn failures(&self) -> Vec<&ModelCatalogEntry> {
        self.entries.iter().filter(|e| e.result.is_err()).collect()
    }
}
