import { useEffect, useRef } from "react";

import { useDockPanelBounds } from "../hooks/useDockPanelBounds";
import type { DockPanelBounds } from "../types";

type DockPanelProps = {
  dockedWindowTitle: string | null;
  onBoundsChange: (bounds: DockPanelBounds | null) => void;
};

export function DockPanel({ dockedWindowTitle, onBoundsChange }: DockPanelProps) {
  // Measures and displays the native docking target area.
  const panelRef = useRef<HTMLDivElement>(null);
  const bounds = useDockPanelBounds(panelRef);

  useEffect(() => {
    onBoundsChange(bounds);
  }, [bounds, onBoundsChange]);

  return (
    <div className="dock-panel" ref={panelRef}>
      <div className="dock-panel__surface">
        {dockedWindowTitle ? `Docked: ${dockedWindowTitle}` : "Dock target"}
      </div>
    </div>
  );
}
