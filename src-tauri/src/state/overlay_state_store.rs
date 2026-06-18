use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::models::{DockPanelBounds, OverlayConfig, OverlayConfigFile, OverlaySize};

const DEFAULT_OVERLAY_JSON: &str = include_str!("../../default_overlay.json");

pub struct OverlayStateStore {
    pub path: PathBuf,
    pub overlays: Mutex<HashMap<String, OverlayConfig>>,
}

impl OverlayStateStore {
    pub fn load(path: PathBuf) -> Result<Self, String> {
        // Loads overlay.json from local app data and seeds it from default_overlay.json on first run.
        let config = if path.exists() {
            let default_config = default_overlay_config()?;
            let content = fs::read_to_string(&path)
                .map_err(|error| format!("Failed to read overlay.json: {error}"))?;
            let saved_config = serde_json::from_str::<OverlayConfigFile>(&content)
                .map_err(|error| format!("Failed to parse overlay.json: {error}"))?;
            let config = merge_overlay_config(default_config, saved_config);
            write_config_file(&path, &config)?;
            config
        } else {
            let config = default_overlay_config()?;
            write_config_file(&path, &config)?;
            config
        };

        Ok(Self {
            path,
            overlays: Mutex::new(config.overlays),
        })
    }

    pub fn overlay(&self, id: &str) -> Result<OverlayConfig, String> {
        // Reads one overlay config snapshot from the in-memory cache.
        let overlays = self
            .overlays
            .lock()
            .map_err(|_| "Overlay state lock is poisoned.".to_string())?;
        overlays
            .get(id)
            .cloned()
            .ok_or_else(|| format!("Unknown overlay: {id}"))
    }

    pub fn visible_overlays(&self) -> Result<Vec<OverlayConfig>, String> {
        // Returns overlay configs that should be shown on startup.
        let overlays = self
            .overlays
            .lock()
            .map_err(|_| "Overlay state lock is poisoned.".to_string())?;
        Ok(overlays
            .values()
            .filter(|overlay| overlay.visible)
            .cloned()
            .collect())
    }

    pub fn move_overlay(
        &self,
        id: &str,
        bounds: DockPanelBounds,
        is_minimized: bool,
    ) -> Result<OverlayConfig, String> {
        // Updates overlay bounds, using either normal limits or fixed minimized size.
        let config = {
            let mut overlays = self
                .overlays
                .lock()
                .map_err(|_| "Overlay state lock is poisoned.".to_string())?;
            let overlay = overlays
                .get_mut(id)
                .ok_or_else(|| format!("Unknown overlay: {id}"))?;

            overlay.bounds = if is_minimized {
                minimize_bounds(bounds, overlay.minimize_size)
            } else {
                clamp_bounds(bounds, overlay.min_size, overlay.max_size)
            };
            overlay.visible = true;
            overlay.clone()
        };

        self.save()?;
        Ok(config)
    }

    fn save(&self) -> Result<(), String> {
        // Persists the current overlay cache to overlay.json.
        let overlays = self
            .overlays
            .lock()
            .map_err(|_| "Overlay state lock is poisoned.".to_string())?
            .clone();
        write_config_file(&self.path, &OverlayConfigFile { overlays })
    }
}

fn default_overlay_config() -> Result<OverlayConfigFile, String> {
    // Parses the checked-in default overlay list used to seed overlay.json.
    serde_json::from_str::<OverlayConfigFile>(DEFAULT_OVERLAY_JSON)
        .map_err(|error| format!("Failed to parse default_overlay.json: {error}"))
}

fn merge_overlay_config(
    mut default_config: OverlayConfigFile,
    saved_config: OverlayConfigFile,
) -> OverlayConfigFile {
    // Preserves user bounds/visibility while keeping default overlay definitions authoritative.
    for (id, default_overlay) in default_config.overlays.iter_mut() {
        if let Some(saved_overlay) = saved_config.overlays.get(id) {
            default_overlay.visible = saved_overlay.visible;
            if is_size_within_limits(
                saved_overlay.bounds,
                default_overlay.min_size,
                default_overlay.max_size,
            ) {
                default_overlay.bounds = saved_overlay.bounds;
            }
        }
    }

    default_config
}

fn is_size_within_limits(
    bounds: DockPanelBounds,
    min_size: OverlaySize,
    max_size: OverlaySize,
) -> bool {
    // Checks whether saved user bounds can be trusted under the current overlay limits.
    bounds.width >= min_size.width
        && bounds.width <= max_size.width
        && bounds.height >= min_size.height
        && bounds.height <= max_size.height
}

fn clamp_bounds(
    bounds: DockPanelBounds,
    min_size: OverlaySize,
    max_size: OverlaySize,
) -> DockPanelBounds {
    // Applies per-overlay min and max size constraints to a proposed bounds update.
    DockPanelBounds {
        x: bounds.x,
        y: bounds.y,
        width: bounds.width.clamp(min_size.width, max_size.width),
        height: bounds.height.clamp(min_size.height, max_size.height),
    }
}

fn minimize_bounds(bounds: DockPanelBounds, minimize_size: OverlaySize) -> DockPanelBounds {
    // Forces minimized overlays to their configured fixed size.
    DockPanelBounds {
        x: bounds.x,
        y: bounds.y,
        width: minimize_size.width,
        height: minimize_size.height,
    }
}

fn write_config_file(path: &PathBuf, config: &OverlayConfigFile) -> Result<(), String> {
    // Writes overlay config JSON, creating the app data directory when needed.
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create overlay config directory: {error}"))?;
    }

    let content = serde_json::to_string_pretty(config)
        .map_err(|error| format!("Failed to serialize overlay.json: {error}"))?;
    fs::write(path, content).map_err(|error| format!("Failed to write overlay.json: {error}"))
}
