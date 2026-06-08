import { Trans, useLingui } from "@lingui/react/macro";
import { toast } from "sonner";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { useAsyncAction } from "@/hooks/useAsyncAction";
import { commands, type MemberInfo } from "@/lib/bindings";

interface RevokeMemberDialogProps {
  /** Target member, or `null` when closed. */
  member: MemberInfo | null;
  workspaceId: string;
  onOpenChange: (open: boolean) => void;
  onDone: () => void;
}

export function RevokeMemberDialog({
  member,
  workspaceId,
  onOpenChange,
  onDone,
}: RevokeMemberDialogProps) {
  const { t } = useLingui();
  const { loading, run } = useAsyncAction();
  const name = member?.name ?? member?.peerId.slice(0, 12) ?? "";

  async function handleConfirm() {
    if (!member) return;
    await run(async () => {
      await commands.revokeWorkspaceMember(workspaceId, member.peerId);
      toast.success(t`已移除 ${name}`);
      onDone();
    });
  }

  return (
    <AlertDialog open={member !== null} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>
            <Trans>移除成员？</Trans>
          </AlertDialogTitle>
          {/* Honest about lazy revocation: revoking stops future content, not
              already-synced content (no forward secrecy in v1). */}
          <AlertDialogDescription>
            <Trans>
              移除 {name} 后，该设备将无法获取此工作区的新内容；它此前已同步的内容仍保留在其本地。
            </Trans>
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel disabled={loading}>
            <Trans>取消</Trans>
          </AlertDialogCancel>
          <AlertDialogAction variant="destructive" onClick={handleConfirm} disabled={loading}>
            <Trans>移除</Trans>
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
