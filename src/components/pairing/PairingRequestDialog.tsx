import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { DeviceInfoCard } from "./DeviceInfoCard";

interface PairingRequestPayload {
  pendingId: number;
  peerId: string;
  osInfo: { name?: string; hostname: string; os: string; platform: string; arch: string };
  method: { type: "Direct" } | { type: "Code"; code: string };
  expiresAt: string;
}

interface PairingRequestDialogProps {
  data: PairingRequestPayload;
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
  const [remaining, setRemaining] = useState(() =>
    Math.max(0, Math.ceil((new Date(data.expiresAt).getTime() - Date.now()) / 1000)),
  );

  // Countdown timer — auto-reject on expiry
  useEffect(() => {
    const interval = setInterval(() => {
      const left = Math.max(0, Math.ceil((new Date(data.expiresAt).getTime() - Date.now()) / 1000));
      setRemaining(left);
      if (left <= 0) {
        clearInterval(interval);
        onReject();
      }
    }, 1000);

    return () => clearInterval(interval);
  }, [data.expiresAt, onReject]);

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>收到配对请求</DialogTitle>
        </DialogHeader>

        <DeviceInfoCard
          name={data.osInfo.name}
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
