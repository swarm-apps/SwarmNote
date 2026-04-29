import { LazyStore } from "@tauri-apps/plugin-store";
import { createJSONStorage } from "zustand/middleware";

const storeCache = new Map<string, LazyStore>();

function getOrCreateStore(filename: string): LazyStore {
  let store = storeCache.get(filename);
  if (!store) {
    store = new LazyStore(filename);
    storeCache.set(filename, store);
  }
  return store;
}

export function createTauriStorage(filename: string) {
  const store = getOrCreateStore(filename);

  return createJSONStorage(() => ({
    getItem: async (key: string) => {
      const value = await store.get<string>(key);
      return value ?? null;
    },
    setItem: async (key: string, value: string) => {
      await store.set(key, value);
      await store.save();
    },
    removeItem: async (key: string) => {
      await store.delete(key);
      await store.save();
    },
  }));
}

/**
 * Listen for cross-window changes on a specific key in a Tauri store file.
 * When another window writes to the same key, the callback fires with the new value.
 */
export function onTauriStoreKeyChange<T>(
  filename: string,
  key: string,
  cb: (value: T | undefined) => void,
) {
  const store = getOrCreateStore(filename);
  // Returns a promise that resolves to an unlisten function
  return store.onKeyChange<T>(key, cb);
}

/**
 * Generic hydration guard for any Zustand persist store.
 * Resolves when the store has finished loading from persistent storage.
 */
export function waitForHydration(store: {
  persist: { hasHydrated: () => boolean; onFinishHydration: (cb: () => void) => void };
}): Promise<void> {
  return new Promise((resolve) => {
    if (store.persist.hasHydrated()) {
      resolve();
    } else {
      store.persist.onFinishHydration(() => resolve());
    }
  });
}
