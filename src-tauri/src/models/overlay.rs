use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::DockPanelBounds;

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlaySize {
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayConfig {
    pub id: String,
    pub route: String,
    pub visible: bool,
    pub bounds: DockPanelBounds,
    pub min_size: OverlaySize,
    pub max_size: OverlaySize,
    #[serde(default = "default_minimize_size")]
    pub minimize_size: OverlaySize,
}

fn default_minimize_size() -> OverlaySize {
    // Supplies a migration fallback for overlay.json files created before minimizeSize existed.
    OverlaySize {
        width: 200,
        height: 42,
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OverlayConfigFile {
    pub overlays: HashMap<String, OverlayConfig>,
}
