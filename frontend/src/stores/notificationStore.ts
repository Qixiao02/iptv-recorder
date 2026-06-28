import { create } from 'zustand';
import type { AppNotification } from '@/types';
import {
  getUnreadCount,
  markNotificationRead,
  markAllNotificationsRead,
  deleteNotification,
} from '@/api/notifications';

interface NotificationState {
  /** 未读数量（持久化） */
  unreadCount: number;
  /** 是否正在加载未读数 */
  loadingCount: boolean;
  /** 从后端拉取未读数 */
  fetchUnreadCount: () => Promise<void>;
  /** 收到实时 WebSocket 推送时，把一条新通知置顶并自增未读 */
  onNotificationReceived: (n: AppNotification) => void;
  /** 标记单条已读（本地 + 后端） */
  markRead: (id: string) => Promise<void>;
  /** 全部已读（本地 + 后端） */
  markAllRead: () => Promise<void>;
  /** 删除单条（本地未读数同步） */
  remove: (id: string) => Promise<void>;
  /** 重置（登出/切换用户） */
  reset: () => void;
}

export const useNotificationStore = create<NotificationState>((set, get) => ({
  unreadCount: 0,
  loadingCount: false,

  fetchUnreadCount: async () => {
    set({ loadingCount: true });
    try {
      const { count } = await getUnreadCount();
      set({ unreadCount: count });
    } catch {
      // 静默失败，不打扰用户
    } finally {
      set({ loadingCount: false });
    }
  },

  onNotificationReceived: (n) => {
    if (!n.read) {
      set((state) => ({ unreadCount: state.unreadCount + 1 }));
    }
  },

  markRead: async (id) => {
    const prev = get().unreadCount;
    // 乐观更新：先减再请求，失败则回滚
    set({ unreadCount: Math.max(0, prev - 1) });
    try {
      await markNotificationRead(id);
    } catch {
      set({ unreadCount: prev });
    }
  },

  markAllRead: async () => {
    set({ unreadCount: 0 });
    try {
      await markAllNotificationsRead();
    } catch {
      // 失败则重新拉取真实未读数
      await get().fetchUnreadCount();
    }
  },

  remove: async (id) => {
    // 删除前无法确定是否未读，删除后统一刷新真实值
    try {
      await deleteNotification(id);
      await get().fetchUnreadCount();
    } catch {
      // 静默
    }
  },

  reset: () => set({ unreadCount: 0, loadingCount: false }),
}));
