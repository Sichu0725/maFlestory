import { useCallback, useLayoutEffect, useState, type RefObject } from "react";

import type { DockPanelBounds } from "../types";

export function useDockPanelBounds(
  panelRef: RefObject<HTMLElement | null>,
): DockPanelBounds | null {
  // Tracks the panel rectangle in viewport CSS pixels for Tauri docking commands.
  const [bounds, setBounds] = useState<DockPanelBounds | null>(null);

  const measureBounds = useCallback(() => {
    const panel = panelRef.current;

    if (!panel) {
      setBounds(null);
      return;
    }

    const rect = panel.getBoundingClientRect();
    setBounds({
      x: Math.round(rect.left),
      y: Math.round(rect.top),
      width: Math.round(rect.width),
      height: Math.round(rect.height),
    });
  }, [panelRef]);

  useLayoutEffect(() => {
    measureBounds();

    const panel = panelRef.current;
    if (!panel) {
      return undefined;
    }

    const resizeObserver = new ResizeObserver(measureBounds);
    resizeObserver.observe(panel);
    window.addEventListener("resize", measureBounds);

    return () => {
      resizeObserver.disconnect();
      window.removeEventListener("resize", measureBounds);
    };
  }, [measureBounds, panelRef]);

  return bounds;
}
