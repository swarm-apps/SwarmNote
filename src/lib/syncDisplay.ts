import type { SyncDisplayState } from "@/hooks/useSyncDisplayState";
import type { NodeStatus } from "@/stores/networkStore";

export type SyncDot = "gray" | "yellow" | "yellow-pulse" | "green" | "red";

/**
 * Pure labels produced by `computeSyncDisplay`. Each variant carries enough
 * context for the caller to translate it via Lingui without losing structure.
 * The component layer is responsible for turning these into final strings.
 */
export type SyncDisplayLabel =
  | { kind: "connecting" }
  | { kind: "error" }
  | { kind: "stopped" }
  | { kind: "waiting-for-peers" }
  | { kind: "syncing"; peerCount: number; completed: number; total: number }
  | { kind: "synced"; peerCount: number }
  | { kind: "online"; peerCount: number };

export interface SyncDisplayInput {
  nodeLoading: boolean;
  nodeStatus: NodeStatus;
  connectedPeerCount: number;
  syncState: SyncDisplayState;
}

export interface SyncDisplayResult {
  dot: SyncDot;
  label: SyncDisplayLabel;
}

/**
 * Deterministic mapping from network + sync state to (dot color, label variant).
 *
 * The ordering of branches here IS the contract — every tuple of inputs
 * resolves to exactly one output. See openspec specs/sync-status-indicator.
 */
export function computeSyncDisplay(input: SyncDisplayInput): SyncDisplayResult {
  const { nodeLoading, nodeStatus, connectedPeerCount, syncState } = input;

  if (nodeLoading) {
    return { dot: "yellow-pulse", label: { kind: "connecting" } };
  }

  if (nodeStatus === "error") {
    return { dot: "red", label: { kind: "error" } };
  }

  if (nodeStatus !== "running") {
    return { dot: "gray", label: { kind: "stopped" } };
  }

  if (connectedPeerCount === 0) {
    return { dot: "yellow", label: { kind: "waiting-for-peers" } };
  }

  // Node running, peers connected. Now decide on sync overlay.
  // Edge case: syncState.total === 0 must not produce "0/0" — fall back to idle.
  if (
    syncState.status === "syncing" &&
    typeof syncState.total === "number" &&
    syncState.total > 0
  ) {
    return {
      dot: "green",
      label: {
        kind: "syncing",
        peerCount: connectedPeerCount,
        completed: syncState.completed ?? 0,
        total: syncState.total,
      },
    };
  }

  if (syncState.status === "synced") {
    return {
      dot: "green",
      label: { kind: "synced", peerCount: connectedPeerCount },
    };
  }

  return {
    dot: "green",
    label: { kind: "online", peerCount: connectedPeerCount },
  };
}

/** Maps a `SyncDot` value to its Tailwind class string. */
export function syncDotClass(dot: SyncDot): string {
  switch (dot) {
    case "gray":
      return "bg-gray-400";
    case "yellow":
      return "bg-yellow-500";
    case "yellow-pulse":
      return "animate-pulse bg-yellow-500";
    case "green":
      return "bg-green-500";
    case "red":
      return "bg-red-500";
  }
}
