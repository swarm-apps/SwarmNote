import { create } from "zustand";

import type { PairingRequestReceived } from "@/lib/bindings";

export type ActionNotification = {
  id: string;
  timestamp: number;
} & { type: "pairing-request"; payload: PairingRequestReceived };

interface NotificationState {
  queue: ActionNotification[];
  current: ActionNotification | null;
}

interface NotificationActions {
  push(notification: ActionNotification): void;
  respond(id: string): void;
  dismiss(id: string): void;
}

export const useNotificationStore = create<NotificationState & NotificationActions>()(
  (set, get) => ({
    queue: [],
    current: null,

    push(notification) {
      const { current } = get();
      if (current === null) {
        set({ current: notification });
      } else {
        set((state) => ({ queue: [...state.queue, notification] }));
      }
    },

    respond(id) {
      const { current, queue } = get();
      if (current?.id === id) {
        const [next, ...rest] = queue;
        set({ current: next ?? null, queue: rest });
      }
    },

    dismiss(id) {
      get().respond(id);
    },
  }),
);
