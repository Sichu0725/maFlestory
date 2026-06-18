import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
} from "react";

import type { DockPanelBounds } from "../../docking/types";
import { getOverlay, moveOverlay } from "../api/overlayCommands";
import {
  DragState,
  OverlayConfig,
  OverlayFrameProps,
  ResizeEdge,
  ResizeState,
  ViewportSize,
} from "../types";
import { RESIZE_EDGES } from "../consts";

export function OverlayFrame({
  children,
  className,
  overlayId,
  title,
  initialBounds,
  onError,
}: OverlayFrameProps) {
  // Renders a reusable native overlay frame with dragging, resizing, and minimize behavior.
  const frameRef = useRef<HTMLElement>(null);
  const dragStateRef = useRef<DragState | null>(null);
  const resizeStateRef = useRef<ResizeState | null>(null);
  const expandedBoundsRef = useRef<DockPanelBounds>(initialBounds);
  const [overlayBounds, setOverlayBounds] = useState<DockPanelBounds>(initialBounds);
  const [overlayConfig, setOverlayConfig] = useState<OverlayConfig | null>(null);
  const [isMinimized, setIsMinimized] = useState(false);

  const updateOverlayBounds = useCallback(
    async (bounds: DockPanelBounds, isMinimizedBounds = false) => {
      // Applies one native overlay bounds update through Rust and stores the clamped result.
      setOverlayBounds(bounds);
      try {
        const overlay = await moveOverlay(overlayId, bounds, isMinimizedBounds);
        setOverlayConfig(overlay);
        setOverlayBounds(overlay.bounds);
      } catch (error) {
        onError?.(getErrorMessage(error));
      }
    },
    [onError, overlayId],
  );

  const handleDragStart = useCallback(
    (event: ReactPointerEvent<HTMLElement>) => {
      // Starts frame dragging from the titlebar.
      if (event.button !== 0 || !frameRef.current?.getBoundingClientRect()) {
        return;
      }

      dragStateRef.current = {
        pointerId: event.pointerId,
        startScreenX: event.screenX,
        startScreenY: event.screenY,
        startBounds: overlayBounds,
      };
      event.currentTarget.setPointerCapture(event.pointerId);
    },
    [overlayBounds],
  );

  const handleDragMove = useCallback(
    (event: ReactPointerEvent<HTMLElement>) => {
      // Moves the native overlay webview while keeping it inside the host viewport.
      const dragState = dragStateRef.current;
      if (!dragState || dragState.pointerId !== event.pointerId) {
        return;
      }

      const nextPosition = constrainPosition(
        {
          x: dragState.startBounds.x + event.screenX - dragState.startScreenX,
          y: dragState.startBounds.y + event.screenY - dragState.startScreenY,
        },
        dragState.startBounds.width,
        dragState.startBounds.height,
      );
      const nextBounds = {
        ...dragState.startBounds,
        ...nextPosition,
      };

      void updateOverlayBounds(nextBounds, isMinimized);
    },
    [isMinimized, updateOverlayBounds],
  );

  const handleDragEnd = useCallback((event: ReactPointerEvent<HTMLElement>) => {
    // Finishes frame dragging and releases pointer capture.
    if (dragStateRef.current?.pointerId !== event.pointerId) {
      return;
    }

    dragStateRef.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  }, []);

  const handleResizeStart = useCallback(
    (edge: ResizeEdge) => (event: ReactPointerEvent<HTMLSpanElement>) => {
      // Starts resizing the native overlay webview from one border or corner.
      if (event.button !== 0 || isMinimized) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();
      resizeStateRef.current = {
        pointerId: event.pointerId,
        edge,
        startScreenX: event.screenX,
        startScreenY: event.screenY,
        startBounds: overlayBounds,
      };
      event.currentTarget.setPointerCapture(event.pointerId);
    },
    [isMinimized, overlayBounds],
  );

  const handleResizeMove = useCallback(
    (event: ReactPointerEvent<HTMLSpanElement>) => {
      // Resizes the native overlay webview while respecting overlay config limits.
      const resizeState = resizeStateRef.current;
      if (isMinimized || !resizeState || resizeState.pointerId !== event.pointerId) {
        return;
      }

      event.preventDefault();
      const nextBounds = constrainResizeBounds(
        resizeState.startBounds,
        resizeState.edge,
        event.screenX - resizeState.startScreenX,
        event.screenY - resizeState.startScreenY,
        overlayConfig,
      );

      void updateOverlayBounds(nextBounds);
    },
    [isMinimized, overlayConfig, updateOverlayBounds],
  );

  const handleResizeEnd = useCallback((event: ReactPointerEvent<HTMLSpanElement>) => {
    // Finishes overlay resizing and releases pointer capture.
    if (resizeStateRef.current?.pointerId !== event.pointerId) {
      return;
    }

    resizeStateRef.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  }, []);

  const handleMinimizeToggle = useCallback(() => {
    // Toggles minimized mode, forcing minimizeSize while restoring the last expanded size.
    if (isMinimized) {
      const nextBounds = constrainBoundsToViewport({
        ...expandedBoundsRef.current,
        x: overlayBounds.x,
        y: overlayBounds.y,
      });
      setIsMinimized(false);
      void updateOverlayBounds(nextBounds, false);
      return;
    }

    expandedBoundsRef.current = overlayBounds;
    resizeStateRef.current = null;
    setIsMinimized(true);
    void updateOverlayBounds(createMinimizedBounds(overlayBounds, overlayConfig), true);
  }, [isMinimized, overlayBounds, overlayConfig, updateOverlayBounds]);

  useEffect(() => {
    void getOverlay(overlayId)
      .then((overlay) => {
        setOverlayConfig(overlay);
        setOverlayBounds(overlay.bounds);
        if (isSizeWithinLimits(overlay.bounds, overlay)) {
          expandedBoundsRef.current = overlay.bounds;
        }
      })
      .catch((error: unknown) => onError?.(getErrorMessage(error)));
  }, [onError, overlayId]);

  useEffect(() => {
    const handleResize = () => {
      // Keeps the overlay inside the host screen when available dimensions change.
      setOverlayBounds((currentBounds) => constrainBoundsToViewport(currentBounds));
    };

    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, []);

  return (
    <main>
      <aside ref={frameRef} className={className} data-minimized={isMinimized}>
        {!isMinimized
          ? RESIZE_EDGES.map((edge) => (
              <span
                aria-hidden="true"
                className={`overlay-resize overlay-resize--${edge}`}
                key={edge}
                onPointerCancel={handleResizeEnd}
                onPointerDown={handleResizeStart(edge)}
                onPointerMove={handleResizeMove}
                onPointerUp={handleResizeEnd}
              />
            ))
          : null}
        <header
          className={`${className}__titlebar`}
          onDoubleClick={handleMinimizeToggle}
          onPointerCancel={handleDragEnd}
          onPointerDown={handleDragStart}
          onPointerMove={handleDragMove}
          onPointerUp={handleDragEnd}
        >
          <span className={`${className}__drag-handle`} aria-hidden="true" />
          <h1>{title}</h1>
        </header>

        {!isMinimized ? <div className={`${className}__body`}>{children}</div> : null}
      </aside>
    </main>
  );
}

function getViewportSize(): ViewportSize {
  // Reads the host viewport size used for overlay movement.
  return {
    width: window.screen.availWidth || window.innerWidth,
    height: window.screen.availHeight || window.innerHeight,
  };
}

function constrainPosition(
  position: Pick<DockPanelBounds, "x" | "y">,
  width: number,
  height: number,
): Pick<DockPanelBounds, "x" | "y"> {
  // Clamps a floating overlay position to the host screen.
  const viewportSize = getViewportSize();
  return {
    x: clamp(position.x, 0, Math.max(0, viewportSize.width - width)),
    y: clamp(position.y, 0, Math.max(0, viewportSize.height - height)),
  };
}

function constrainResizeBounds(
  startBounds: DockPanelBounds,
  edge: ResizeEdge,
  deltaX: number,
  deltaY: number,
  overlayConfig: OverlayConfig | null,
): DockPanelBounds {
  // Resizes from one edge while preserving the opposite edge and JSON-defined limits.
  const minWidth = overlayConfig?.minSize.width ?? 1;
  const minHeight = overlayConfig?.minSize.height ?? 1;
  const maxWidth = overlayConfig?.maxSize.width ?? Number.MAX_SAFE_INTEGER;
  const maxHeight = overlayConfig?.maxSize.height ?? Number.MAX_SAFE_INTEGER;
  let nextX = startBounds.x;
  let nextY = startBounds.y;
  let nextWidth = startBounds.width;
  let nextHeight = startBounds.height;

  if (edge.includes("e")) {
    nextWidth = startBounds.width + deltaX;
  }
  if (edge.includes("s")) {
    nextHeight = startBounds.height + deltaY;
  }
  if (edge.includes("w")) {
    nextWidth = startBounds.width - deltaX;
  }
  if (edge.includes("n")) {
    nextHeight = startBounds.height - deltaY;
  }

  nextWidth = clamp(nextWidth, minWidth, maxWidth);
  nextHeight = clamp(nextHeight, minHeight, maxHeight);

  if (edge.includes("w")) {
    nextX = startBounds.x + startBounds.width - nextWidth;
  }
  if (edge.includes("n")) {
    nextY = startBounds.y + startBounds.height - nextHeight;
  }

  return constrainBoundsToViewport({
    x: nextX,
    y: nextY,
    width: nextWidth,
    height: nextHeight,
  });
}

function createMinimizedBounds(
  currentBounds: DockPanelBounds,
  overlayConfig: OverlayConfig | null,
): DockPanelBounds {
  // Creates fixed minimized bounds from overlay minimizeSize without normal min/max limits.
  const minimizeSize = overlayConfig?.minimizeSize ?? { width: 200, height: 42 };
  return constrainBoundsToViewport({
    x: currentBounds.x,
    y: currentBounds.y,
    width: minimizeSize.width,
    height: minimizeSize.height,
  });
}

function isSizeWithinLimits(bounds: DockPanelBounds, overlayConfig: OverlayConfig): boolean {
  // Checks whether bounds represent a normal expanded overlay size.
  return (
    bounds.width >= overlayConfig.minSize.width &&
    bounds.width <= overlayConfig.maxSize.width &&
    bounds.height >= overlayConfig.minSize.height &&
    bounds.height <= overlayConfig.maxSize.height
  );
}

function constrainBoundsToViewport(bounds: DockPanelBounds): DockPanelBounds {
  // Keeps resized overlay bounds inside the visible host screen area.
  const nextPosition = constrainPosition(bounds, bounds.width, bounds.height);
  return {
    ...bounds,
    ...nextPosition,
  };
}

function clamp(value: number, min: number, max: number): number {
  // Restricts a number to an inclusive range.
  return Math.min(Math.max(value, min), max);
}

function getErrorMessage(error: unknown): string {
  // Converts command errors into readable UI text.
  return error instanceof Error ? error.message : String(error);
}
