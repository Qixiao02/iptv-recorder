import { create } from 'zustand';

interface UIState {
  sidebarCollapsed: boolean;
  currentPath: string;
  setSidebarCollapsed: (collapsed: boolean) => void;
  setCurrentPath: (path: string) => void;
}

export const useUIStore = create<UIState>((set) => ({
  sidebarCollapsed: false,
  currentPath: '/',

  setSidebarCollapsed: (collapsed) => set({ sidebarCollapsed: collapsed }),

  setCurrentPath: (path) => set({ currentPath: path }),
}));
