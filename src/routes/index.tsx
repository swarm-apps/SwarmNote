import { Trans } from "@lingui/react/macro";
import { createFileRoute } from "@tanstack/react-router";
import { Loader2 } from "lucide-react";

import { AppLayout } from "@/components/layout/AppLayout";
import { EditorPane } from "@/components/layout/EditorPane";
import { WorkspacePicker } from "@/components/workspace/WorkspacePicker";
import { useUIStore } from "@/stores/uiStore";
import { useWorkspaceStore } from "@/stores/workspaceStore";

export const Route = createFileRoute("/")({
  component: IndexPage,
});

function HydrateOverlay() {
  const progress = useWorkspaceStore((s) => s.hydrateProgress);
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-background/80 backdrop-blur-sm">
      <div className="flex flex-col items-center gap-3">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
        <p className="text-sm text-muted-foreground">
          {progress ? (
            <Trans>
              正在同步文档 {progress.current}/{progress.total}...
            </Trans>
          ) : (
            <Trans>正在准备工作区...</Trans>
          )}
        </p>
      </div>
    </div>
  );
}

function IndexPage() {
  const workspace = useWorkspaceStore((s) => s.workspace);
  const isLoading = useWorkspaceStore((s) => s.isLoading);
  const hydrating = useWorkspaceStore((s) => s.hydrating);
  const workspacePickerOpen = useUIStore((s) => s.workspacePickerOpen);
  const setWorkspacePickerOpen = useUIStore((s) => s.setWorkspacePickerOpen);

  // 工作区窗口创建时后端已预绑定 workspace，
  // 前端只需等待 workspace:ready 事件。
  if (isLoading || !workspace) {
    return (
      <div className="flex h-screen items-center justify-center">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  return (
    <>
      <AppLayout>
        <EditorPane />
      </AppLayout>
      {hydrating && <HydrateOverlay />}
      <WorkspacePicker open={workspacePickerOpen} onOpenChange={setWorkspacePickerOpen} />
    </>
  );
}
