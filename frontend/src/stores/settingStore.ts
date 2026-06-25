import { create } from 'zustand';
import { changeAppLanguage, getSavedLanguage } from '@/i18n';
import type { AppLanguage } from '@/i18n/types';

interface SettingState {
  language: AppLanguage;
  setLanguage: (language: AppLanguage) => Promise<void>;
}

export const useSettingStore = create<SettingState>((set) => ({
  language: getSavedLanguage(),

  setLanguage: async (language) => {
    await changeAppLanguage(language);
    set({ language });
  },
}));
