import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent,
} from "react";

import { useLayerDataset } from "../features/layers/hooks/useLayerDataset";
import {
  positionDockedWindow,
  syncDockedWindowBounds,
} from "../features/windows/api/windowCommands";
import type { DockedWindowInfo } from "../features/windows/types";

type CameraOffset = {
  x: number;
  y: number;
};

type PanState = {
  pointerId: number;
  startClientX: number;
  startClientY: number;
  startCameraX: number;
  startCameraY: number;
};

type ViewportSize = {
  width: number;
  height: number;
};

const VIRTUAL_CANVAS_SCALE = 10;

export function BottomPage() {
  // Renders the bottom app-stage page that sits below docked native HWND layers.
  useLayerDataset("bottom");
  const panStateRef = useRef<PanState | null>(null);
  const [viewportSize, setViewportSize] = useState<ViewportSize>(getViewportSize);
  const [cameraOffset, setCameraOffset] = useState<CameraOffset>(() =>
    getCenteredCameraOffset(getViewportSize()),
  );
  const [dockedWindows, setDockedWindows] = useState<DockedWindowInfo[]>([]);
  const [isPanningCanvas, setIsPanningCanvas] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const virtualCanvasSize = useMemo(() => getVirtualCanvasSize(viewportSize), [viewportSize]);
  const virtualCanvasStyle = useMemo(
    () => ({
      width: virtualCanvasSize.width,
      height: virtualCanvasSize.height,
      transform: `translate3d(${-cameraOffset.x}px, ${-cameraOffset.y}px, 0)`,
    }),
    [cameraOffset, virtualCanvasSize],
  );

  const handleCanvasPanStart = useCallback(
    (event: PointerEvent<HTMLElement>) => {
      // Starts panning across the larger virtual canvas from the bottom webview.
      if (event.button !== 0) {
        return;
      }

      event.preventDefault();
      const target = event.currentTarget;
      const pointerId = event.pointerId;
      const startClientX = event.clientX;
      const startClientY = event.clientY;
      const startCameraX = cameraOffset.x;
      const startCameraY = cameraOffset.y;
      event.currentTarget.setPointerCapture(event.pointerId);

      void syncDockedWindowBounds(createCameraBounds(cameraOffset, viewportSize))
        .then((syncedWindows) => {
          setDockedWindows(syncedWindows);
          panStateRef.current = {
            pointerId,
            startClientX,
            startClientY,
            startCameraX,
            startCameraY,
          };
          setIsPanningCanvas(true);
          setErrorMessage(null);
        })
        .catch((error: unknown) => {
          setErrorMessage(getErrorMessage(error));
          if (target.hasPointerCapture(pointerId)) {
            target.releasePointerCapture(pointerId);
          }
        });
    },
    [cameraOffset, viewportSize],
  );

  const handleCanvasPanMove = useCallback(
    (event: PointerEvent<HTMLElement>) => {
      // Updates the bottom webview camera while the pointer is captured.
      const panState = panStateRef.current;
      if (!panState || panState.pointerId !== event.pointerId) {
        return;
      }

      event.preventDefault();
      setCameraOffset(
        clampCameraOffset(
          {
            x: panState.startCameraX - (event.clientX - panState.startClientX),
            y: panState.startCameraY - (event.clientY - panState.startClientY),
          },
          viewportSize,
        ),
      );
    },
    [viewportSize],
  );

  const handleCanvasPanEnd = useCallback((event: PointerEvent<HTMLElement>) => {
    // Ends bottom webview panning and releases pointer capture.
    if (panStateRef.current?.pointerId !== event.pointerId) {
      return;
    }

    panStateRef.current = null;
    setIsPanningCanvas(false);
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  }, []);

  const handleCanvasAuxClick = useCallback((event: ReactMouseEvent<HTMLElement>) => {
    // Blocks the browser middle-click autoscroll gesture on the canvas.
    if (event.button === 1) {
      event.preventDefault();
    }
  }, []);

  useEffect(() => {
    const handleResize = () => {
      const nextViewportSize = getViewportSize();
      setViewportSize(nextViewportSize);
      setCameraOffset((currentCameraOffset) =>
        clampCameraOffset(currentCameraOffset, nextViewportSize),
      );
    };

    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, []);

  useEffect(() => {
    if (dockedWindows.length === 0) {
      return;
    }

    const frameId = window.requestAnimationFrame(() => {
      void Promise.all(
        dockedWindows.map((dockedWindow) =>
          positionDockedWindow(
            dockedWindow.hwnd,
            dockedWindow.virtualBounds,
            projectVirtualBounds(dockedWindow.virtualBounds, cameraOffset),
          ),
        ),
      ).catch((error: unknown) => setErrorMessage(getErrorMessage(error)));
    });

    return () => window.cancelAnimationFrame(frameId);
  }, [cameraOffset, dockedWindows]);

  return (
    <main
      className="app-stage app-stage--bottom"
      data-panning={isPanningCanvas}
      onAuxClick={handleCanvasAuxClick}
      onPointerCancel={handleCanvasPanEnd}
      onPointerDown={handleCanvasPanStart}
      onPointerMove={handleCanvasPanMove}
      onPointerUp={handleCanvasPanEnd}
    >
      <div className="virtual-canvas" style={virtualCanvasStyle} aria-hidden="true" />
      {errorMessage ? <p className="bottom-status">{errorMessage}</p> : null}
    </main>
  );
}

function getViewportSize(): ViewportSize {
  // Reads the visible bottom webview size in React CSS pixels.
  return {
    width: window.innerWidth,
    height: window.innerHeight,
  };
}

function getVirtualCanvasSize(viewportSize: ViewportSize): ViewportSize {
  // Expands the navigable background beyond the current viewport.
  return {
    width: viewportSize.width * VIRTUAL_CANVAS_SCALE,
    height: viewportSize.height * VIRTUAL_CANVAS_SCALE,
  };
}

function getCenteredCameraOffset(viewportSize: ViewportSize): CameraOffset {
  // Places the initial viewport at the center of the virtual canvas.
  const virtualCanvasSize = getVirtualCanvasSize(viewportSize);
  return {
    x: Math.max(0, (virtualCanvasSize.width - viewportSize.width) / 2),
    y: Math.max(0, (virtualCanvasSize.height - viewportSize.height) / 2),
  };
}

function createCameraBounds(cameraOffset: CameraOffset, viewportSize: ViewportSize) {
  // Packs the current viewport camera into the bounds shape used by Tauri.
  return {
    x: Math.round(cameraOffset.x),
    y: Math.round(cameraOffset.y),
    width: Math.round(viewportSize.width),
    height: Math.round(viewportSize.height),
  };
}

function projectVirtualBounds(
  virtualBounds: DockedWindowInfo["virtualBounds"],
  cameraOffset: CameraOffset,
) {
  // Converts stored virtual canvas coordinates into visible viewport coordinates.
  return {
    x: Math.round(virtualBounds.x - cameraOffset.x),
    y: Math.round(virtualBounds.y - cameraOffset.y),
    width: virtualBounds.width,
    height: virtualBounds.height,
  };
}

function clampCameraOffset(cameraOffset: CameraOffset, viewportSize: ViewportSize): CameraOffset {
  // Keeps the virtual canvas camera inside the available 10x background.
  const virtualCanvasSize = getVirtualCanvasSize(viewportSize);
  return {
    x: clamp(cameraOffset.x, 0, Math.max(0, virtualCanvasSize.width - viewportSize.width)),
    y: clamp(cameraOffset.y, 0, Math.max(0, virtualCanvasSize.height - viewportSize.height)),
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
