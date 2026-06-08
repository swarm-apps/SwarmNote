import { Trans, useLingui } from "@lingui/react/macro";
import { Share2, UserPlus } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import { DeviceAvatar } from "@/components/pairing/DeviceAvatar";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ErrorMessage } from "@/components/ui/error-message";
import { useAsyncAction } from "@/hooks/useAsyncAction";
import { commands, type MemberInfo } from "@/lib/bindings";
import { useNetworkStore } from "@/stores/networkStore";
import { MemberRow } from "./MemberRow";
import { RevokeMemberDialog } from "./RevokeMemberDialog";

interface ShareDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  workspaceId: string;
  workspaceName: string;
}

/** Workspace sharing + member management. Owner can invite paired devices as
 * Collaborators and revoke members; non-owners see a read-only member list.
 * A granted device picks up the workspace via the normal sync flow. */
export function ShareDialog({ open, onOpenChange, workspaceId, workspaceName }: ShareDialogProps) {
  const { t } = useLingui();
  const [members, setMembers] = useState<MemberInfo[]>([]);
  const [revokeTarget, setRevokeTarget] = useState<MemberInfo | null>(null);
  const { loading, error, run } = useAsyncAction();

  const devices = useNetworkStore((s) => s.devices);
  const pairedDevices = useMemo(() => devices.filter((d) => d.isPaired), [devices]);

  const refreshMembers = useCallback(async () => {
    setMembers(await commands.listWorkspaceMembers(workspaceId));
  }, [workspaceId]);

  useEffect(() => {
    if (!open) return;
    void useNetworkStore.getState().refreshDevices();
    void run(refreshMembers);
  }, [open, run, refreshMembers]);

  const amOwner = members.find((m) => m.isSelf)?.role === "owner";
  const memberPeers = useMemo(() => new Set(members.map((m) => m.peerId)), [members]);
  const addable = pairedDevices.filter((d) => !memberPeers.has(d.peerId));

  async function handleShare(peerId: string, name: string) {
    await run(async () => {
      await commands.shareWorkspaceToDevice(workspaceId, peerId);
      toast.success(t`已分享给 ${name}`);
      await refreshMembers();
    });
  }

  return (
    <>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <Share2 className="h-4 w-4" />
              <Trans>共享「{workspaceName}」</Trans>
            </DialogTitle>
            <DialogDescription>
              {amOwner ? (
                <Trans>邀请已配对的设备协作编辑此工作区</Trans>
              ) : (
                <Trans>此工作区的成员</Trans>
              )}
            </DialogDescription>
          </DialogHeader>

          {/* Members */}
          <section className="space-y-2">
            <h3 className="text-[13px] font-medium text-foreground">
              <Trans>成员</Trans>
            </h3>
            <div className="overflow-hidden rounded-lg border border-border">
              {members.length === 0 ? (
                <p className="px-3.5 py-6 text-center text-xs text-muted-foreground">
                  {loading ? <Trans>加载中…</Trans> : <Trans>暂无成员</Trans>}
                </p>
              ) : (
                members.map((m, i) => (
                  <MemberRow
                    key={m.peerId}
                    member={m}
                    canManage={amOwner}
                    isLast={i === members.length - 1}
                    onRevoke={setRevokeTarget}
                  />
                ))
              )}
            </div>
          </section>

          {/* Add devices (owner only) */}
          {amOwner && (
            <section className="space-y-2">
              <h3 className="text-[13px] font-medium text-foreground">
                <Trans>添加设备</Trans>
              </h3>
              {addable.length === 0 ? (
                <div className="flex flex-col items-center gap-1.5 rounded-lg border border-dashed border-border py-6">
                  <UserPlus className="h-5 w-5 text-muted-foreground/40" />
                  <p className="text-xs font-medium text-muted-foreground">
                    <Trans>没有可添加的设备</Trans>
                  </p>
                  <p className="text-[11px] text-muted-foreground/60">
                    <Trans>先在「设备」设置里配对设备</Trans>
                  </p>
                </div>
              ) : (
                <div className="overflow-hidden rounded-lg border border-border">
                  {addable.map((d, i) => (
                    <div
                      key={d.peerId}
                      className={`flex items-center gap-2.5 px-3.5 py-2.5 ${
                        i === addable.length - 1 ? "" : "border-b border-border"
                      }`}
                    >
                      <DeviceAvatar os={d.os} />
                      <span className="min-w-0 flex-1 truncate text-sm font-medium text-foreground">
                        {d.name ?? d.peerId.slice(0, 12)}
                      </span>
                      <Button
                        size="sm"
                        disabled={loading}
                        onClick={() => handleShare(d.peerId, d.name ?? d.peerId.slice(0, 8))}
                      >
                        <Share2 className="h-3.5 w-3.5" />
                        <Trans>分享</Trans>
                      </Button>
                    </div>
                  ))}
                </div>
              )}
            </section>
          )}

          <ErrorMessage error={error} />
        </DialogContent>
      </Dialog>

      <RevokeMemberDialog
        member={revokeTarget}
        workspaceId={workspaceId}
        onOpenChange={(o) => {
          if (!o) setRevokeTarget(null);
        }}
        onDone={() => {
          setRevokeTarget(null);
          void run(refreshMembers);
        }}
      />
    </>
  );
}
