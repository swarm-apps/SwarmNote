import { Trans, useLingui } from "@lingui/react/macro";
import { createFileRoute } from "@tanstack/react-router";
import { Check, Link, Loader2, Radio, RefreshCw, X } from "lucide-react";
import type * as React from "react";
import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { type DeviceInfo, getDeviceInfo, setDeviceName } from "@/commands/identity";
import { CodePairingCard } from "@/components/pairing/CodePairingCard";
import { DeviceAvatar } from "@/components/pairing/DeviceAvatar";
import { FoundDeviceDialog } from "@/components/pairing/FoundDeviceDialog";
import { InputCodeDialog } from "@/components/pairing/InputCodeDialog";
import { NearbyDeviceCard } from "@/components/pairing/NearbyDeviceCard";
import { PairedDeviceCard } from "@/components/pairing/PairedDeviceCard";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { setupPairingListeners, usePairingStore } from "@/stores/pairingStore";

function EmptyState({
  icon: Icon,
  title,
  description,
}: {
  icon: React.ComponentType<{ className?: string }>;
  title: React.ReactNode;
  description: React.ReactNode;
}) {
  return (
    <div className="flex flex-col items-center gap-1.5 rounded-lg border border-dashed py-6">
      <Icon className="h-5 w-5 text-muted-foreground/40" />
      <p className="text-xs font-medium text-muted-foreground">{title}</p>
      <p className="text-[11px] text-muted-foreground/60">{description}</p>
    </div>
  );
}

function InlineEditName({
  currentName,
  onSave,
  onCancel,
}: {
  currentName: string;
  onSave: (name: string) => Promise<void>;
  onCancel: () => void;
}) {
  const [name, setName] = useState(currentName);
  const [error, setError] = useState(false);
  const [saving, setSaving] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);

  async function handleSave() {
    const trimmed = name.trim();
    if (!trimmed) {
      setError(true);
      return;
    }
    setSaving(true);
    try {
      await onSave(trimmed);
    } finally {
      setSaving(false);
    }
  }

  const preventBlur = (e: React.MouseEvent) => e.preventDefault();

  return (
    <div className="flex items-center gap-1.5">
      <Input
        ref={inputRef}
        value={name}
        onChange={(e) => {
          setName(e.target.value);
          setError(false);
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            handleSave();
          } else if (e.key === "Escape") {
            onCancel();
          }
        }}
        onBlur={onCancel}
        className={`h-7 text-sm font-semibold ${error ? "border-destructive" : ""}`}
        disabled={saving}
      />
      <button
        type="button"
        onMouseDown={preventBlur}
        onClick={handleSave}
        className="flex h-6 w-6 items-center justify-center rounded text-muted-foreground hover:bg-background hover:text-foreground"
        disabled={saving}
      >
        {saving ? (
          <Loader2 className="h-3.5 w-3.5 animate-spin" />
        ) : (
          <Check className="h-3.5 w-3.5" />
        )}
      </button>
      <button
        type="button"
        onMouseDown={preventBlur}
        onClick={onCancel}
        className="flex h-6 w-6 items-center justify-center rounded text-muted-foreground hover:bg-background hover:text-foreground"
      >
        <X className="h-3.5 w-3.5" />
      </button>
    </div>
  );
}

