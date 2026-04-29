import { getVersion } from "@tauri-apps/api/app";
import { useEffect, useState } from "react";

/**
 * Returns the current app version from Tauri at runtime.
 * Returns `null` until resolved to avoid rendering a placeholder / stale value.
 */
export function useAppVersion(): string | null {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    getVersion()
      .then((v) => {
        if (!cancelled) setVersion(v);
      })
      .catch((err) => console.error("Failed to get app version:", err));
    return () => {
      cancelled = true;
    };
  }, []);

  return version;
}
