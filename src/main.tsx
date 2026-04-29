import { createRouter, RouterProvider } from "@tanstack/react-router";
import React from "react";
import ReactDOM from "react-dom/client";
import "@/App.css";
import { Toaster } from "@/components/ui/sonner";
import { initI18n } from "@/i18n";
import { routeTree } from "./routeTree.gen";

// Initialize source locale synchronously before first render
initI18n();

const router = createRouter({ routeTree });

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <RouterProvider router={router} />
    <Toaster position="top-center" />
  </React.StrictMode>,
);
