import { Trans } from "@lingui/react/macro";
import { CheckCircle, Fingerprint, Monitor } from "lucide-react";
import { useEffect, useState } from "react";
import { getDeviceInfo } from "@/commands/identity";
import { finishOnboarding } from "@/commands/workspace";
import { Button } from "@/components/ui/button";
import { useOnboardingStore } from "@/stores/onboardingStore";

export function CompleteStep() {
  const complete = useOnboardingStore((s) => s.complete);
  const userPath = useOnboardingStore((s) => s.userPath);
  const pairedInOnboarding = useOnboardingStore((s) => s.pairedInOnboarding);
  const [deviceName, setDeviceName] = useState("");
  const [peerId, setPeerId] = useState("");

  useEffect(() => {
    getDeviceInfo().then((info) => {
      setDeviceName(info.device_name);
      setPeerId(info.peer_id.slice(0, 8));
    });
  }, []);

  async function handleFinish() {
    // 先持久化 onboarding 完成状态，再由 Rust 端处理窗口切换
    complete();
    await finishOnboarding();
  }

  const isAddDevice = userPath === "add-device";

  const title =
    isAddDevice && pairedInOnboarding ? <Trans>配对成功！</Trans> : <Trans>准备就绪!</Trans>;

  const subtitle =
    isAddDevice && pairedInOnboarding ? (
      <Trans>你可以在工作区管理窗口中选择要同步的工作区</Trans>
    ) : isAddDevice && !pairedInOnboarding ? (
      <Trans>你可以稍后在设置 → 设备中配对设备</Trans>
    ) : (
      <Trans>你的设备身份已建立，可以开始使用 SwarmNote 了。</Trans>
    );

  return (
    <>
      <div className="flex h-16 w-16 items-center justify-center rounded-2xl bg-green-100">
        <CheckCircle className="h-8 w-8 text-green-600" />
      </div>

      <div className="flex flex-col items-center gap-2">
        <h2 className="text-xl font-bold text-foreground">{title}</h2>
        <p className="text-center text-sm text-muted-foreground">{subtitle}</p>
      </div>

      <div className="flex w-full flex-col gap-3 rounded-lg border border-border bg-muted/50 p-4">
        <div className="flex items-center gap-3">
          <Monitor className="h-4 w-4 text-muted-foreground" />
          <div className="flex flex-col">
            <span className="text-xs text-muted-foreground">
              <Trans>设备名称</Trans>
            </span>
            <span className="text-sm font-medium text-foreground">{deviceName}</span>
          </div>
        </div>
        <div className="h-px bg-border" />
        <div className="flex items-center gap-3">
          <Fingerprint className="h-4 w-4 text-muted-foreground" />
          <div className="flex flex-col">
            <span className="text-xs text-muted-foreground">
              <Trans>设备 ID</Trans>
            </span>
            <span className="font-mono text-sm text-foreground">{peerId}</span>
          </div>
        </div>
      </div>

      <Button className="w-full" size="lg" onClick={handleFinish}>
        <Trans>进入 SwarmNote</Trans>
      </Button>
    </>
  );
}
