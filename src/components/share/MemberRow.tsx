import { Trans } from "@lingui/react/macro";
import { UserMinus } from "lucide-react";
import { DeviceAvatar } from "@/components/pairing/DeviceAvatar";
import { Button } from "@/components/ui/button";
import type { MemberInfo } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { RoleBadge } from "./RoleBadge";

interface MemberRowProps {
  member: MemberInfo;
  /** Viewer is the workspace owner — show the revoke action on hover. */
  canManage: boolean;
  isLast?: boolean;
  onRevoke: (member: MemberInfo) => void;
}

/** A workspace member row — same layout/spacing as PairedDeviceCard. */
export function MemberRow({ member, canManage, isLast, onRevoke }: MemberRowProps) {
  const label = member.name ?? member.peerId.slice(0, 12);
  return (
    <div
      className={cn(
        "group flex items-center gap-2.5 px-3.5 py-2.5 transition hover:bg-muted/50",
        !isLast && "border-b border-border",
      )}
    >
      <DeviceAvatar os={member.os} isCurrent={member.isSelf} />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="truncate text-sm font-medium text-foreground">{label}</span>
          {member.isSelf && (
            <span className="text-xs text-muted-foreground">
              <Trans>（你）</Trans>
            </span>
          )}
          <RoleBadge role={member.role} />
        </div>
        <div className="mt-0.5 flex items-center gap-1.5 text-xs text-muted-foreground">
          <span
            className={cn(
              "h-1.5 w-1.5 rounded-full",
              member.isOnline ? "bg-primary" : "bg-muted-foreground/30",
            )}
          />
          {member.isOnline ? <Trans>在线</Trans> : <Trans>离线</Trans>}
        </div>
      </div>
      {canManage && !member.isSelf && (
        <Button
          variant="ghost"
          size="icon-xs"
          className="shrink-0 opacity-0 transition group-hover:opacity-100"
          onClick={() => onRevoke(member)}
          aria-label="revoke member"
        >
          <UserMinus className="h-3.5 w-3.5 text-destructive" />
        </Button>
      )}
    </div>
  );
}
