import { Trans, useLingui } from "@lingui/react/macro";
import { toast } from "sonner";
import { unpairDevice } from "@/commands/pairing";
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

interface UnpairConfirmDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  deviceName: string;
  peerId: string;
  onConfirm: () => void;
}

export function UnpairConfirmDialog({
  open,
  onOpenChange,
  deviceName,
  peerId,
  onConfirm,
}: UnpairConfirmDialogProps) {
  const { t } = useLingui();
  const { loading, run } = useAsyncAction();

  async function handleConfirm() {
    await run(async () => {
      await unpairDevice(peerId);
      toast.success(t`已取消与 ${deviceName} 的配对`);
      onConfirm();
    });
  }

  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>
            <Trans>确定要取消配对吗？</Trans>
          </AlertDialogTitle>
          <AlertDialogDescription>
            <Trans>取消与 {deviceName} 的配对后，将停止与该设备的笔记同步。</Trans>
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel disabled={loading}>
            <Trans>取消</Trans>
          </AlertDialogCancel>
          <AlertDialogAction variant="destructive" onClick={handleConfirm} disabled={loading}>
            <Trans>确认</Trans>
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
