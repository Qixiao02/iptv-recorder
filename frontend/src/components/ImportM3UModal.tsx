import React, { useState, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { importM3UFromUrl, importM3UFromContent } from '@/api/channels';
import { friendlyError } from '@/api/channels';
import { useI18nNamespace } from '@/i18n/useI18nNamespace';
import { X, Upload, Link, FileText, Loader2, CheckCircle, AlertCircle, FileUp } from 'lucide-react';
import { toast } from '@/stores/toastStore';
import { channelKeys } from '@/lib/queryKeys';
import type { ImportM3UResponse } from '@/types';
import './Modal.css';

interface ImportM3UModalProps {
  isOpen: boolean;
  onClose: () => void;
  onImported?: (result: ImportM3UResponse) => void;
}

type ImportMode = 'url' | 'content' | 'file';

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
  const [fileName, setFileName] = useState<string | null>(null);
  const [fileContent, setFileContent] = useState<string | null>(null);
  const [dragging, setDragging] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const importUrlMutation = useMutation({
    mutationFn: importM3UFromUrl,
    onSuccess: async (data) => {
      setErrorMessage(null);
      setResult(data);
      onImported?.(data);
      await queryClient.invalidateQueries({ queryKey: channelKeys.root });
      await queryClient.refetchQueries({ queryKey: channelKeys.root, type: 'active' });
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
      await queryClient.invalidateQueries({ queryKey: channelKeys.root });
      await queryClient.refetchQueries({ queryKey: channelKeys.root, type: 'active' });
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
    } else if (mode === 'file' && fileContent && fileContent.trim()) {
      importContentMutation.mutate({ content: fileContent.trim(), overwrite });
    }
  };

  // 读取上传的 M3U 文件为文本
  const handleFile = (file: File) => {
    // 校验文件类型(.m3u / .m3u8 / 文本)
    const validExt = /\.(m3u8?|txt)$/i.test(file.name);
    if (!validExt && !file.type.startsWith('text/')) {
      toast.error(t('components:importM3u.invalidFileType', { defaultValue: '请选择 .m3u 或 .m3u8 文件' }));
      return;
    }
    // 大小限制 10MB
    if (file.size > 10 * 1024 * 1024) {
      toast.error(t('components:importM3u.fileTooLarge', { defaultValue: '文件不能超过 10MB' }));
      return;
    }
    const reader = new FileReader();
    reader.onload = (e) => {
      const text = e.target?.result as string;
      setFileContent(text);
      setFileName(file.name);
    };
    reader.onerror = () => {
      toast.error(t('components:importM3u.fileReadError', { defaultValue: '文件读取失败' }));
    };
    reader.readAsText(file);
  };

  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) handleFile(file);
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setDragging(false);
    const file = e.dataTransfer.files?.[0];
    if (file) handleFile(file);
  };

  const handleClearFile = () => {
    setFileContent(null);
    setFileName(null);
    if (fileInputRef.current) fileInputRef.current.value = '';
  };

  const handleClose = () => {
    setUrl('');
    setContent('');
    setResult(null);
    setErrorMessage(null);
    setOverwrite(false);
    setFileContent(null);
    setFileName(null);
    setDragging(false);
    if (fileInputRef.current) fileInputRef.current.value = '';
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
            <button className={`modal-tab ${mode === 'file' ? 'active' : ''}`} onClick={() => setMode('file')}>
              <FileUp size={16} />
              {t('components:importM3u.fromFile', { defaultValue: '上传文件' })}
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

          {mode === 'file' && (
            <div className="form-group">
              <label>{t('components:importM3u.fileLabel', { defaultValue: '选择本地 M3U 文件' })}</label>
              <input
                ref={fileInputRef}
                type="file"
                accept=".m3u,.m3u8,.txt"
                onChange={handleFileSelect}
                style={{ display: 'none' }}
              />
              {fileName ? (
                <div className="file-selected">
                  <FileText size={18} />
                  <span className="file-name">{fileName}</span>
                  <button type="button" className="btn btn-ghost btn-sm" onClick={handleClearFile}>
                    <X size={14} />
                  </button>
                </div>
              ) : (
                <div
                  className={`file-dropzone ${dragging ? 'dragging' : ''}`}
                  onClick={() => fileInputRef.current?.click()}
                  onDragOver={(e) => { e.preventDefault(); setDragging(true); }}
                  onDragLeave={() => setDragging(false)}
                  onDrop={handleDrop}
                >
                  <FileUp size={32} />
                  <span className="dropzone-text">
                    {t('components:importM3u.dropHere', { defaultValue: '点击或拖拽 .m3u / .m3u8 文件到此处' })}
                  </span>
                  <span className="dropzone-hint">
                    {t('components:importM3u.fileHint', { defaultValue: '支持 .m3u、.m3u8 格式，最大 10MB' })}
                  </span>
                </div>
              )}
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
          <button className="btn btn-primary" onClick={handleSubmit} disabled={isLoading || (mode === 'url' ? !url.trim() : mode === 'content' ? !content.trim() : !fileContent?.trim())}>
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
