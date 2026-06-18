import { useEffect } from "react";

export function useLayerDataset(layer: "bottom" | "overlay") {
  // Marks the document with the active native layer for layer-specific CSS.
  useEffect(() => {
    document.documentElement.dataset.layer = layer;
    document.body.dataset.layer = layer;
  }, [layer]);
}
