import React from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { getEpgPrograms } from '@/api/epg';
import { useI18nNamespace } from '@/i18n/useI18nNamespace';
import { formatShortDateTime } from '@/i18n/format';
import { epgKeys } from '@/lib/queryKeys';
import type { AppLanguage } from '@/i18n/types';
import { CalendarDays, Loader2, X } from 'lucide-react';
import './Modal.css';

interface EpgProgramsModalProps {
  isOpen: boolean;
  onClose: () => void;
  channelRef: string;
  channelName: string;
}

export const EpgProgramsModal: React.FC<EpgProgramsModalProps> = ({ isOpen, onClose, channelRef, channelName }) => {
  const { t, i18n } = useTranslation(['components']);
  useI18nNamespace('components');
  const { data, isLoading, isError, error } = useQuery({
    queryKey: epgKeys.programs(channelRef),
    queryFn: () => getEpgPrograms(channelRef, 24),
    enabled: isOpen && channelRef.length > 0,
  });

  if (!isOpen) return null;

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-content" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2>{t('components:epgPrograms.title', { channelName })}</h2>
          <button className="modal-close" onClick={onClose}>
            <X size={20} />
          </button>
        </div>

        <div className="modal-body">
          {isLoading ? (
            <div className="empty-state">
              <Loader2 size={24} className="animate-spin" />
              <div className="empty-desc">{t('components:epgPrograms.loading')}</div>
            </div>
          ) : isError ? (
            <div className="empty-state">
              <div className="empty-title">{t('components:epgPrograms.loadFailed')}</div>
              <div className="empty-desc">{(error as Error).message}</div>
            </div>
          ) : data && data.length > 0 ? (
            <div className="epg-program-list">
              {data.map((program) => (
                <div key={program.id} className="epg-program-item">
                  <div className="epg-program-time">
                    {formatShortDateTime(program.start_at, i18n.language as AppLanguage)}
                    {' - '}
                    {new Intl.DateTimeFormat(i18n.language, { hour: '2-digit', minute: '2-digit' }).format(new Date(program.end_at))}
                  </div>
                  <div className="epg-program-title">{program.title}</div>
                  {program.category && <div className="epg-program-meta">{program.category}</div>}
                  {program.description && <div className="epg-program-meta">{program.description}</div>}
                </div>
              ))}
            </div>
          ) : (
            <div className="empty-state">
              <CalendarDays size={32} />
              <div className="empty-title">{t('components:epgPrograms.emptyTitle')}</div>
              <div className="empty-desc">{t('components:epgPrograms.emptyDesc')}</div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

export default EpgProgramsModal;
