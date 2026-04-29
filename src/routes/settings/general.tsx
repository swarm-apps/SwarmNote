import { Trans, useLingui } from "@lingui/react/macro";
import { createFileRoute } from "@tanstack/react-router";
import { FolderOpen, Globe, Palette, WrapText } from "lucide-react";
import { SettingRow } from "@/components/settings/SettingRow";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import type { Locale } from "@/i18n";
import { usePreferencesStore } from "@/stores/preferencesStore";
import { useUIStore } from "@/stores/uiStore";

function GeneralSettingsPage() {
  const { t } = useLingui();
  const theme = useUIStore((s) => s.theme);
  const locale = useUIStore((s) => s.locale);
  const setTheme = useUIStore((s) => s.setTheme);
  const setLocale = useUIStore((s) => s.setLocale);

  const readableLineLength = useUIStore((s) => s.readableLineLength);
  const setReadableLineLength = useUIStore((s) => s.setReadableLineLength);

  const restoreLastWorkspace = usePreferencesStore((s) => s.restoreLastWorkspace);
  const setRestoreLastWorkspace = usePreferencesStore((s) => s.setRestoreLastWorkspace);

  return (
    <div>
      <div className="mb-6">
        <h1 className="text-base font-semibold tracking-tight">
          <Trans>通用</Trans>
        </h1>
      </div>

      <div className="space-y-5">
        {/* Appearance Section */}
        <section className="space-y-2">
          <h2 className="text-[13px] font-medium">
            <Trans>外观</Trans>
          </h2>
          <div className="overflow-hidden rounded-lg border">
            <div className="border-b">
              <SettingRow icon={Globe} label={t`语言`} description={t`选择界面显示语言`}>
                <Select value={locale} onValueChange={(v) => setLocale(v as Locale)}>
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="zh">{t`中文`}</SelectItem>
                    <SelectItem value="en">English</SelectItem>
                  </SelectContent>
                </Select>
              </SettingRow>
            </div>
            <div className="border-b">
              <SettingRow icon={Palette} label={t`外观`} description={t`选择明亮或暗色主题`}>
                <Select
                  value={theme}
                  onValueChange={(v) => setTheme(v as "light" | "dark" | "system")}
                >
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="light">{t`浅色`}</SelectItem>
                    <SelectItem value="dark">{t`深色`}</SelectItem>
                    <SelectItem value="system">{t`跟随系统`}</SelectItem>
                  </SelectContent>
                </Select>
              </SettingRow>
            </div>
            <SettingRow
              icon={WrapText}
              label={t`可读行宽`}
              description={t`限制编辑器内容宽度以提升阅读体验`}
            >
              <Switch checked={readableLineLength} onCheckedChange={setReadableLineLength} />
            </SettingRow>
          </div>
        </section>

        {/* Startup Section */}
        <section className="space-y-2">
          <h2 className="text-[13px] font-medium">
            <Trans>启动行为</Trans>
          </h2>
          <div className="overflow-hidden rounded-lg border">
            <SettingRow
              icon={FolderOpen}
              label={t`恢复上次工作区`}
              description={t`启动时自动打开上次使用的工作区`}
            >
              <Switch checked={restoreLastWorkspace} onCheckedChange={setRestoreLastWorkspace} />
            </SettingRow>
          </div>
        </section>
      </div>
    </div>
  );
}

export const Route = createFileRoute("/settings/general")({
  component: GeneralSettingsPage,
});
