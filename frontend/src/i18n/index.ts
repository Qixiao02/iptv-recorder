import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import legacyZhCN from '@/locales/zh-CN';
import legacyEnUS from '@/locales/en-US';
import { namespaceLoaders } from './loaders';
import { DEFAULT_LANGUAGE, SUPPORTED_LANGUAGES, type AppLanguage, type I18nNamespace } from './types';

const isSupportedLanguage = (language: string | null): language is AppLanguage => {
  return SUPPORTED_LANGUAGES.includes(language as AppLanguage);
};

export const getSavedLanguage = (): AppLanguage => {
  const savedLanguage = localStorage.getItem('language');
  return isSupportedLanguage(savedLanguage) ? savedLanguage : DEFAULT_LANGUAGE;
};

i18n.use(initReactI18next).init({
  resources: {
    'zh-CN': { translation: legacyZhCN },
    'en-US': { translation: legacyEnUS },
  },
  lng: getSavedLanguage(),
  fallbackLng: DEFAULT_LANGUAGE,
  ns: ['translation'],
  defaultNS: 'translation',
  fallbackNS: 'translation',
  interpolation: {
    escapeValue: false,
  },
});

export const loadI18nNamespace = async (
  namespace: I18nNamespace,
  language: AppLanguage = i18n.resolvedLanguage as AppLanguage || getSavedLanguage()
): Promise<void> => {
  if (i18n.hasResourceBundle(language, namespace)) {
    return;
  }

  const resource = await namespaceLoaders[language][namespace]();
  i18n.addResourceBundle(language, namespace, resource.default, true, true);
};

export const loadI18nNamespaces = async (
  namespaces: I18nNamespace[],
  language: AppLanguage = i18n.resolvedLanguage as AppLanguage || getSavedLanguage()
): Promise<void> => {
  await Promise.all(namespaces.map((namespace) => loadI18nNamespace(namespace, language)));
};

export const changeAppLanguage = async (language: AppLanguage): Promise<void> => {
  const namespaces = Object.keys(i18n.store.data[getSavedLanguage()] ?? {})
    .filter((namespace): namespace is I18nNamespace => namespace in namespaceLoaders[language]);

  await loadI18nNamespaces(namespaces, language);
  localStorage.setItem('language', language);
  await i18n.changeLanguage(language);
};

export default i18n;

