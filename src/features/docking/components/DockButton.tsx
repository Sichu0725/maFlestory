type DockButtonProps = {
  disabled: boolean;
  onDock: () => void;
};

export function DockButton({ disabled, onDock }: DockButtonProps) {
  return (
    <button type="button" className="toolbar-button" disabled={disabled} onClick={onDock}>
      Dock
    </button>
  );
}
