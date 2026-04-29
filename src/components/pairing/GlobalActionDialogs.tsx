import { useCallback, useState } from "react";
import { toast } from "sonner";
import { respondPairingRequest } from "@/commands/pairing";
import { useNotificationStore } from "@/stores/notificationStore";
import { PairingRequestDialog } from "./PairingRequestDialog";

export function GlobalActionDialogs() {
  const current = useNotificationStore((s) => s.current);
  const [responding, setResponding] = useState(false);

  const dismiss = useCallback(() => {
    if (!current) return;
    useNotificationStore.getState().respond(current.id);
    setResponding(false);
  }, [current]);

  const handlePairingRespond = useCallback(
    (pendingId: number, accept: boolean) => {
      setResponding(true);
      respondPairingRequest(pendingId, accept).catch(() => {
        toast.error(accept ? "接受配对失败" : "拒绝配对失败");
      });
      dismiss();
    },
    [dismiss],
  );

  if (!current) return null;

  if (current.type === "pairing-request") {
    // biome-ignore lint/suspicious/noExplicitAny: payload type is validated by event source
    const data = current.payload as any;
    return (
      <PairingRequestDialog
        data={data}
        responding={responding}
        onAccept={() => handlePairingRespond(data.pendingId, true)}
        onReject={() => handlePairingRespond(data.pendingId, false)}
        onClose={dismiss}
      />
    );
  }

  return null;
}
