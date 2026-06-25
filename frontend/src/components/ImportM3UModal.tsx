import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { importM3UFromUrl, importM3UFromContent } from '@/api/channels';
import { friendlyError } from '@/api/channels';
import { useI18nNamespace } from '@/i18n/useI18nNamespace';
import { X, Upload, Link, FileText, Loader2, CheckCircle, AlertCircle } from 'lucide-react';
import type { ImportM3UResponse } from '@/types';
import './Modal.css';

interface ImportM3UModalProps {
  isOpen: boolean;
  onClose: () => void;
  onImported?: (result: ImportM3UResponse) => void;
}

type ImportMode = 'url' | 'content';

export const ImportM3UModal: React.FC<ImportM3UModalProps> = ({ isOpen, onClose, onImported }) => {
  const { t } = useTranslation(['components', 'common']);
  useI18nNamespace(['components', 'common']);
  const queryClient = useQueryClient();
  const [mode, setMode] = useState<ImportMode>('url');
  const [url, setUrl] = useState('');
  const [content, setContent] = useState('');
  const [overwrite, setOverwrite] = useState(false);
  const [result, setResult] = useState<ImportM3UResponse | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const importUrlMutation = useMutation({
    mutationFn: importM3UFromUrl,
    onSuccess: async (data) => {
      setErrorMessage(null);
      setResult(data);
      onImported?.(data);
      await queryClient.invalidateQueries({ queryKey: ['channels'] });
      await queryClient.refetchQueries({ queryKey: ['channels'], type: 'active' });
    },
    onError: (error) => {
      setErrorMessage(friendlyError(error));
    },
  });

  const importContentMutation = useMutation({
    mutationFn: importM3UFromContent,
    onSuccess: async (data) => {
      setErrorMessage(null);
      setResult(data);
      onImported?.(data);
      await queryClient.invalidateQueries({ queryKey: ['channels'] });
      await queryClient.refetchQueries({ queryKey: ['channels'], type: 'active' });
    },
    onError: (error) => {
      setErrorMessage(friendlyError(error));
    },
  });

  const isLoading = importUrlMutation.isPending || importContentMutation.isPending;

  const handleSubmit = () => {
    setResult(null);
    setErrorMessage(null);
    if (mode === 'url' && url.trim()) {
      importUrlMutation.mutate({ url: url.trim(), overwrite });
    } else if (mode === 'content' && content.trim()) {
      importContentMutation.mutate({ content: content.trim(), overwrite });
    }
  };

  const handleClose = () => {
    setUrl('');
    setContent('');
    setResult(null);
    setErrorMessage(null);
    setOverwrite(false);
    onClose();
  };

  if (!isOpen) return null;

  return (
    <div className="modal-overlay" onClick={handleClose}>
      <div className="modal-content" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2>{t('components:importM3u.title')}</h2>
          <button className="modal-close" onClick={handleClose}>
            <X size={20} />
          </button>
        </div>

        <div className="modal-body">
          <div className="modal-tabs">
            <button className={`modal-tab ${mode === 'url' ? 'active' : ''}`} onClick={() => setMode('url')}>
              <Link size={16} />
              {t('components:importM3u.fromUrl')}
            </button>
            <button className={`modal-tab ${mode === 'content' ? 'active' : ''}`} onClick={() => setMode('content')}>
              <FileText size={16} />
              {t('components:importM3u.fromContent')}
            </button>
          </div>

          {mode === 'url' && (
            <div className="form-group">
              <label>{t('components:importM3u.urlLabel')}</label>
              <input className="input" placeholder="https://example.com/playlist.m3u" value={url} onChange={(e) => setUrl(e.target.value)} />
            </div>
          )}

          {mode === 'content' && (
            <div className="form-group">
              <label>{t('components:importM3u.contentLabel')}</label>
              <textarea
                className="input textarea"
                placeholder="#EXTM3U&#10;#EXTINF:-1,CCTV-1&#10;http://example.com/stream.m3u8"
                value={content}
                onChange={(e) => setContent(e.target.value)}
                rows={10}
              />
            </div>
          )}

          <div className="form-group">
            <label className="checkbox-label">
              <input type="checkbox" checked={overwrite} onChange={(e) => setOverwrite(e.target.checked)} />
              {t('components:importM3u.overwrite')}
            </label>
          </div>

          {result && (
            <div className="import-result">
              <div className="result-header">
                <CheckCircle size={18} />
                {t('components:importM3u.completed')}
              </div>
              <div className="result-stats">
                <span className="stat success">{t('components:importM3u.imported', { count: result.imported })}</span>
                <span className="stat warning">{t('components:importM3u.skipped', { count: result.skipped })}</span>
                <span className="stat error">{t('components:importM3u.failed', { count: result.failed })}</span>
              </div>
              {result.errors.length > 0 && (
                <div className="result-errors">
                  {result.errors.slice(0, 5).map((err, index) => (
                    <div key={`${err}-${index}`} className="error-item">
                      <AlertCircle size={14} />
                      {err}
                    </div>
                  ))}
                  {result.errors.length > 5 && (
                    <div className="more-errors">{t('components:importM3u.moreErrors', { count: result.errors.length - 5 })}</div>
                  )}
                </div>
              )}
            </div>
          )}

          {errorMessage && !result && (
            <div className="import-result" style={{ borderColor: 'rgba(239, 68, 68, 0.3)' }}>
              <div className="result-header" style={{ color: 'var(--color-error)' }}>
                <AlertCircle size={18} />
                {t('components:importM3u.importFailed', { defaultValue: '导入失败' })}
              </div>
              <div className="result-errors">
                <div className="error-item">{errorMessage}</div>
              </div>
            </div>
          )}
        </div>

        <div className="modal-footer">
          <button className="btn btn-ghost" onClick={handleClose}>{t('common:cancel')}</button>
          <button className="btn btn-primary" onClick={handleSubmit} disabled={isLoading || (mode === 'url' ? !url.trim() : !content.trim())}>
            {isLoading ? (
              <>
                <Loader2 size={16} className="animate-spin" />
                {t('components:importM3u.importing')}
              </>
            ) : (
              <>
                <Upload size={16} />
                {t('components:importM3u.start')}
              </>
            )}
          </button>
        </div>
      </div>
    </div>
  );
};

export default ImportM3UModal;
