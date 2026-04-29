import { Trans, useLingui } from "@lingui/react/macro";
import { Copy, KeyRound, Timer, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import type { PairingCodeInfo } from "@/commands/pairing";
import { generatePairingCode } from "@/commands/pairing";
import { Button } from "@/components/ui/button";

function formatSeconds(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export function CodePairingCard() {
  const { t } = useLingui();
  const [codeInfo, setCodeInfo] = useState<PairingCodeInfo | null>(null);
  const [remaining, setRemaining] = useState(0);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const clearTimer = useCallback(() => {
    if (intervalRef.current) {
      clearInterval(intervalRef.current);
      intervalRef.current = null;
    }
  }, []);

  const reset = useCallback(() => {
    clearTimer();
    setCodeInfo(null);
    setRemaining(0);
  }, [clearTimer]);

  useEffect(() => {
    if (!codeInfo) return;

    const updateRemaining = () => {
      const left = Math.max(
        0,
        Math.floor((new Date(codeInfo.expiresAt).getTime() - Date.now()) / 1000),
      );
      setRemaining(left);
      if (left <= 0) reset();
    };

    updateRemaining();
    intervalRef.current = setInterval(updateRemaining, 1000);
    return clearTimer;
  }, [codeInfo, clearTimer, reset]);

  async function handleGenerate() {
    try {
      setCodeInfo(await generatePairingCode(300));
    } catch {
      toast.error(t`生成配对码失败`);
    }
  }

  function handleCopy() {
    if (!codeInfo) return;
    navigator.clipboard.writeText(codeInfo.code).catch(console.error);
    toast.success(t`配对码已复制`);
  }

  if (codeInfo) {
    return (
      <div className="group/code relative flex min-h-15 items-center justify-between rounded-lg border border-primary bg-primary/5 px-3.5 py-2.5">
        <button
          type="button"
          onClick={reset}
          className="absolute -top-2 -right-2 flex h-5 w-5 items-center justify-center rounded-full bg-muted text-muted-foreground/70 opacity-0 shadow-sm transition-all hover:bg-destructive/15 hover:text-destructive group-hover/code:opacity-100"
          title={t`关闭`}
        >
          <X className="h-3 w-3" />
        </button>
        <div className="space-y-1">
          <div className="flex h-4 items-center gap-1.5">
            <KeyRound className="h-3.5 w-3.5 text-primary" />
            <span className="text-xs font-semibold leading-none text-primary">
              <Trans>配对码</Trans>
            </span>
            <span className="font-mono text-sm font-bold leading-none tracking-[0.15em]">
              {codeInfo.code}
            </span>
          </div>
          <div className="flex items-center gap-1 text-[11px] text-muted-foreground">
            <Timer className="h-3 w-3" />
            <span>
              <Trans>{formatSeconds(remaining)} 后过期</Trans>
            </span>
            <span>·</span>
            <span>
              <Trans>在另一台设备输入此码</Trans>
            </span>
          </div>
        </div>
        <button
          type="button"
          onClick={handleCopy}
          className="flex shrink-0 items-center gap-1 rounded border px-2 py-1 text-[11px] text-muted-foreground hover:bg-muted"
          title={t`复制`}
        >
          <Copy className="h-3 w-3" />
          <Trans>复制</Trans>
        </button>
      </div>
    );
  }

  return (
    <div className="flex min-h-15 items-center justify-between rounded-lg border bg-muted/50 px-3.5 py-2.5">
      <div className="space-y-1">
        <div className="flex h-4 items-center gap-1.5">
          <KeyRound className="h-3.5 w-3.5 text-muted-foreground" />
          <span className="text-xs font-semibold leading-none">
            <Trans>配对码</Trans>
          </span>
        </div>
        <p className="text-[11px] text-muted-foreground">
          <Trans>生成 6 位配对码，在另一台设备输入即可配对</Trans>
        </p>
      </div>
      <Button size="sm" onClick={handleGenerate}>
        <Trans>生成</Trans>
      </Button>
    </div>
  );
}
