import type { AppLanguage, I18nNamespace } from './types';

type ResourceLoader = () => Promise<{ default: Record<string, unknown> }>;

export const namespaceLoaders: Record<AppLanguage, Record<I18nNamespace, ResourceLoader>> = {
  'zh-CN': {
    common: () => import('./modules/common/zh-CN'),
    layout: () => import('./modules/layout/zh-CN'),
    login: () => import('./modules/login/zh-CN'),
    dashboard: () => import('./modules/dashboard/zh-CN'),
    channels: () => import('./modules/channels/zh-CN'),
    schedules: () => import('./modules/schedules/zh-CN'),
    tasks: () => import('./modules/tasks/zh-CN'),
    settings: () => import('./modules/settings/zh-CN'),
    components: () => import('./modules/components/zh-CN'),
  },
  'en-US': {
    common: () => import('./modules/common/en-US'),
    layout: () => import('./modules/layout/en-US'),
    login: () => import('./modules/login/en-US'),
    dashboard: () => import('./modules/dashboard/en-US'),
    channels: () => import('./modules/channels/en-US'),
    schedules: () => import('./modules/schedules/en-US'),
    tasks: () => import('./modules/tasks/en-US'),
    settings: () => import('./modules/settings/en-US'),
    components: () => import('./modules/components/en-US'),
  },
};
