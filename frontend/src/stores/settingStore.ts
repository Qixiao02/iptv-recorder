import { create } from 'zustand';

interface SettingState {
  language: 'zh-CN' | 'en-US';
  setLanguage: (language: 'zh-CN' | 'en-US') => void;
}

export const useSettingStore = create<SettingState>((set) => ({
  language: (localStorage.getItem('language') as 'zh-CN' | 'en-US') || 'zh-CN',

  setLanguage: (language) => {
    localStorage.setItem('language', language);
    set({ language });
  },
}));
