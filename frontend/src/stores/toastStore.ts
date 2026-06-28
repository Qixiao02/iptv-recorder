import { create } from 'zustand';

export type ToastType = 'success' | 'error' | 'info' | 'warning';

export interface ToastItem {
  id: number;
  message: string;
  type: ToastType;
}

interface ToastState {
  toasts: ToastItem[];
  /** 推送一条 toast,默认 3 秒后自动消失 */
  showToast: (message: string, type?: ToastType) => void;
  /** 成功提示(语法糖) */
  success: (message: string) => void;
  /** 失败/错误提示(语法糖) */
  error: (message: string) => void;
  /** 信息提示(语法糖) */
  info: (message: string) => void;
  removeToast: (id: number) => void;
}

let counter = 0;

export const useToastStore = create<ToastState>((set) => ({
  toasts: [],

  showToast: (message, type = 'info') => {
    const id = ++counter;
    set((state) => ({ toasts: [...state.toasts, { id, message, type }] }));
    setTimeout(() => {
      set((state) => ({ toasts: state.toasts.filter((t) => t.id !== id) }));
    }, 3000);
  },

  success: (message) => useToastStore.getState().showToast(message, 'success'),
  error: (message) => useToastStore.getState().showToast(message, 'error'),
  info: (message) => useToastStore.getState().showToast(message, 'info'),

  removeToast: (id) => set((state) => ({ toasts: state.toasts.filter((t) => t.id !== id) })),
}));

/**
 * 非 hook 形式的全局 toast 触发器。
 * 适用于在 React 组件外(如 mutation 回调、工具函数)直接调用。
 */
export const toast = {
  success: (message: string) => useToastStore.getState().success(message),
  error: (message: string) => useToastStore.getState().error(message),
  info: (message: string) => useToastStore.getState().info(message),
  show: (message: string, type: ToastType = 'info') =>
    useToastStore.getState().showToast(message, type),
};
