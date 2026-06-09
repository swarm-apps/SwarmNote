import { useLingui } from "@lingui/react/macro";
import { useCallback, useState } from "react";
import { toast } from "sonner";
import { ShareInvitationDialog } from "@/components/share/ShareInvitationDialog";
import { commands } from "@/lib/bindings";
import { useNotificationStore } from "@/stores/notificationStore";
import { PairingRequestDialog } from "./PairingRequestDialog";

export function GlobalActionDialogs() {
  const { t } = useLingui();
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
      commands.respondPairingRequest(pendingId, accept).catch(() => {
        toast.error(accept ? t`接受配对失败` : t`拒绝配对失败`);
      });
      dismiss();
    },
    [dismiss, t],
  );

  // 应答分享邀请:接受会让邀请方(阻塞中的 invite)解阻塞并签发授权。
  const handleShareInvitationRespond = useCallback(
    (pendingId: number, accept: boolean) => {
      setResponding(true);
      commands.respondShareInvitation(pendingId, accept).catch(() => {
        toast.error(accept ? t`接受邀请失败` : t`拒绝邀请失败`);
      });
      dismiss();
    },
    [dismiss, t],
  );

  if (!current) return null;

  if (current.type === "pairing-request") {
    const data = current.payload;
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

  if (current.type === "share-invitation") {
    const data = current.payload;
    return (
      <ShareInvitationDialog
        data={data}
        responding={responding}
        onAccept={() => handleShareInvitationRespond(data.pendingId, true)}
        onReject={() => handleShareInvitationRespond(data.pendingId, false)}
        onClose={dismiss}
      />
    );
  }

  return null;
}
