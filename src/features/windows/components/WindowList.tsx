import type { WindowInfo } from "../types";

type WindowListProps = {
  windows: WindowInfo[];
  selectedWindowId: number | null;
  onSelectWindow: (hwnd: number) => void;
};

export function WindowList({ windows, selectedWindowId, onSelectWindow }: WindowListProps) {
  // Displays selectable native windows returned by the Tauri command layer.
  if (windows.length === 0) {
    return (
      <div className="window-list">
        <p className="window-list__empty">No windows found</p>
      </div>
    );
  }

  return (
    <div className="window-list" role="listbox" aria-label="Windows">
      {windows.map((window) => (
        <button
          key={window.hwnd}
          type="button"
          className="window-list__item"
          aria-selected={window.hwnd === selectedWindowId}
          onClick={() => onSelectWindow(window.hwnd)}
        >
          {window.title}
        </button>
      ))}
    </div>
  );
}
