import { create } from "zustand";

import type { PairingRequestReceived, ShareInvitationReceived } from "@/lib/bindings";

// 需要用户决策的通知(顺序队列,一次一个对话框)。
export type ActionNotification = {
  id: string;
  timestamp: number;
} & (
  | { type: "pairing-request"; payload: PairingRequestReceived }
  | { type: "share-invitation"; payload: ShareInvitationReceived }
);

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
