import { create } from "zustand";
import { persist } from "zustand/middleware";
import { createTauriStorage, waitForHydration } from "@/lib/tauriStore";

/**
 * 内置编辑器 plugin id 集合。与 `@swarmnote/editor-core/plugins/<name>` 对应。
 *
 * 顺序即为 settings UI 的展示顺序。
 */
export const EDITOR_PLUGIN_IDS = [
  "math",
  "table",
  "mermaid",
  "admonition",
  "codeBlock",
  "blockImage",
  "rawHtml",
  "smartPaste",
  "slash",
  "wikilink",
  "selectionToolbar",
] as const;

export type EditorPluginId = (typeof EDITOR_PLUGIN_IDS)[number];

/**
 * codeBlock plugin 的渲染模式。`off` 通过 `enabledPlugins` 排除 codeBlock 表达，
 * 故此处仅含三个真正"启用"语义的模式。
 */
export type CodeBlockPluginMode = "inline" | "auto" | "toggle";

interface PreferencesState {
  autoStartP2P: boolean;
  restoreLastWorkspace: boolean;
  /** 启用的编辑器 plugin id 集合（与 EDITOR_PLUGIN_IDS 对应） */
  enabledPlugins: EditorPluginId[];
  /** codeBlock plugin 启用时的渲染模式 */
  codeBlockMode: CodeBlockPluginMode;
}

interface PreferencesActions {
  setAutoStartP2P: (value: boolean) => void;
  setRestoreLastWorkspace: (value: boolean) => void;
  setPluginEnabled: (id: EditorPluginId, enabled: boolean) => void;
  setCodeBlockMode: (mode: CodeBlockPluginMode) => void;
}

const DEFAULT_ENABLED_PLUGINS: EditorPluginId[] = [...EDITOR_PLUGIN_IDS];
const DEFAULT_CODE_BLOCK_MODE: CodeBlockPluginMode = "inline";

/**
 * v0.0.x → v0.1 持久化迁移：把旧 `features.*` 键翻译为 `enabledPlugins` /
 * `codeBlockMode`，并清掉旧键。
 *
 * v0.0.x 的 SwarmNote host 并未把 editor `features` 写入 preferencesStore，
 * 但 spec 要求 migration step 存在以处理理论上可能的 stale storage。
 * 这里在 hydration 时无副作用地探测旧字段——找到才翻译，找不到不修改。
 */
function migrateLegacyFeatures(state: unknown): Partial<PreferencesState> {
  if (!state || typeof state !== "object") return {};
  const raw = state as Record<string, unknown>;
  const legacy = raw.features as Record<string, unknown> | undefined;
  if (!legacy || typeof legacy !== "object") return {};

  const enabledSet = new Set<EditorPluginId>(DEFAULT_ENABLED_PLUGINS);
  let codeBlockMode: CodeBlockPluginMode | undefined;

  const trySet = (key: string, id: EditorPluginId) => {
    if (key in legacy) {
      if (legacy[key] === false) enabledSet.delete(id);
      else enabledSet.add(id);
    }
  };
  trySet("mathRendering", "math");
  trySet("mermaidRendering", "mermaid");
  trySet("blockImageRendering", "blockImage");
  trySet("rawHtmlRendering", "rawHtml");
  trySet("smartPaste", "smartPaste");
  trySet("admonition", "admonition");
  // table 历史上与 blockImageRendering 同生共死，这里复用 blockImageRendering 的取值
  if ("blockImageRendering" in legacy) {
    if (legacy.blockImageRendering === false) enabledSet.delete("table");
    else enabledSet.add("table");
  }
  if ("codeBlockMode" in legacy) {
    const mode = legacy.codeBlockMode;
    if (mode === "off") enabledSet.delete("codeBlock");
    else if (mode === "inline" || mode === "auto" || mode === "toggle") {
      enabledSet.add("codeBlock");
      codeBlockMode = mode;
    }
  }

  // 删除旧 features 键，避免下次启动重复 migrate
  delete raw.features;

  return {
    enabledPlugins: EDITOR_PLUGIN_IDS.filter((id) => enabledSet.has(id)),
    ...(codeBlockMode ? { codeBlockMode } : {}),
  };
}

export const usePreferencesStore = create<PreferencesState & PreferencesActions>()(
  persist(
    (set) => ({
      autoStartP2P: true,
      restoreLastWorkspace: true,
      enabledPlugins: DEFAULT_ENABLED_PLUGINS,
      codeBlockMode: DEFAULT_CODE_BLOCK_MODE,

      setAutoStartP2P: (value: boolean) => set({ autoStartP2P: value }),
      setRestoreLastWorkspace: (value: boolean) => set({ restoreLastWorkspace: value }),
      setPluginEnabled: (id: EditorPluginId, enabled: boolean) =>
        set((state) => {
          const has = state.enabledPlugins.includes(id);
          if (enabled && !has) {
            return {
              enabledPlugins: EDITOR_PLUGIN_IDS.filter(
                (pid) => pid === id || state.enabledPlugins.includes(pid),
              ),
            };
          }
          if (!enabled && has) {
            return { enabledPlugins: state.enabledPlugins.filter((pid) => pid !== id) };
          }
          return state;
        }),
      setCodeBlockMode: (mode: CodeBlockPluginMode) => set({ codeBlockMode: mode }),
    }),
    {
      name: "swarmnote-preferences",
      storage: createTauriStorage("settings.json"),
      version: 1,
      // Migration runs on hydration; the returned object is shallow-merged into
      // the rehydrated state. Idempotent: legacy keys are deleted after first run.
      migrate: (persistedState, _version) => {
        const migrated = migrateLegacyFeatures(persistedState);
        return {
          ...(persistedState as PreferencesState),
          ...migrated,
        } as PreferencesState & PreferencesActions;
      },
    },
  ),
);

export const waitForPreferencesHydration = () => waitForHydration(usePreferencesStore);
