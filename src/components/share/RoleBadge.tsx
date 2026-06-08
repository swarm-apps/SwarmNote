import { Trans } from "@lingui/react/macro";
import { Badge } from "@/components/ui/badge";
import type { Role } from "@/lib/bindings";

/** Role pill, matching ConnectionBadge's rounded-full chip style. Colors go
 * through theme variables (Owner = primary, Collaborator = secondary). */
export function RoleBadge({ role }: { role: Role }) {
  return (
    <Badge
      variant={role === "owner" ? "default" : "secondary"}
      className="rounded-full px-2 py-0.5 text-[11px] font-medium"
    >
      {role === "owner" ? <Trans>所有者</Trans> : <Trans>协作者</Trans>}
    </Badge>
  );
}
