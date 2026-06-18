import { invoke } from "@tauri-apps/api/core";

import type { DockPanelBounds } from "../../docking/types";
import { OverlayConfig } from "../types";


export async function getOverlay(id: string): Promise<OverlayConfig> {
  // Reads one overlay config from Rust's overlay.json-backed cache.
  return invoke<OverlayConfig>("get_overlay", { id });
}

export async function moveOverlay(
  id: string,
  bounds: DockPanelBounds,
  isMinimized = false,
): Promise<OverlayConfig> {
  // Moves/resizes one native overlay webview and persists its clamped bounds.
  return invoke<OverlayConfig>("move_overlay", { id, bounds, isMinimized });
}
