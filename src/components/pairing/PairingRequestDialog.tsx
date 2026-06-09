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
import type { PairingRequestReceived } from "@/lib/bindings";
import { DeviceInfoCard } from "./DeviceInfoCard";

interface PairingRequestDialogProps {
  data: PairingRequestReceived;
  responding: boolean;
  onAccept: () => void;
  onReject: () => void;
  onClose: () => void;
}

export function PairingRequestDialog({
  data,
  responding,
  onAccept,
  onReject,
  onClose,
}: PairingRequestDialogProps) {
  // 倒计时——到期自动拒绝。
  const remaining = useCountdown(data.expiresAt, onReject);

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>收到配对请求</DialogTitle>
        </DialogHeader>

        <DeviceInfoCard
          name={data.osInfo.name ?? undefined}
          hostname={data.osInfo.hostname}
          os={data.osInfo.os}
          platform={data.osInfo.platform}
        />

        <DialogDescription>
          该设备请求与您配对。
          <br />
          配对后可同步笔记。
        </DialogDescription>

        <div className="text-center text-xs text-muted-foreground">剩余时间：{remaining} 秒</div>

        <DialogFooter>
          <Button variant="outline" onClick={onReject} disabled={responding}>
            拒绝
          </Button>
          <Button onClick={onAccept} disabled={responding}>
            接受
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
