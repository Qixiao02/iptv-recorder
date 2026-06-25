import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { loadI18nNamespaces } from './index';
import type { I18nNamespace } from './types';

export const useI18nNamespace = (namespaces: I18nNamespace | I18nNamespace[]) => {
  const namespaceKey = Array.isArray(namespaces) ? namespaces.join('|') : namespaces;
  const normalizedNamespaces = useMemo(() => namespaceKey.split('|') as I18nNamespace[], [namespaceKey]);
  const { i18n } = useTranslation();
  const [isReady, setIsReady] = useState(() =>
    normalizedNamespaces.every((namespace) => i18n.hasResourceBundle(i18n.language, namespace))
  );

  useEffect(() => {
    let cancelled = false;
    if (!normalizedNamespaces.every((namespace) => i18n.hasResourceBundle(i18n.language, namespace))) {
      queueMicrotask(() => {
        if (!cancelled) {
          setIsReady(false);
        }
      });
    }

    loadI18nNamespaces(normalizedNamespaces).then(() => {
      if (!cancelled) {
        setIsReady(true);
      }
    });

    return () => {
      cancelled = true;
    };
  }, [i18n, i18n.language, normalizedNamespaces]);

  return isReady;
};
