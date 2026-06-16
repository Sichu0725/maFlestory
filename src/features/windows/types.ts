import type { DockPanelBounds } from "../docking/types";

export type WindowInfo = {
  hwnd: number;
  title: string;
  className?: string;
  processId?: number;
};

export type DockedWindowInfo = WindowInfo & {
  virtualBounds: DockPanelBounds;
};
