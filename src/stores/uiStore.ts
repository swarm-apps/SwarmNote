import { create } from "zustand";
import { persist } from "zustand/middleware";
import { activateLocale, detectLocale, type Locale } from "@/i18n";
import { createTauriStorage, onTauriStoreKeyChange, waitForHydration } from "@/lib/tauriStore";

type Theme = "light" | "dark" | "system";
export type SidebarTab = "filetree" | "outline";

export const SIDEBAR_WIDTH_MIN = 200;
export const SIDEBAR_WIDTH_MAX = 480;
export const SIDEBAR_WIDTH_DEFAULT = 256;

interface UIState {
  sidebarOpen: boolean;
  /** Persisted width of the sidebar (before collapse). Clamped to [200, 480]. */
  sidebarWidth: number;
  /** Which panel is shown in the sidebar content area. */
  sidebarTab: SidebarTab;
  /** When true, editor content is constrained to a comfortable reading width. */
  readableLineLength: boolean;
  workspacePickerOpen: boolean;
  theme: Theme;
  resolvedTheme: "light" | "dark";
  locale: Locale;
}

interface UIActions {
  toggleSidebar: () => void;
  setSidebarOpen: (open: boolean) => void;
  setSidebarWidth: (width: number) => void;
  setSidebarTab: (tab: SidebarTab) => void;
  setReadableLineLength: (enabled: boolean) => void;
  setWorkspacePickerOpen: (open: boolean) => void;
  setTheme: (theme: Theme) => void;
  setLocale: (locale: Locale) => void;
}

function getSystemTheme(): "light" | "dark" {
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

function applyTheme(resolved: "light" | "dark") {
  document.documentElement.classList.toggle("dark", resolved === "dark");
}

export const useUIStore = create<UIState & UIActions>()(
  persist(
    (set) => ({
      sidebarOpen: true,
      sidebarWidth: SIDEBAR_WIDTH_DEFAULT,
      sidebarTab: "filetree" as SidebarTab,
      readableLineLength: true,
      workspacePickerOpen: false,
      theme: "light",
      resolvedTheme: "light",
      locale: detectLocale(),

      toggleSidebar: () => set((s) => ({ sidebarOpen: !s.sidebarOpen })),

      setSidebarOpen: (open) => set({ sidebarOpen: open }),

      setSidebarWidth: (width) => {
        const clamped = Math.max(SIDEBAR_WIDTH_MIN, Math.min(SIDEBAR_WIDTH_MAX, width));
        // Skip the set when the clamped value hasn't changed — avoids firing
        // subscribers on every mousemove once the drag goes past the clamp.
        if (clamped === useUIStore.getState().sidebarWidth) return;
        set({ sidebarWidth: clamped });
      },

      setSidebarTab: (tab) => set({ sidebarTab: tab }),

      setReadableLineLength: (enabled) => set({ readableLineLength: enabled }),

      setWorkspacePickerOpen: (open) => set({ workspacePickerOpen: open }),

      setTheme: (theme) => {
        const resolved = theme === "system" ? getSystemTheme() : theme;
        applyTheme(resolved);
        set({ theme, resolvedTheme: resolved });
      },

      setLocale: (locale) => {
        activateLocale(locale).then(
          () => set({ locale }),
          (err) => console.error("Failed to activate locale:", err),
        );
      },
    }),
    {
      name: "swarmnote-ui",
      storage: createTauriStorage("settings.json"),
      partialize: (state) => ({
        sidebarOpen: state.sidebarOpen,
        sidebarWidth: state.sidebarWidth,
        sidebarTab: state.sidebarTab,
        readableLineLength: state.readableLineLength,
        theme: state.theme,
        locale: state.locale,
      }),
      onRehydrateStorage: () => (state) => {
        if (state) {
          const resolved = state.theme === "system" ? getSystemTheme() : state.theme;
          applyTheme(resolved);
          state.resolvedTheme = resolved;
          activateLocale(state.locale).catch(console.error);
        }
      },
    },
  ),
);

export const waitForUiHydration = () => waitForHydration(useUIStore);

// Listen to system theme changes when in "system" mode
window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
  const { theme, setTheme } = useUIStore.getState();
  if (theme === "system") {
    setTheme("system");
  }
});

// Sync theme/locale from other windows via Tauri plugin-store cross-window events
onTauriStoreKeyChange<string>("settings.json", "swarmnote-ui", (raw) => {
  if (!raw) return;
  try {
    const persisted = JSON.parse(raw) as { state?: { theme?: Theme; locale?: Locale } };
    const incoming = persisted.state;
    if (!incoming) return;
    const current = useUIStore.getState();
    if (incoming.theme && incoming.theme !== current.theme) {
      current.setTheme(incoming.theme);
    }
    if (incoming.locale && incoming.locale !== current.locale) {
      current.setLocale(incoming.locale);
    }
  } catch {
    // Ignore parse errors
  }
});
