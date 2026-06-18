import { type ReactNode } from "react";
import { DockPanelBounds } from "../docking/types";

export type OverlaySize = {
    width: number;
    height: number;
};

export type OverlayConfig = {
    id: string;
    route: string;
    visible: boolean;
    bounds: DockPanelBounds;
    minSize: OverlaySize;
    maxSize: OverlaySize;
    minimizeSize: OverlaySize;
};

export type OverlayFrameProps = {
    children: ReactNode;
    className: string;
    overlayId: string;
    title: string;
    initialBounds: DockPanelBounds;
    onError?: (message: string) => void;
};

export type DragState = {
    pointerId: number;
    startScreenX: number;
    startScreenY: number;
    startBounds: DockPanelBounds;
};

export type ResizeEdge = "n" | "s" | "e" | "w" | "ne" | "nw" | "se" | "sw";

export type ResizeState = {
    pointerId: number;
    edge: ResizeEdge;
    startScreenX: number;
    startScreenY: number;
    startBounds: DockPanelBounds;
};

export type ViewportSize = {
    width: number;
    height: number;
};
