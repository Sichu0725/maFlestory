import { createFileRoute } from "@tanstack/react-router";

import { DockManagerPage } from "../../pages/DockManagerPage";

export const Route = createFileRoute("/overlay/dock-manager")({
  component: DockManagerPage,
});
