export const SUPPORTED_LANGUAGES = ['zh-CN', 'en-US'] as const;

export type AppLanguage = (typeof SUPPORTED_LANGUAGES)[number];

export const DEFAULT_LANGUAGE: AppLanguage = 'zh-CN';

export const I18N_NAMESPACES = [
  'common',
  'layout',
  'login',
  'dashboard',
  'channels',
  'schedules',
  'tasks',
  'settings',
  'components',
] as const;

export type I18nNamespace = (typeof I18N_NAMESPACES)[number];
