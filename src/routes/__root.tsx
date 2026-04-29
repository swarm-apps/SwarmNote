import { i18n } from "@lingui/core";
import { I18nProvider } from "@lingui/react";
import { createRootRoute, Outlet } from "@tanstack/react-router";
import { TanStackRouterDevtools } from "@tanstack/router-devtools";
import { listen } from "@tauri-apps/api/event";
import { Loader2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { getRecentWorkspaces } from "@/commands/workspace";
import { CommandPalette } from "@/components/layout/CommandPalette";
import { GlobalActionDialogs } from "@/components/pairing/GlobalActionDialogs";
import { TooltipProvider } from "@/components/ui/tooltip";
import { ForceUpdateDialog, PromptUpdateDialog } from "@/components/upgrade";
import { useKeyboardShortcuts } from "@/hooks/useKeyboardShortcuts";
import { useEditorStore, waitForEditorHydration } from "@/stores/editorStore";
import { useNotificationStore } from "@/stores/notificationStore";
import { waitForOnboardingHydration } from "@/stores/onboardingStore";
import { useUpgradeStore } from "@/stores/upgradeStore";
import { useWorkspaceStore } from "@/stores/workspaceStore";

export const Route = createRootRoute({
  component: RootComponent,
});

function RootComponent() {
  useKeyboardShortcuts();

  const initFromBackend = useWorkspaceStore((s) => s.initFromBackend);
  const [hydrated, setHydrated] = useState(false);
  const checkForUpdate = useUpgradeStore((s) => s.checkForUpdate);
  const upgradeStatus = useUpgradeStore((s) => s.status);
  const [promptOpen, setPromptOpen] = useState(false);
  const prevStatusRef = useRef<string>("idle");

  useEffect(() => {
    Promise.all([waitForOnboardingHydration(), waitForEditorHydration(), initFromBackend()])
      .then(async () => {
        // Prune recentDocs for workspaces that no longer exist in the recent list.
        try {
          const recents = await getRecentWorkspaces();
          const validIds = new Set(recents.map((w) => w.uuid).filter((id): id is string => !!id));
          useEditorStore.getState().pruneRecentDocs(validIds);
        } catch (err) {
          console.warn("Failed to prune recent docs:", err);
        }
      })
      .catch((err) => console.error("Hydration failed:", err))
      .finally(() => setHydrated(true));
  }, [initFromBackend]);

  // 启动 3 秒后自动检查更新
  useEffect(() => {
    const timer = setTimeout(() => {
      checkForUpdate();
    }, 3000);
    return () => clearTimeout(timer);
  }, [checkForUpdate]);

  // 有可选更新时弹出 Dialog
  useEffect(() => {
    if (prevStatusRef.current !== "available" && upgradeStatus === "available") {
      setPromptOpen(true);
    }
    prevStatusRef.current = upgradeStatus;
  }, [upgradeStatus]);

  // Listen for pairing request events from the Tauri backend
  useEffect(() => {
    const unlisten = listen("pairing-request-received", (event) => {
      useNotificationStore.getState().push({
        id: `pairing-${Date.now()}`,
        type: "pairing-request",
        payload: event.payload,
        timestamp: Date.now(),
      });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  if (!hydrated) {
    return (
      <div className="flex h-screen items-center justify-center">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  return (
    <I18nProvider i18n={i18n}>
      <TooltipProvider>
        <Outlet />
        <CommandPalette />
        <GlobalActionDialogs />
        <ForceUpdateDialog />
        <PromptUpdateDialog open={promptOpen} onOpenChange={setPromptOpen} />
        {import.meta.env.DEV && <TanStackRouterDevtools position="bottom-right" />}
      </TooltipProvider>
    </I18nProvider>
  );
}
