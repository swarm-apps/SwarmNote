import { Trans, useLingui } from "@lingui/react/macro";
import { toast } from "sonner";
import type { Device } from "@/commands/pairing";
import { requestPairing } from "@/commands/pairing";
import { Button } from "@/components/ui/button";
import { useAsyncAction } from "@/hooks/useAsyncAction";
import { cn } from "@/lib/utils";
import { ConnectionBadge } from "./ConnectionBadge";
import { DeviceAvatar } from "./DeviceAvatar";

interface NearbyDeviceCardProps {
  device: Device;
  onPaired?: () => void;
  isLast?: boolean;
}

export function NearbyDeviceCard({ device, onPaired, isLast }: NearbyDeviceCardProps) {
  const { t } = useLingui();
  const { loading, run } = useAsyncAction();

  async function handlePair() {
    await run(async () => {
      const resp = await requestPairing(
        device.peerId,
        { type: "Direct" },
        {
          name: device.name,
          hostname: device.hostname,
          os: device.os,
          platform: device.platform,
          arch: device.arch,
        },
      );
      if (resp.status === "Success") {
        toast.success(t`已与 ${device.name ?? device.hostname} 配对`);
        onPaired?.();
      } else {
        toast.error(resp.reason ?? t`配对被拒绝`);
      }
    });
  }

  return (
    <div className={cn("flex items-center gap-2.5 px-3.5 py-3", !isLast && "border-b")}>
      <DeviceAvatar os={device.os} />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1.5">
          <span className="text-[13px]">{device.name ?? device.hostname}</span>
          {device.connection && (
            <ConnectionBadge type={device.connection} latency={device.latency} />
          )}
        </div>
        <div className="text-[11px] text-muted-foreground">
          {device.name ? `${device.hostname} · ` : ""}
          {device.os} · {device.platform}
        </div>
      </div>
      <Button size="sm" onClick={handlePair} loading={loading}>
        {loading ? <Trans>配对中...</Trans> : <Trans>配对</Trans>}
      </Button>
    </div>
  );
}
