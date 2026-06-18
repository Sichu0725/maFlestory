import { useCallback, useEffect, useMemo, useState } from "react";

import { DockButton } from "../features/docking/components/DockButton";
import type { DockPanelBounds } from "../features/docking/types";
import { useLayerDataset } from "../features/layers/hooks/useLayerDataset";
import { OverlayFrame } from "../features/overlays/components/OverlayFrame";
import {
  dockWindow,
  listDockedWindows,
  listWindows,
  undockWindow,
} from "../features/windows/api/windowCommands";
import { WindowList } from "../features/windows/components/WindowList";
import type { DockedWindowInfo, WindowInfo } from "../features/windows/types";

type ManagerPosition = {
  x: number;
  y: number;
};

type ViewportSize = {
  width: number;
  height: number;
};

const FALLBACK_MANAGER_WIDTH = 380;
const FALLBACK_MANAGER_HEIGHT = 520;
const VIRTUAL_CANVAS_SCALE = 10;
const DOCK_MANAGER_OVERLAY_ID = "dock-manager";
const INITIAL_OVERLAY_BOUNDS: DockPanelBounds = {
  x: 24,
  y: 24,
  width: FALLBACK_MANAGER_WIDTH,
  height: FALLBACK_MANAGER_HEIGHT,
};

export function DockManagerPage() {
  // Renders the overlay page that sits above docked native HWND layers.
  useLayerDataset("overlay");

  return <DockManagerLayer />;
}

function DockManagerLayer() {
  // Connects the floating React dock manager to the Tauri window docking commands.
  const [viewportSize, setViewportSize] = useState<ViewportSize>(getViewportSize);
  const [cameraOffset, setCameraOffset] = useState<ManagerPosition>(() =>
    getCenteredCameraOffset(getViewportSize()),
  );
  const [selectedWindowId, setSelectedWindowId] = useState<number | null>(null);
  const [dockedWindows, setDockedWindows] = useState<DockedWindowInfo[]>([]);
  const [windows, setWindows] = useState<WindowInfo[]>([]);
  const [isLoadingWindows, setIsLoadingWindows] = useState(false);
  const [isCommandRunning, setIsCommandRunning] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const selectedWindow = useMemo(
    () => windows.find((window) => window.hwnd === selectedWindowId) ?? null,
    [selectedWindowId, windows],
  );

  const refreshWindowState = useCallback(async () => {
    // Refreshes both visible window candidates and already docked HWND state.
    setIsLoadingWindows(true);
    setErrorMessage(null);

    try {
      const [windowsList, dockedWindows] = await Promise.all([listWindows(), listDockedWindows()]);
      setWindows(windowsList);
      setDockedWindows(dockedWindows);
      setSelectedWindowId((currentWindowId) =>
        currentWindowId !== null && !windowsList.some((window) => window.hwnd === currentWindowId)
          ? null
          : currentWindowId,
      );
    } catch (error) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsLoadingWindows(false);
    }
  }, []);

  const handleOnDock = useCallback(async () => {
    // Docks the selected HWND while preserving its current screen rectangle.
    if (selectedWindowId === null) {
      return;
    }

    setIsCommandRunning(true);
    setErrorMessage(null);

    try {
      await dockWindow(selectedWindowId, createCameraBounds(cameraOffset, viewportSize));
      await refreshWindowState();
    } catch (error) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsCommandRunning(false);
    }
  }, [cameraOffset, refreshWindowState, selectedWindowId, viewportSize]);

  const handleUndockWindow = useCallback(
    async (hwnd: number) => {
      // Restores one docked HWND to its original parent and style.
      setIsCommandRunning(true);
      setErrorMessage(null);

      try {
        await undockWindow(hwnd);
        setDockedWindows((currentWindows) =>
          currentWindows.filter((window) => window.hwnd !== hwnd),
        );
        await refreshWindowState();
      } catch (error) {
        setErrorMessage(getErrorMessage(error));
      } finally {
        setIsCommandRunning(false);
      }
    },
    [refreshWindowState],
  );

  useEffect(() => {
    void refreshWindowState();
  }, [refreshWindowState]);

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

  return (
    <OverlayFrame
      className="dock-manager"
      initialBounds={INITIAL_OVERLAY_BOUNDS}
      overlayId={DOCK_MANAGER_OVERLAY_ID}
      title="Dock Manager"
      onError={setErrorMessage}
    >
      <section className="window-sidebar">
        <header className="section-header">
          <h2>Windows</h2>
          <button
            type="button"
            className="text-button"
            disabled={isLoadingWindows}
            onClick={refreshWindowState}
          >
            Refresh
          </button>
        </header>

        {dockedWindows.length > 0 ? (
          <section className="docked-status" aria-label="Docked windows">
            <span className="docked-status__label">Docked ({dockedWindows.length})</span>
            <div className="docked-status__list">
              {dockedWindows.map((window) => (
                <article className="docked-status__item" key={window.hwnd}>
                  <div className="docked-status__content">
                    <strong className="docked-status__title">{window.title}</strong>
                    <span className="docked-status__meta">
                      {window.className ?? "Unknown class"} - HWND {window.hwnd}
                    </span>
                  </div>
                  <button
                    type="button"
                    className="docked-status__undock"
                    disabled={isCommandRunning}
                    onClick={() => void handleUndockWindow(window.hwnd)}
                  >
                    Undock
                  </button>
                </article>
              ))}
            </div>
          </section>
        ) : null}

        <WindowList
          selectedWindowId={selectedWindowId}
          windows={windows}
          onSelectWindow={setSelectedWindowId}
        />
      </section>

      <footer className="toolbar" aria-label="Dock controls">
        <DockButton
          disabled={selectedWindowId === null || isCommandRunning}
          onDock={handleOnDock}
        />
        <p className="toolbar-status">
          {errorMessage ??
            (isLoadingWindows
              ? "Loading windows"
              : dockedWindows.length > 0
                ? `Docked: ${dockedWindows.length}`
                : selectedWindow
                  ? `Selected: ${selectedWindow.title}`
                  : "Select a window")}
        </p>
      </footer>
    </OverlayFrame>
  );
}

function getViewportSize(): ViewportSize {
  // Reads the host viewport size used for overlay movement and dock placement.
  return {
    width: window.screen.availWidth || window.innerWidth,
    height: window.screen.availHeight || window.innerHeight,
  };
}

function getVirtualCanvasSize(viewportSize: ViewportSize): ViewportSize {
  // Expands the navigable background beyond the current viewport.
  return {
    width: viewportSize.width * VIRTUAL_CANVAS_SCALE,
    height: viewportSize.height * VIRTUAL_CANVAS_SCALE,
  };
}

function getCenteredCameraOffset(viewportSize: ViewportSize): ManagerPosition {
  // Places the initial viewport at the center of the virtual canvas.
  const virtualCanvasSize = getVirtualCanvasSize(viewportSize);
  return {
    x: Math.max(0, (virtualCanvasSize.width - viewportSize.width) / 2),
    y: Math.max(0, (virtualCanvasSize.height - viewportSize.height) / 2),
  };
}

function createCameraBounds(
  cameraOffset: ManagerPosition,
  viewportSize: ViewportSize,
): DockPanelBounds {
  // Packs the current viewport camera into the bounds shape used by Tauri.
  return {
    x: Math.round(cameraOffset.x),
    y: Math.round(cameraOffset.y),
    width: Math.round(viewportSize.width),
    height: Math.round(viewportSize.height),
  };
}

function clampCameraOffset(
  cameraOffset: ManagerPosition,
  viewportSize: ViewportSize,
): ManagerPosition {
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
