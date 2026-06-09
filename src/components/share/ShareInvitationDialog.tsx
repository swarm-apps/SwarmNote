import { Trans } from "@lingui/react/macro";
import { Share2 } from "lucide-react";
import { useMemo } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useCountdown } from "@/hooks/useCountdown";
import type { ShareInvitationReceived } from "@/lib/bindings";
import { useNetworkStore } from "@/stores/networkStore";

interface ShareInvitationDialogProps {
  data: ShareInvitationReceived;
  responding: boolean;
  onAccept: () => void;
  onReject: () => void;
  onClose: () => void;
}

/** 收到工作区协作邀请的弹窗(镜像 PairingRequestDialog)。未接受前对端不授权;
 * 接受后对端才签发 grant op,本设备可在「同步」里拉取该工作区。 */
export function ShareInvitationDialog({
  data,
  responding,
  onAccept,
  onReject,
  onClose,
}: ShareInvitationDialogProps) {
  // 倒计时——到期自动拒绝。
  const remaining = useCountdown(data.expiresAt, onReject);

  // 邀请方设备名:从已知设备列表解析,解析不到则用截断的 peerId 兜底。
  const devices = useNetworkStore((s) => s.devices);
  const inviterName = useMemo(() => {
    const d = devices.find((dev) => dev.peerId === data.peerId);
    return d?.name ?? data.peerId.slice(0, 12);
  }, [devices, data.peerId]);

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Share2 className="h-4 w-4" />
            <Trans>工作区协作邀请</Trans>
          </DialogTitle>
        </DialogHeader>

        <DialogDescription>
          <Trans>
            {inviterName} 邀请你协作工作区「{data.workspaceName}」。接受后你将作为协作者加入,
            可在「同步」里拉取其内容。
          </Trans>
        </DialogDescription>

        <div className="text-center text-xs text-muted-foreground">
          <Trans>剩余时间：{remaining} 秒</Trans>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={onReject} disabled={responding}>
            <Trans>拒绝</Trans>
          </Button>
          <Button onClick={onAccept} disabled={responding}>
            <Trans>接受</Trans>
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
