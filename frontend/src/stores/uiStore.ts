import { create } from 'zustand';
import type { SystemAlertData, UiAlert } from '@/types';

interface UIState {
  sidebarCollapsed: boolean;
  currentPath: string;
  alerts: UiAlert[];
  setSidebarCollapsed: (collapsed: boolean) => void;
  setCurrentPath: (path: string) => void;
  addAlert: (alert: SystemAlertData) => void;
  markAllAlertsRead: () => void;
  dismissAlert: (id: string) => void;
}

export const useUIStore = create<UIState>((set) => ({
  sidebarCollapsed: false,
  currentPath: '/',
  alerts: [],

  setSidebarCollapsed: (collapsed) => set({ sidebarCollapsed: collapsed }),

  setCurrentPath: (path) => set({ currentPath: path }),

  addAlert: (alert) =>
    set((state) => ({
      alerts: [
        {
          ...alert,
          id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
          created_at: new Date().toISOString(),
          read: false,
        },
        ...state.alerts,
      ].slice(0, 20),
    })),

  markAllAlertsRead: () =>
    set((state) => ({
      alerts: state.alerts.map((alert) => ({ ...alert, read: true })),
    })),

  dismissAlert: (id) =>
    set((state) => ({
      alerts: state.alerts.filter((alert) => alert.id !== id),
    })),
}));
