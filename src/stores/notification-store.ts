import { create } from "zustand";

export type NotificationKind = "account_live" | "recording_finished" | "clip_ready" | "info";

export interface AppNotification {
  id: string;
  kind: NotificationKind;
  title: string;
  body: string;
  read: boolean;
  createdAt: number;
}

const MAX_QUEUE = 200;

function newId(): string {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 11)}`;
}

type NotificationStoreState = {
  items: AppNotification[];
  addNotification: (input: {
    kind: NotificationKind;
    title: string;
    body?: string;
  }) => string;
  markRead: (id: string) => void;
  markAllRead: () => void;
  /** Current number of unread notifications (queue snapshot). */
  getUnreadCount: () => number;
};

export const useNotificationStore = create<NotificationStoreState>((set, get) => ({
  items: [],

  getUnreadCount: () => selectUnreadCount(get().items),

  addNotification: (input) => {
    const id = newId();
    const row: AppNotification = {
      id,
      kind: input.kind,
      title: input.title,
      body: input.body ?? "",
      read: false,
      createdAt: Date.now(),
    };
    set((s) => {
      const next = [row, ...s.items];
      if (next.length > MAX_QUEUE) {
        next.length = MAX_QUEUE;
      }
      return { items: next };
    });
    return id;
  },

  markRead: (id) =>
    set((s) => ({
      items: s.items.map((n) => (n.id === id ? { ...n, read: true } : n)),
    })),

  markAllRead: () =>
    set((s) => ({
      items: s.items.map((n) => ({ ...n, read: true })),
    })),
}));

export function selectUnreadCount(items: AppNotification[]): number {
  return items.filter((n) => !n.read).length;
}

export function useUnreadNotificationCount(): number {
  return useNotificationStore((s) => selectUnreadCount(s.items));
}
