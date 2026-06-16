import { invoke } from "@tauri-apps/api/core";

import type { DockPanelBounds } from "../../docking/types";
import type { DockedWindowInfo, WindowInfo } from "../types";

export async function listWindows(): Promise<WindowInfo[]> {
  // Requests the visible top-level Windows window list from Tauri.
  return invoke<WindowInfo[]>("list_windows");
}

export async function listDockedWindows(): Promise<DockedWindowInfo[]> {
  // Requests window metadata currently tracked in the Rust dock state.
  return invoke<DockedWindowInfo[]>("list_docked_windows");
}

export async function dockWindow(hwnd: number, cameraBounds: DockPanelBounds): Promise<void> {
  // Reparents the selected window into the no-activate Tauri container.
  return invoke("dock_window", { hwnd, cameraBounds });
}

export async function undockWindow(hwnd: number): Promise<void> {
  // Restores the selected window to its pre-docking parent and styles.
  return invoke("undock_window", { hwnd });
}

export async function resizeDockedWindow(hwnd: number, bounds: DockPanelBounds): Promise<void> {
  // Moves and resizes a docked child window without activation.
  return invoke("resize_docked_window", { hwnd, bounds });
}

export async function positionDockedWindow(
  hwnd: number,
  virtualBounds: DockPanelBounds,
  viewportBounds: DockPanelBounds,
): Promise<void> {
  // Projects a docked child from virtual canvas space into the visible viewport.
  return invoke("position_docked_window", { hwnd, virtualBounds, viewportBounds });
}

export async function syncDockedWindowBounds(
  cameraBounds: DockPanelBounds,
): Promise<DockedWindowInfo[]> {
  // Reads native docked HWND rectangles and stores them as virtual canvas bounds.
  return invoke<DockedWindowInfo[]>("sync_docked_window_bounds", { cameraBounds });
}
