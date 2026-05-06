import { Laptop, Smartphone } from "lucide-react";
import { useEffect, useState } from "react";
import type { Awareness } from "y-protocols/awareness";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";

interface RemoteUser {
  clientId: number;
  name: string;
  platform: "desktop" | "mobile";
  deviceId: string;
  color: string;
}

interface AwarenessUserPayload {
  name: string;
  platform: "desktop" | "mobile";
  deviceId: string;
  color: string;
}

function readRemoteUsers(awareness: Awareness): RemoteUser[] {
  const localId = awareness.clientID;
  const out: RemoteUser[] = [];
  for (const [clientId, raw] of awareness.getStates()) {
    if (clientId === localId) continue;
    const state = raw as { user?: AwarenessUserPayload };
    const u = state.user;
    if (!u || typeof u.name !== "string") continue;
    out.push({
      clientId,
      name: u.name,
      platform: u.platform,
      deviceId: u.deviceId,
      color: u.color,
    });
  }
  return out.sort((a, b) => a.deviceId.localeCompare(b.deviceId));
}

function PlatformIcon({
  platform,
  className,
}: {
  platform: RemoteUser["platform"];
  className?: string;
}) {
  return platform === "desktop" ? (
    <Laptop className={className} />
  ) : (
    <Smartphone className={className} />
  );
}

export function PresenceAvatars({ awareness }: { awareness: Awareness | null }) {
  const [users, setUsers] = useState<RemoteUser[]>([]);

  useEffect(() => {
    if (!awareness) {
      setUsers([]);
      return;
    }
    const update = () => setUsers(readRemoteUsers(awareness));
    update();
    awareness.on("change", update);
    return () => {
      awareness.off("change", update);
    };
  }, [awareness]);

  if (users.length === 0) return null;

  const visible = users.slice(0, 3);
  const overflow = users.length - visible.length;

  return (
    <Popover>
      <PopoverTrigger asChild>
        <button
          type="button"
          className="flex items-center -space-x-1.5 rounded-md px-1 py-0.5 hover:bg-muted focus:outline-none focus:ring-2 focus:ring-ring/50"
          aria-label={`协作者 ${users.length} 人`}
        >
          {visible.map((u) => (
            <div
              key={u.clientId}
              className="h-5 w-5 rounded-full ring-2 ring-card flex items-center justify-center text-[10px] font-semibold text-white select-none"
              style={{ backgroundColor: u.color }}
            >
              {u.name.slice(0, 1).toUpperCase()}
            </div>
          ))}
          {overflow > 0 && (
            <div className="h-5 w-5 rounded-full ring-2 ring-card flex items-center justify-center text-[10px] font-medium bg-muted text-muted-foreground select-none">
              +{overflow}
            </div>
          )}
        </button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-64 p-2">
        <div className="px-1.5 py-1 text-xs font-medium text-muted-foreground">
          协作者 · {users.length}
        </div>
        <ul className="mt-1 space-y-0.5">
          {users.map((u) => (
            <li key={u.clientId} className="flex items-center gap-2 rounded-md px-2 py-1.5">
              <div
                className="h-6 w-6 shrink-0 rounded-full flex items-center justify-center text-[11px] font-semibold text-white"
                style={{ backgroundColor: u.color }}
              >
                {u.name.slice(0, 1).toUpperCase()}
              </div>
              <span className="flex-1 truncate text-sm text-foreground">{u.name}</span>
              <PlatformIcon
                platform={u.platform}
                className="h-3.5 w-3.5 shrink-0 text-muted-foreground"
              />
            </li>
          ))}
        </ul>
      </PopoverContent>
    </Popover>
  );
}
