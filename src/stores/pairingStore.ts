import { create } from "zustand";
import { commands, type Device, events } from "@/lib/bindings";

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
      const result = await commands.listDevices("paired");
      set({ pairedDevices: result.devices });
    } catch (e) {
      console.error("Failed to load paired devices:", e);
    }
  },

  async loadNearbyDevices() {
    try {
      const devices = await commands.getNearbyDevices();
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

  events.pairedDeviceAdded.listen(() => {
    usePairingStore.getState().refresh();
  });

  events.pairedDeviceRemoved.listen(() => {
    usePairingStore.getState().refresh();
  });

  events.devicesChanged.listen(() => {
    usePairingStore.getState().loadNearbyDevices();
    usePairingStore.getState().loadPairedDevices();
  });
}