function DevicesPage() {
  const { t } = useLingui();
  const pairedDevices = usePairingStore((s) => s.pairedDevices);
  const nearbyDevices = usePairingStore((s) => s.nearbyDevices);
  const isLoading = usePairingStore((s) => s.isLoading);
  const refresh = usePairingStore((s) => s.refresh);
  const [myDevice, setMyDevice] = useState<DeviceInfo | null>(null);
  const [isEditing, setIsEditing] = useState(false);
  const [inputCodeOpen, setInputCodeOpen] = useState(false);
  const [foundDevice, setFoundDevice] = useState<{
    peerId: string;
    osInfo: { name?: string; hostname: string; os: string; platform: string; arch: string };
    code: string;
  } | null>(null);

  useEffect(() => {
    setupPairingListeners();
    refresh();
  }, [refresh]);

  useEffect(() => {
    getDeviceInfo()
      .then(setMyDevice)
      .catch(() => null);
  }, []);

  return (
    <div>
      {/* Title Row */}
      <div className="mb-6 flex items-center justify-between">
        <h1 className="text-base font-semibold tracking-tight">
          <Trans>设备管理</Trans>
        </h1>
        <Button variant="outline" size="sm" onClick={() => setInputCodeOpen(true)}>
          <Trans>输入配对码</Trans>
        </Button>
      </div>

      <div className="space-y-5">
        {/* My Device Card */}
        <div className="flex items-center gap-3 rounded-lg bg-muted/50 p-4">
          <DeviceAvatar
            os={myDevice?.os ?? ""}
            isCurrent
            className="h-10 w-10 rounded-xl [&_svg]:h-5 [&_svg]:w-5"
          />
          <div className="min-w-0 flex-1">
            {isEditing ? (
              <InlineEditName
                currentName={myDevice?.device_name ?? ""}
                onSave={async (name) => {
                  try {
                    await setDeviceName(name);
                    setMyDevice((prev) => (prev ? { ...prev, device_name: name } : prev));
                    setIsEditing(false);
                    toast.success(t`设备名称已更新，网络身份已同步`);
                  } catch {
                    toast.error(t`更新名称失败`);
                  }
                }}
                onCancel={() => setIsEditing(false)}
              />
            ) : (
              <div className="text-sm font-semibold">{myDevice?.device_name ?? "—"}</div>
            )}
            <div className="mt-0.5 text-xs text-muted-foreground">
              {myDevice
                ? `${myDevice.device_name !== myDevice.hostname ? `${myDevice.hostname} · ` : ""}${myDevice.os} · ${myDevice.platform} · `
                : "— · "}
              <Trans>当前设备</Trans>
            </div>
            {myDevice && (
              <div className="mt-0.5 text-[11px] text-muted-foreground">
                Peer ID: {myDevice.peer_id.slice(0, 12)}…{myDevice.peer_id.slice(-4)}
              </div>
            )}
          </div>
          {!isEditing && (
            <Button
              variant="outline"
              size="sm"
              onClick={() => setIsEditing(true)}
              className="shrink-0"
            >
              <Trans>编辑名称</Trans>
            </Button>
          )}
        </div>

        {/* Code Pairing */}
        <CodePairingCard />

        {/* Paired Devices Section */}
        <section className="space-y-2">
          <div className="flex items-center justify-between">
            <h2 className="text-[13px] font-medium">
              <Trans>已配对设备</Trans>
            </h2>
            {pairedDevices.length > 0 && (
              <span className="rounded-full bg-muted px-2 py-0.5 text-[11px] text-muted-foreground">
                {t`${pairedDevices.length} 台`}
              </span>
            )}
          </div>
          {pairedDevices.length > 0 ? (
            <div className="overflow-hidden rounded-lg border">
              {pairedDevices.map((device, i) => (
                <PairedDeviceCard
                  key={device.peerId}
                  device={device}
                  onUnpaired={refresh}
                  isLast={i === pairedDevices.length - 1}
                />
              ))}
            </div>
          ) : (
            <EmptyState
              icon={Link}
              title={<Trans>暂无配对设备</Trans>}
              description={<Trans>生成配对码或发现附近设备来配对</Trans>}
            />
          )}
        </section>

        {/* Nearby Devices Section */}
        <section className="space-y-2">
          <div className="flex items-center justify-between">
            <h2 className="text-[13px] font-medium">
              <Trans>附近设备</Trans>
            </h2>
            <button
              type="button"
              onClick={refresh}
              disabled={isLoading}
              className="flex items-center gap-1 rounded-md border px-2.5 py-1 text-[11px] text-muted-foreground hover:bg-muted disabled:opacity-50"
            >
              <RefreshCw className={`h-3 w-3 ${isLoading ? "animate-spin" : ""}`} />
              <Trans>刷新</Trans>
            </button>
          </div>
          {nearbyDevices.length > 0 ? (
            <div className="overflow-hidden rounded-lg border">
              {nearbyDevices.map((device, i) => (
                <NearbyDeviceCard
                  key={device.peerId}
                  device={device}
                  onPaired={refresh}
                  isLast={i === nearbyDevices.length - 1}
                />
              ))}
            </div>
          ) : (
            <EmptyState
              icon={Radio}
              title={<Trans>未发现附近设备</Trans>}
              description={<Trans>确保其他设备在同一局域网内</Trans>}
            />
          )}
        </section>
      </div>

      {/* Dialogs */}
      <InputCodeDialog
        open={inputCodeOpen}
        onOpenChange={setInputCodeOpen}
        onDeviceFound={(peerId, osInfo, code) => {
          setInputCodeOpen(false);
          setFoundDevice({ peerId, osInfo, code });
        }}
      />
      {foundDevice && (
        <FoundDeviceDialog
          open
          onOpenChange={(open) => {
            if (!open) setFoundDevice(null);
          }}
          peerId={foundDevice.peerId}
          osInfo={foundDevice.osInfo}
          code={foundDevice.code}
          onSuccess={() => {
            setFoundDevice(null);
            refresh();
          }}
        />
      )}
    </div>
  );
}

export const Route = createFileRoute("/settings/devices")({
  component: DevicesPage,
});
