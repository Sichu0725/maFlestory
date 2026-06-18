import { createFileRoute } from "@tanstack/react-router";

import { BottomPage } from "../pages/BottomPage";

export const Route = createFileRoute("/")({
  component: BottomPage,
});
