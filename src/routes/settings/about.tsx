import { Trans, useLingui } from "@lingui/react/macro";
import { createFileRoute } from "@tanstack/react-router";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  BookOpen,
  CheckCircle2,
  Code2,
  Download,
  FileText,
  Loader2,
  MessageSquare,
  RefreshCw,
} from "lucide-react";
import { useEffect, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { useUpgradeStore } from "@/stores/upgradeStore";

function AboutPage() {
  const { t } = useLingui();
  const [appVersion, setAppVersion] = useState<string | null>(null);

  const { status, latestVersion, currentVersion, progress, checkForUpdate, startDownload } =
    useUpgradeStore(
      useShallow((s) => ({
        status: s.status,
        latestVersion: s.latestVersion,
        currentVersion: s.currentVersion,
        progress: s.progress,
        checkForUpdate: s.checkForUpdate,
        startDownload: s.startDownload,
      })),
    );

  useEffect(() => {
    getVersion().then(setAppVersion).catch(console.error);
  }, []);

  const displayVersion = currentVersion ?? appVersion;

  return (
    <div className="flex flex-1 flex-col items-center justify-center pb-20">
      <div className="flex flex-col items-center gap-5">
        {/* App Icon + Name + Version */}
        <div className="flex items-center gap-4">
          <div className="flex h-14 w-14 items-center justify-center rounded-2xl bg-primary/10">
            <span className="text-xl font-bold text-primary">SN</span>
          </div>
          <div>
            <h1 className="text-lg font-semibold tracking-tight">SwarmNote</h1>
            <div className="flex items-center gap-2">
              {displayVersion && (
                <span className="text-sm text-muted-foreground">v{displayVersion}</span>
              )}
              <span
                className={`inline-flex items-center gap-1 text-xs font-medium text-green-600 ${status === "up-to-date" ? "" : "invisible"}`}
              >
                <CheckCircle2 className="h-3 w-3" />
                <Trans>已是最新</Trans>
              </span>
            </div>
          </div>
        </div>

        {/* Slogan */}
        <p className="text-sm text-muted-foreground">
          <Trans>去中心化、本地优先的 P2P 笔记应用</Trans>
        </p>

        {/* Download Progress */}
        {(status === "downloading" || status === "ready") && progress && (
          <div className="w-64 space-y-2">
            <div className="flex items-center justify-between text-xs">
              <span className="text-muted-foreground">
                <Trans>正在下载 v{latestVersion ?? "?"}</Trans>
              </span>
              <span className="font-medium text-primary">{progress.percent}%</span>
            </div>
            <div className="h-1.5 w-full overflow-hidden rounded-full bg-muted">
              <div
                className="h-full rounded-full bg-primary transition-all duration-300"
                style={{ width: `${progress.percent}%` }}
              />
            </div>
            <div className="flex items-center justify-between text-xs text-muted-foreground">
              <span>
                {formatBytes(progress.downloaded)} / {formatBytes(progress.total)}
              </span>
              <span>{formatBytes(progress.speed)}/s</span>
            </div>
          </div>
        )}

        {/* Action Buttons */}
        <div className="flex items-center gap-3">
          <UpdateActionButton
            status={status}
            latestVersion={latestVersion}
            onCheck={() => checkForUpdate(true)}
            onUpdate={startDownload}
          />
          <Button
            variant="outline"
            size="sm"
            onClick={() => openUrl("https://github.com/yexiyue/SwarmNote/releases")}
          >
            <FileText className="h-3.5 w-3.5" />
            <Trans>更新日志</Trans>
          </Button>
        </div>
      </div>

      {/* Bottom Links */}
      <div className="absolute bottom-6 flex items-center gap-4">
        <LinkButton icon={Code2} label="GitHub" url="https://github.com/yexiyue/SwarmNote" />
        <Separator orientation="vertical" className="h-3" />
        <LinkButton icon={BookOpen} label={t`文档`} url="https://yexiyue.github.io/SwarmNote/" />
        <Separator orientation="vertical" className="h-3" />
        <LinkButton
          icon={MessageSquare}
          label={t`反馈`}
          url="https://github.com/yexiyue/SwarmNote/issues"
        />
      </div>
    </div>
  );
}

function LinkButton({
  icon: Icon,
  label,
  url,
}: {
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  url: string;
}) {
  return (
    <button
      type="button"
      onClick={() => openUrl(url)}
      className="flex items-center gap-1 text-xs text-muted-foreground transition-colors hover:text-foreground"
    >
      <Icon className="h-3 w-3" />
      {label}
    </button>
  );
}

function UpdateActionButton({
  status,
  latestVersion,
  onCheck,
  onUpdate,
}: {
  status: string;
  latestVersion: string | null;
  onCheck: () => void;
  onUpdate: () => void;
}) {
  switch (status) {
    case "checking":
      return (
        <Button variant="outline" size="sm" disabled>
          <Loader2 className="h-3.5 w-3.5 animate-spin" />
          <Trans>检查中...</Trans>
        </Button>
      );
    case "available":
      return (
        <Button size="sm" onClick={onUpdate}>
          <Download className="h-3.5 w-3.5" />
          <Trans>更新到 v{latestVersion ?? "?"}</Trans>
        </Button>
      );
    case "force-required":
    case "downloading":
    case "ready":
      return (
        <Button size="sm" disabled>
          <Loader2 className="h-3.5 w-3.5 animate-spin" />
          <Trans>下载中...</Trans>
        </Button>
      );
    default:
      return (
        <Button variant="outline" size="sm" onClick={onCheck}>
          <RefreshCw className="h-3.5 w-3.5" />
          <Trans>检查更新</Trans>
        </Button>
      );
  }
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

export const Route = createFileRoute("/settings/about")({
  component: AboutPage,
});
