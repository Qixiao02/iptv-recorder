import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { UserInfo } from '@/api/auth';

interface AuthState {
  token: string | null;
  user: UserInfo | null;
  isAuthenticated: boolean;
  setAuth: (token: string, user: UserInfo) => void;
  logout: () => void;
  updateUser: (user: UserInfo) => void;
}

const AUTH_STORAGE_KEY = 'auth-storage';

export const getStoredAuthToken = (): string | null => {
  const authStorage = localStorage.getItem(AUTH_STORAGE_KEY);
  if (!authStorage) {
    return null;
  }

  try {
    const parsed = JSON.parse(authStorage);
    return parsed?.state?.token ?? null;
  } catch {
    return null;
  }
};

export const useAuthStore = create<AuthState>()(
  persist(
    (set) => ({
      token: null,
      user: null,
      isAuthenticated: false,
      setAuth: (token, user) =>
        set({
          token,
          user,
          isAuthenticated: true,
        }),
      logout: () =>
        set({
          token: null,
          user: null,
          isAuthenticated: false,
        }),
      updateUser: (user) =>
        set({ user }),
    }),
    {
      name: AUTH_STORAGE_KEY,
    }
  )
);
