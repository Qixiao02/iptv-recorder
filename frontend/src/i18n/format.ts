import type { TFunction } from 'i18next';
import type { AppLanguage } from './types';

export const formatBytes = (bytes: number, language: AppLanguage): string => {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return '-';
  }

  const value = bytes / 1024 ** 3;
  const formatted = new Intl.NumberFormat(language, {
    minimumFractionDigits: 0,
    maximumFractionDigits: 2,
  }).format(value);

  return `${formatted}G`;
};

export const formatShortDateTime = (value: string | Date, language: AppLanguage): string => {
  return new Intl.DateTimeFormat(language, {
    month: 'numeric',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(value));
};

export const formatMinutes = (seconds: number, t: TFunction): string => {
  if (seconds <= 0) {
    return '-';
  }

  return t('common:units.minute', { count: Math.floor(seconds / 60) });
};
