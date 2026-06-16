import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
} from "react";

import { DockButton } from "./features/docking/components/DockButton";
import type { DockPanelBounds } from "./features/docking/types";
import {
  dockWindow,
  listDockedWindows,
  listWindows,
  positionDockedWindow,
  syncDockedWindowBounds,
  undockWindow,
} from "./features/windows/api/windowCommands";
import { WindowList } from "./features/windows/components/WindowList";
import type { DockedWindowInfo, WindowInfo } from "./features/windows/types";

type ManagerPosition = {
  x: number;
  y: number;
};

type DragState = {
  pointerId: number;
  offsetX: number;
  offsetY: number;
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

const INITIAL_MANAGER_POSITION: ManagerPosition = { x: 24, y: 24 };
const FALLBACK_MANAGER_WIDTH = 380;
const FALLBACK_MANAGER_HEIGHT = 520;
const VIRTUAL_CANVAS_SCALE = 10;

export function App() {
  // Connects the floating React dock manager to the Tauri window docking commands.
  const managerRef = useRef<HTMLElement>(null);
  const dragStateRef = useRef<DragState | null>(null);
  const panStateRef = useRef<PanState | null>(null);
  const [managerPosition, setManagerPosition] = useState(INITIAL_MANAGER_POSITION);
  const [viewportSize, setViewportSize] = useState<ViewportSize>(getViewportSize);
  const [cameraOffset, setCameraOffset] = useState<ManagerPosition>(() =>
    getCenteredCameraOffset(getViewportSize()),
  );
  const [isPanningCanvas, setIsPanningCanvas] = useState(false);
  const [isMinimized, setIsMinimized] = useState(false);
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

  const virtualCanvasSize = useMemo(() => getVirtualCanvasSize(viewportSize), [viewportSize]);
  const virtualCanvasStyle = useMemo(
    () => ({
      width: virtualCanvasSize.width,
      height: virtualCanvasSize.height,
      transform: `translate3d(${-cameraOffset.x}px, ${-cameraOffset.y}px, 0)`,
    }),
    [cameraOffset, virtualCanvasSize],
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

  const handleDragStart = useCallback((event: ReactPointerEvent<HTMLElement>) => {
    // Starts panel dragging from the titlebar handle without moving docked native windows.
    if (event.button !== 0) {
      return;
    }

    const rect = managerRef.current?.getBoundingClientRect();
    if (!rect) {
      return;
    }

    dragStateRef.current = {
      pointerId: event.pointerId,
      offsetX: event.clientX - rect.left,
      offsetY: event.clientY - rect.top,
    };
    event.currentTarget.setPointerCapture(event.pointerId);
  }, []);

  const handleDragMove = useCallback((event: ReactPointerEvent<HTMLElement>) => {
    // Moves the floating manager while keeping it inside the Tauri viewport.
    const dragState = dragStateRef.current;
    if (!dragState || dragState.pointerId !== event.pointerId) {
      return;
    }

    const rect = managerRef.current?.getBoundingClientRect();
    setManagerPosition(
      constrainPosition(
        {
          x: event.clientX - dragState.offsetX,
          y: event.clientY - dragState.offsetY,
        },
        rect?.width ?? FALLBACK_MANAGER_WIDTH,
        rect?.height ?? FALLBACK_MANAGER_HEIGHT,
      ),
    );
  }, []);

  const handleDragEnd = useCallback((event: ReactPointerEvent<HTMLElement>) => {
    // Finishes a pointer drag and releases the captured pointer.
    if (dragStateRef.current?.pointerId !== event.pointerId) {
      return;
    }

    dragStateRef.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  }, []);

  const handleCanvasPanStart = useCallback(
    (event: ReactPointerEvent<HTMLElement>) => {
      // Starts middle-button panning across the larger virtual canvas.
      if (event.button !== 0 || isEventInsideDockManager(event)) {
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
    (event: ReactPointerEvent<HTMLElement>) => {
      // Updates the viewport camera while the middle mouse button is held.
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

  const handleCanvasPanEnd = useCallback((event: ReactPointerEvent<HTMLElement>) => {
    // Ends middle-button panning and releases pointer capture.
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
    void refreshWindowState();
  }, [refreshWindowState]);

  useEffect(() => {
    const handleResize = () => {
      const nextViewportSize = getViewportSize();
      const rect = managerRef.current?.getBoundingClientRect();
      setViewportSize(nextViewportSize);
      setCameraOffset((currentCameraOffset) =>
        clampCameraOffset(currentCameraOffset, nextViewportSize),
      );
      setManagerPosition((currentPosition) =>
        constrainPosition(
          currentPosition,
          rect?.width ?? FALLBACK_MANAGER_WIDTH,
          rect?.height ?? FALLBACK_MANAGER_HEIGHT,
        ),
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
  }, [cameraOffset, dockedWindows, viewportSize]);

  return (
    <main
      className="app-stage"
      data-panning={isPanningCanvas}
      onAuxClick={handleCanvasAuxClick}
      onPointerCancel={handleCanvasPanEnd}
      onPointerDown={handleCanvasPanStart}
      onPointerMove={handleCanvasPanMove}
      onPointerUp={handleCanvasPanEnd}
    >
      <div className="virtual-canvas" style={virtualCanvasStyle} aria-hidden="true" />
      <aside
        ref={managerRef}
        className="dock-manager"
        data-minimized={isMinimized}
        style={{
          transform: `translate3d(${managerPosition.x}px, ${managerPosition.y}px, 0)`,
        }}
      >
        <header
          className="dock-manager__titlebar"
          onDoubleClick={() => setIsMinimized((currentValue) => !currentValue)}
          onPointerDown={handleDragStart}
          onPointerMove={handleDragMove}
          onPointerUp={handleDragEnd}
          onPointerCancel={handleDragEnd}
        >
          <span className="dock-manager__drag-handle" aria-hidden="true" />
          <h1>maFlestory Dock</h1>
          {!isMinimized ? (
            <button
              type="button"
              className="dock-manager__minimize"
              aria-label="Minimize dock manager"
              onClick={() => setIsMinimized(true)}
            >
              _
            </button>
          ) : null}
        </header>

        {!isMinimized ? (
          <div className="dock-manager__body">
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
          </div>
        ) : null}
      </aside>
    </main>
  );
}

function getViewportSize(): ViewportSize {
  // Reads the visible Tauri webview size in React CSS pixels.
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

function projectVirtualBounds(
  virtualBounds: DockPanelBounds,
  cameraOffset: ManagerPosition,
): DockPanelBounds {
  // Converts stored virtual canvas coordinates into visible viewport coordinates.
  return {
    x: Math.round(virtualBounds.x - cameraOffset.x),
    y: Math.round(virtualBounds.y - cameraOffset.y),
    width: virtualBounds.width,
    height: virtualBounds.height,
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

function constrainPosition(
  position: ManagerPosition,
  width: number,
  height: number,
): ManagerPosition {
  // Clamps a floating panel position to the current browser viewport.
  return {
    x: clamp(position.x, 0, Math.max(0, window.innerWidth - width)),
    y: clamp(position.y, 0, Math.max(0, window.innerHeight - height)),
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

function isEventInsideDockManager(event: ReactPointerEvent<HTMLElement>): boolean {
  // Checks whether a pointer event started from the floating manager UI.
  return event.target instanceof HTMLElement && event.target.closest(".dock-manager") !== null;
}
