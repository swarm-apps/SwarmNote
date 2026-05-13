import { Trans, useLingui } from "@lingui/react/macro";
import { createFileRoute } from "@tanstack/react-router";
import type { LucideIcon } from "lucide-react";
import {
  AlertCircle,
  Code2,
  Command,
  FolderOpen,
  Globe,
  Image as ImageIcon,
  Link as LinkIcon,
  MousePointer,
  Palette,
  Puzzle,
  Sigma,
  Sparkles,
  Table as TableIcon,
  Wand2,
  WrapText,
} from "lucide-react";
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
import {
  type CodeBlockPluginMode,
  type EditorPluginId,
  usePreferencesStore,
} from "@/stores/preferencesStore";
import { useUIStore } from "@/stores/uiStore";

interface PluginRowMeta {
  id: EditorPluginId;
  icon: LucideIcon;
  label: string;
  description: string;
}

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

  const enabledPlugins = usePreferencesStore((s) => s.enabledPlugins);
  const setPluginEnabled = usePreferencesStore((s) => s.setPluginEnabled);
  const codeBlockMode = usePreferencesStore((s) => s.codeBlockMode);
  const setCodeBlockMode = usePreferencesStore((s) => s.setCodeBlockMode);

  const isPluginEnabled = (id: EditorPluginId) => enabledPlugins.includes(id);

  // i18n-keyed plugin metadata; order drives row order.
  const pluginRows: PluginRowMeta[] = [
    {
      id: "math",
      icon: Sigma,
      label: t`数学公式`,
      description: t`渲染 KaTeX 数学公式 ($...$ / $$...$$)`,
    },
    {
      id: "table",
      icon: TableIcon,
      label: t`表格`,
      description: t`管道表格渲染为可视化卡片`,
    },
    {
      id: "mermaid",
      icon: Sparkles,
      label: t`Mermaid 图表`,
      description: t`把 mermaid 代码块渲染为 SVG`,
    },
    {
      id: "admonition",
      icon: AlertCircle,
      label: t`Admonition`,
      description: t`渲染 GFM / Obsidian 风格的提示块`,
    },
    {
      id: "codeBlock",
      icon: Code2,
      label: t`代码块`,
      description: t`fenced 代码块渲染与高亮`,
    },
    {
      id: "blockImage",
      icon: ImageIcon,
      label: t`图片渲染`,
      description: t`将 Markdown 图片渲染为内联 / 块级 widget`,
    },
    {
      id: "rawHtml",
      icon: Wand2,
      label: t`HTML 渲染`,
      description: t`通过 DOMPurify 渲染嵌入的原生 HTML`,
    },
    {
      id: "smartPaste",
      icon: Puzzle,
      label: t`智能粘贴`,
      description: t`粘贴 URL 转链接，拖放 / 粘贴文件上传为图片`,
    },
    {
      id: "slash",
      icon: Command,
      label: t`Slash 命令`,
      description: t`输入 / 触发候选菜单，快速插入或跳转笔记`,
    },
    {
      id: "wikilink",
      icon: LinkIcon,
      label: t`Wikilink`,
      description: t`输入 [[ 触发笔记选择，插入 [[note-title]] 链接`,
    },
    {
      id: "selectionToolbar",
      icon: MousePointer,
      label: t`Selection 工具栏`,
      description: t`选中文字时浮出格式化工具栏（粗体 / 斜体 / 链接 等）`,
    },
  ];

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

        {/* Editor plugins section */}
        <section className="space-y-2">
          <h2 className="text-[13px] font-medium">
            <Trans>编辑器插件</Trans>
          </h2>
          <p className="text-xs text-muted-foreground">
            <Trans>切换插件启用状态后,需要重新打开文档或重启应用以生效。</Trans>
          </p>
          <div className="overflow-hidden rounded-lg border">
            {pluginRows.map((row, idx) => (
              <div key={row.id} className={idx < pluginRows.length - 1 ? "border-b" : undefined}>
                <SettingRow icon={row.icon} label={row.label} description={row.description}>
                  {row.id === "codeBlock" ? (
                    <div className="flex items-center gap-2">
                      <Select
                        value={codeBlockMode}
                        onValueChange={(v) => setCodeBlockMode(v as CodeBlockPluginMode)}
                        disabled={!isPluginEnabled("codeBlock")}
                      >
                        <SelectTrigger>
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="inline">{t`内联`}</SelectItem>
                          <SelectItem value="auto">{t`自动`}</SelectItem>
                          <SelectItem value="toggle">{t`切换`}</SelectItem>
                        </SelectContent>
                      </Select>
                      <Switch
                        checked={isPluginEnabled(row.id)}
                        onCheckedChange={(v) => setPluginEnabled(row.id, v)}
                      />
                    </div>
                  ) : (
                    <Switch
                      checked={isPluginEnabled(row.id)}
                      onCheckedChange={(v) => setPluginEnabled(row.id, v)}
                    />
                  )}
                </SettingRow>
              </div>
            ))}
          </div>
        </section>
      </div>
    </div>
  );
}

export const Route = createFileRoute("/settings/general")({
  component: GeneralSettingsPage,
});
