import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import type { Device } from "@/commands/pairing";
import { getNearbyDevices, listDevices } from "@/commands/pairing";

interface PairingState {
  pairedDevices: Device[];
  nearbyDevices: Device[];
  isLoading: boolean;
}

interface PairingActions {
  loadPairedDevices(): Promise<void>;
  loadNearbyDevices(): Promise<void>;
  refresh(): Promise<void>;
}

export const usePairingStore = create<PairingState & PairingActions>()((set, get) => ({
  pairedDevices: [],
  nearbyDevices: [],
  isLoading: false,

  async loadPairedDevices() {
    try {
      const result = await listDevices("paired");
      set({ pairedDevices: result.devices });
    } catch (e) {
      console.error("Failed to load paired devices:", e);
    }
  },

  async loadNearbyDevices() {
    try {
      const devices = await getNearbyDevices();
      set({ nearbyDevices: devices });
    } catch (e) {
      console.error("Failed to load nearby devices:", e);
    }
  },

  async refresh() {
    set({ isLoading: true });
    await Promise.all([get().loadPairedDevices(), get().loadNearbyDevices()]);
    set({ isLoading: false });
  },
}));

let listenersSetup = false;

export function setupPairingListeners() {
  if (listenersSetup) return;
  listenersSetup = true;

  listen("paired-device-added", () => {
    usePairingStore.getState().refresh();
  });

  listen("paired-device-removed", () => {
    usePairingStore.getState().refresh();
  });

  listen("devices-changed", () => {
    usePairingStore.getState().loadNearbyDevices();
    usePairingStore.getState().loadPairedDevices();
  });
}
