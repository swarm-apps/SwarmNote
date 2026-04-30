import { Trans } from "@lingui/react/macro";
import { RefreshCw, Shield, Wifi } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useOnboardingStore } from "@/stores/onboardingStore";

export function WelcomeStep() {
  const nextStep = useOnboardingStore((s) => s.nextStep);

  return (
    <>
      <img src="/logo-64.png" alt="SwarmNote" width={64} height={64} className="h-16 w-16" />

      <div className="flex flex-col items-center gap-2">
        <h1 className="text-2xl font-bold text-foreground">SwarmNote</h1>
        <p className="text-center text-sm text-muted-foreground">
          <Trans>去中心化、本地优先的笔记应用，通过 P2P 网络在设备间安全同步。</Trans>
        </p>
      </div>

      <div className="flex gap-6">
        {(
          [
            { icon: RefreshCw, label: <Trans>多端协作</Trans> },
            { icon: Shield, label: <Trans>安全加密</Trans> },
            { icon: Wifi, label: <Trans>P2P 同步</Trans> },
          ] as const
        ).map(({ icon: Icon, label }, i) => (
          // biome-ignore lint/suspicious/noArrayIndexKey: static list with fixed order
          <div key={i} className="flex flex-col items-center gap-1.5">
            <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-muted">
              <Icon className="h-5 w-5 text-muted-foreground" />
            </div>
            <span className="text-xs text-muted-foreground">{label}</span>
          </div>
        ))}
      </div>

      <Button className="w-full" size="lg" onClick={nextStep}>
        <Trans>开始使用</Trans>
      </Button>
    </>
  );
}
