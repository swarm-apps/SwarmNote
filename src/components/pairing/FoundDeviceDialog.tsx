import { Trans, useLingui } from "@lingui/react/macro";
import { Link } from "lucide-react";
import { requestPairing } from "@/commands/pairing";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ErrorMessage } from "@/components/ui/error-message";
import { useAsyncAction } from "@/hooks/useAsyncAction";
import { DeviceInfoCard } from "./DeviceInfoCard";

interface FoundDeviceDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  peerId: string;
  osInfo: { name?: string; hostname: string; os: string; platform: string; arch: string };
  code: string;
  onSuccess: () => void;
}

export function FoundDeviceDialog({
  open,
  onOpenChange,
  peerId,
  osInfo,
  code,
  onSuccess,
}: FoundDeviceDialogProps) {
  const { t } = useLingui();
  const { loading, error, run, setError } = useAsyncAction();

  async function handleConfirm() {
    await run(async () => {
      const resp = await requestPairing(peerId, { type: "Code", code }, osInfo);
      if (resp.status === "Success") {
        onSuccess();
      } else {
        setError(resp.reason ?? t`配对被拒绝`);
      }
    });
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent showCloseButton={false}>
        <DialogHeader>
          <DialogTitle>
            <Trans>找到设备</Trans>
          </DialogTitle>
        </DialogHeader>

        <DeviceInfoCard
          name={osInfo.name}
          hostname={osInfo.hostname}
          os={osInfo.os}
          platform={osInfo.platform}
        />

        <DialogDescription>
          <Trans>确认与此设备配对？</Trans>
        </DialogDescription>

        <ErrorMessage error={error} />

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)} disabled={loading}>
            <Trans>取消</Trans>
          </Button>
          <Button onClick={handleConfirm} loading={loading}>
            {loading ? (
              <Trans>配对中...</Trans>
            ) : (
              <>
                <Link className="h-4 w-4" />
                <Trans>确认配对</Trans>
              </>
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
