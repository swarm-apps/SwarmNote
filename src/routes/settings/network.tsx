import { Trans, useLingui } from "@lingui/react/macro";
import { createFileRoute } from "@tanstack/react-router";
import { Zap } from "lucide-react";
import { NetworkStatusCard } from "@/components/settings/NetworkStatusCard";
import { SettingRow } from "@/components/settings/SettingRow";
import { Switch } from "@/components/ui/switch";
import { usePreferencesStore } from "@/stores/preferencesStore";

function NetworkSettingsPage() {
  const { t } = useLingui();
  const autoStartP2P = usePreferencesStore((s) => s.autoStartP2P);
  const setAutoStartP2P = usePreferencesStore((s) => s.setAutoStartP2P);

  return (
    <div>
      <div className="mb-6">
        <h1 className="text-base font-semibold tracking-tight">
          <Trans>网络</Trans>
        </h1>
      </div>

      <div className="space-y-5">
        {/* P2P Network Status */}
        <section className="space-y-2">
          <h2 className="text-[13px] font-medium">
            <Trans>P2P 网络</Trans>
          </h2>
          <NetworkStatusCard />
        </section>

        {/* Settings */}
        <section className="space-y-2">
          <h2 className="text-[13px] font-medium">
            <Trans>设置</Trans>
          </h2>
          <div className="overflow-hidden rounded-lg border">
            <SettingRow
              icon={Zap}
              label={t`开机自动启动网络`}
              description={t`打开工作区时自动启动 P2P 节点`}
            >
              <Switch checked={autoStartP2P} onCheckedChange={setAutoStartP2P} />
            </SettingRow>
          </div>
        </section>
      </div>
    </div>
  );
}

export const Route = createFileRoute("/settings/network")({
  component: NetworkSettingsPage,
});
