import { Trans, useLingui } from "@lingui/react/macro";
import { listen } from "@tauri-apps/api/event";
import { Copy, Loader2, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { getDeviceByCode, requestPairing } from "@/commands/pairing";
import { NearbyDeviceCard } from "@/components/pairing/NearbyDeviceCard";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useAsyncAction } from "@/hooks/useAsyncAction";
import { useNetworkStore } from "@/stores/networkStore";
import { useOnboardingStore } from "@/stores/onboardingStore";
import { usePairingCodeStore } from "@/stores/pairingCodeStore";
import { setupPairingListeners, usePairingStore } from "@/stores/pairingStore";

type CodeMode = "idle" | "generate" | "input";

function formatSeconds(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export function PairingStep() {
  const { t } = useLingui();

  const nextStep = useOnboardingStore((s) => s.nextStep);
  const setPairedInOnboarding = useOnboardingStore((s) => s.setPairedInOnboarding);

  const networkStatus = useNetworkStore((s) => s.status);
  const networkLoading = useNetworkStore((s) => s.loading);
  const networkError = useNetworkStore((s) => s.error);

  const nearbyDevices = usePairingStore((s) => s.nearbyDevices);
  const loadNearbyDevices = usePairingStore((s) => s.loadNearbyDevices);

  // Code pairing state
  const [codeMode, setCodeMode] = useState<CodeMode>("idle");
  const [remaining, setRemaining] = useState(0);
  const [inputCode, setInputCode] = useState("");
  const { loading: codeLoading, error: codeError, run, setError, clearError } = useAsyncAction();
  const countdownRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const pollRef = useRef<number | null>(null);

  // 全局码 store —— 跨 onboarding/设置页持久化，过期/被消耗自动续生
  const codeInfo = usePairingCodeStore((s) => s.codeInfo);
  const ensureCode = usePairingCodeStore((s) => s.ensure);
  const regenerateCode = usePairingCodeStore((s) => s.regenerate);

  const clearCountdown = useCallback(() => {
    if (countdownRef.current) {
      clearInterval(countdownRef.current);
      countdownRef.current = null;
    }
  }, []);

  const resetCode = useCallback(() => {
    clearCountdown();
    setCodeMode("idle");
    setRemaining(0);
    setInputCode("");
    clearError();
    // 注意：不清 store 的 codeInfo —— 配对码全局持久化，由 store timer 管生命周期
  }, [clearCountdown, clearError]);

  // Auto-start P2P node on mount
  useEffect(() => {
    useNetworkStore.getState().startNode();
  }, []);

  // Setup pairing listeners + initial load
  useEffect(() => {
    setupPairingListeners();
    usePairingStore.getState().refresh();
  }, []);

  // Poll nearby devices every 3s (sequential to avoid overlap)
  useEffect(() => {
    let active = true;
    const poll = async () => {
      if (!active) return;
      await loadNearbyDevices();
      if (active) {
        pollRef.current = window.setTimeout(poll, 3000);
      }
    };
    pollRef.current = window.setTimeout(poll, 3000);
    return () => {
      active = false;
      if (pollRef.current) {
        clearTimeout(pollRef.current);
        pollRef.current = null;
      }
    };
  }, [loadNearbyDevices]);

  // 倒计时仅展示用；过期续生由 pairingCodeStore 内 timer 负责，UI 不主动 reset
  useEffect(() => {
    if (codeMode !== "generate" || !codeInfo) return;

    const update = () => {
      const left = Math.max(
        0,
        Math.floor((new Date(codeInfo.expiresAt).getTime() - Date.now()) / 1000),
      );
      setRemaining(left);
    };
    update();
    countdownRef.current = setInterval(update, 1000);
    return clearCountdown;
  }, [codeMode, codeInfo, clearCountdown]);

  // Listen for successful pairing → auto-advance
  useEffect(() => {
    const unlisten = listen("paired-device-added", () => {
      setPairedInOnboarding(true);
      setTimeout(() => nextStep(), 500);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [nextStep, setPairedInOnboarding]);

  async function handleGenerate() {
    clearError();
    try {
      // 首次进入生成模式：ensure（已有有效码就复用）；用户主动点"刷新"按钮：regenerate
      await (codeMode === "generate" ? regenerateCode() : ensureCode());
      setCodeMode("generate");
    } catch {
      setError(t`生成配对码失败`);
    }
  }

  async function handleInputConnect() {
    if (inputCode.length !== 6) {
      setError(t`请输入6位配对码`);
      return;
    }
    await run(async () => {
      const deviceInfo = await getDeviceByCode(inputCode);
      const resp = await requestPairing(
        deviceInfo.peerId,
        { type: "Code", code: inputCode },
        deviceInfo.osInfo,
      );
      if (resp.status !== "Success") {
        setError(resp.reason ?? t`配对被拒绝`);
      }
    });
  }

  function handleCopyCode() {
    if (codeInfo) {
      navigator.clipboard.writeText(codeInfo.code).catch(console.error);
    }
  }

  // Loading state while node starts
  if (networkLoading && networkStatus !== "running") {
    return (
      <div className="flex flex-col items-center gap-4">
        <Loader2 className="h-8 w-8 animate-spin text-primary" />
        <p className="text-sm text-muted-foreground">
          <Trans>正在启动 P2P 节点...</Trans>
        </p>
      </div>
    );
  }

  // Error state
  if (networkStatus === "error") {
    return (
      <div className="flex w-full flex-col items-center gap-4">
        <p className="text-center text-sm text-destructive">
          {networkError ?? <Trans>P2P 节点启动失败</Trans>}
        </p>
        <Button variant="outline" onClick={() => useNetworkStore.getState().startNode()}>
          <Trans>重试</Trans>
        </Button>
        <button
          type="button"
          className="text-xs text-muted-foreground underline hover:text-foreground"
          onClick={nextStep}
        >
          <Trans>跳过，稍后设置</Trans>
        </button>
      </div>
    );
  }

  return (
    <>
      <div className="flex flex-col items-center gap-2">
        <h2 className="text-xl font-bold text-foreground">
          <Trans>配对设备</Trans>
        </h2>
        <p className="text-center text-sm text-muted-foreground">
          <Trans>连接你的其他设备以同步笔记</Trans>
        </p>
      </div>

      {/* Nearby devices */}
      <div className="w-full">
        {nearbyDevices.length === 0 ? (
          <div className="flex items-center justify-center gap-2 rounded-lg border border-dashed p-4 text-sm text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" />
            <span>
              <Trans>正在搜索附近设备...</Trans>
            </span>
          </div>
        ) : (
          <div className="flex flex-col gap-2">
            {nearbyDevices.map((device) => (
              <NearbyDeviceCard key={device.peerId} device={device} />
            ))}
          </div>
        )}
      </div>

      {/* Divider */}
      <div className="flex w-full items-center gap-3">
        <div className="h-px flex-1 bg-border" />
        <span className="text-xs text-muted-foreground">
          <Trans>或使用配对码</Trans>
        </span>
        <div className="h-px flex-1 bg-border" />
      </div>

      {/* Code pairing section */}
      <div className="w-full rounded-lg border p-4">
        {codeMode === "idle" && (
          <>
            {codeError && <p className="mb-3 text-xs text-destructive">{codeError}</p>}
            <div className="flex items-center gap-3">
              <Button size="sm" onClick={handleGenerate}>
                <Trans>生成配对码</Trans>
              </Button>
              <button
                type="button"
                className="text-xs text-muted-foreground underline hover:text-foreground"
                onClick={() => {
                  clearError();
                  setCodeMode("input");
                }}
              >
                <Trans>输入配对码</Trans>
              </button>
            </div>
          </>
        )}

        {codeMode === "input" && (
          <>
            <p className="mb-3 text-xs text-muted-foreground">
              <Trans>输入对方设备生成的6位配对码</Trans>
            </p>
            {codeError && <p className="mb-3 text-xs text-destructive">{codeError}</p>}
            <div className="flex items-center gap-2">
              <Input
                value={inputCode}
                onChange={(e) => setInputCode(e.target.value.replace(/\D/g, "").slice(0, 6))}
                placeholder="000000"
                className="w-32 text-center font-mono tracking-widest"
                maxLength={6}
              />
              <Button size="sm" onClick={handleInputConnect} loading={codeLoading}>
                {codeLoading ? <Trans>连接中...</Trans> : <Trans>连接</Trans>}
              </Button>
              <button
                type="button"
                className="text-xs text-muted-foreground underline hover:text-foreground"
                onClick={resetCode}
              >
                <Trans>取消</Trans>
              </button>
            </div>
          </>
        )}

        {codeMode === "generate" && codeInfo && (
          <>
            <p className="mb-3 text-xs text-muted-foreground">
              <Trans>将此配对码告知对方设备，配对码将在 {formatSeconds(remaining)} 后过期</Trans>
            </p>
            <div className="mb-3 flex items-center gap-3">
              <div className="flex items-center gap-1 rounded-lg bg-muted px-4 py-2 font-mono text-2xl tracking-[0.3em]">
                {codeInfo.code.split("").map((digit, i) => (
                  // biome-ignore lint/suspicious/noArrayIndexKey: fixed-length code digits
                  <span key={i}>{digit}</span>
                ))}
              </div>
              <Button variant="ghost" size="icon-sm" onClick={handleCopyCode} title={t`复制配对码`}>
                <Copy className="h-4 w-4" />
              </Button>
              <Button variant="ghost" size="icon-sm" onClick={handleGenerate} title={t`刷新码`}>
                <RefreshCw className="h-4 w-4" />
              </Button>
            </div>
            <div className="flex items-center gap-3">
              <span className="text-xs text-muted-foreground">
                <Trans>等待对方连接...</Trans>
              </span>
              <button
                type="button"
                className="text-xs text-muted-foreground underline hover:text-foreground"
                onClick={resetCode}
              >
                <Trans>取消</Trans>
              </button>
            </div>
          </>
        )}
      </div>

      {/* Skip link */}
      <button
        type="button"
        className="text-xs text-muted-foreground underline hover:text-foreground"
        onClick={nextStep}
      >
        <Trans>跳过，稍后设置</Trans>
      </button>
    </>
  );
}
