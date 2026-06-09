import { i18n } from "@lingui/core";
import { I18nProvider } from "@lingui/react";
import { useLingui } from "@lingui/react/macro";
import { createRootRoute, Outlet } from "@tanstack/react-router";
import { TanStackRouterDevtools } from "@tanstack/router-devtools";
import { Loader2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { CommandPalette } from "@/components/layout/CommandPalette";
import { GlobalActionDialogs } from "@/components/pairing/GlobalActionDialogs";
import { TooltipProvider } from "@/components/ui/tooltip";
import { ForceUpdateDialog, PromptUpdateDialog } from "@/components/upgrade";
import { useKeyboardShortcuts } from "@/hooks/useKeyboardShortcuts";
import { commands, events } from "@/lib/bindings";
import { useEditorStore, waitForEditorHydration } from "@/stores/editorStore";
import { useNotificationStore } from "@/stores/notificationStore";
import { waitForOnboardingHydration } from "@/stores/onboardingStore";
import { usePreferencesStore } from "@/stores/preferencesStore";
import { useUpgradeStore } from "@/stores/upgradeStore";
import { useWorkspaceStore } from "@/stores/workspaceStore";

export const Route = createRootRoute({
  component: RootComponent,
});

function RootComponent() {
  useKeyboardShortcuts();
  const { t } = useLingui();

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
          const recents = await commands.getRecentWorkspaces();
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
    const unlisten = events.pairingRequestReceived.listen((event) => {
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

  // 监听工作区协作邀请。若用户在设置里开启了"自动接受邀请",直接接受、不弹窗;
  // 否则走通知队列弹窗让用户决定。
  useEffect(() => {
    const unlisten = events.shareInvitationReceived.listen((event) => {
      const payload = event.payload;
      if (usePreferencesStore.getState().autoAcceptInvitations) {
        void commands.respondShareInvitation(payload.pendingId, true);
        toast.info(t`已自动接受「${payload.workspaceName}」的协作邀请`);
        return;
      }
      useNotificationStore.getState().push({
        id: `share-invite-${payload.pendingId}`,
        type: "share-invitation",
        payload,
        timestamp: Date.now(),
      });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [t]);

  // Notify when this device is removed from a shared workspace (owner revoked
  // our access). The backend has already unsubscribed us from its realtime
  // updates; we just surface it + refresh workspace state.
  useEffect(() => {
    const unlisten = events.memberRevoked.listen((event) => {
      const current = useWorkspaceStore.getState().workspace;
      const name = current?.id === event.payload.workspaceId ? current.name : null;
      toast.info(name ? t`你已被移出「${name}」工作区` : t`你已被移出一个共享工作区`, {
        description: t`将不再接收其新内容；已同步到本地的内容仍保留。`,
      });
      void useWorkspaceStore.getState().initFromBackend();
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [t]);

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
