import React, { useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useI18nNamespace } from '@/i18n/useI18nNamespace';
import { useModalA11y } from '@/lib/useModalA11y';
import { toast } from '@/stores/toastStore';
import {
  X,
  Copy,
  FolderOpen,
  Clock,
  HardDrive,
  FileVideo,
  Calendar,
  AlertCircle,
  CheckCircle,
} from 'lucide-react';
import { formatBytes } from '@/i18n/format';
import type { AppLanguage } from '@/i18n/types';
import type { Task } from '@/types';
import './Modal.css';
import './TaskDetailModal.css';

interface TaskDetailModalProps {
  isOpen: boolean;
  onClose: () => void;
  task: Task | null;
  channelName?: string;
}

const statusColors: Record<string, string> = {
  pending: 'neutral',
  running: 'recording',
  completed: 'success',
  failed: 'error',
  cancelled: 'neutral',
};

export const TaskDetailModal: React.FC<TaskDetailModalProps> = ({
  isOpen,
  onClose,
  task,
  channelName,
}) => {
  const { t, i18n } = useTranslation(['components', 'common']);
  useI18nNamespace(['components', 'common']);
  const overlayRef = useRef<HTMLDivElement>(null);
  useModalA11y(overlayRef, isOpen, onClose);

  if (!isOpen || !task) return null;

  const statusInfo = {
    label: t(`common:taskStatus.${task.status}`, { defaultValue: task.status }),
    color: statusColors[task.status] || 'neutral',
  };

  const formatDuration = (seconds: number) => {
    if (!seconds) return '-';
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = seconds % 60;
    if (hours > 0) {
      return t('components:taskDetail.duration.hms', { hours, minutes, seconds: secs });
    }
    if (minutes > 0) {
      return t('components:taskDetail.duration.ms', { minutes, seconds: secs });
    }
    return t('components:taskDetail.duration.s', { seconds: secs });
  };

  const formatDateTime = (dateStr: string | null | undefined) => {
    if (!dateStr) return '-';
    return new Date(dateStr).toLocaleString(i18n.language, {
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    });
  };

  const copyToClipboard = (text: string) => {
    navigator.clipboard
      .writeText(text)
      .then(() => toast.success(t('common:toast.copiedToClipboard')))
      .catch(() => toast.error(t('common:toast.operationFailed', { message: '' })));
  };

  return (
    <div
      className="modal-overlay"
      onClick={onClose}
      ref={overlayRef}
      role="dialog"
      aria-modal="true"
      aria-labelledby="task-detail-title"
      tabIndex={-1}
    >
      <div className="modal-content modal-content-fixed" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2 id="task-detail-title">{t('components:taskDetail.title')}</h2>
          <button className="modal-close" onClick={onClose} aria-label={t('common:close', { defaultValue: '关闭' })}>
            <X size={20} />
          </button>
        </div>

        <div className="modal-body">
          <div className={`task-detail-hero ${statusInfo.color}`}>
            {task.status === 'completed' ? (
              <CheckCircle size={24} />
            ) : task.status === 'failed' ? (
              <AlertCircle size={24} />
            ) : task.status === 'running' ? (
              <div className="pulse-dot" />
            ) : (
              <Clock size={24} />
            )}
            <span className="hero-label">{statusInfo.label}</span>
          </div>

          {task.output_path && (
            <div className="detail-path-row">
              <div className="path-text" title={task.output_path}>
                <FolderOpen size={14} />
                <span>{task.output_path}</span>
              </div>
              <div className="path-actions">
                <button
                  className="btn btn-icon"
                  title={t('components:taskDetail.copyPath')}
                  onClick={() => copyToClipboard(task.output_path!)}
                >
                  <Copy size={15} />
                </button>
                <button
                  className="btn btn-icon"
                  title={t('components:taskDetail.openFolder')}
                  onClick={() => {
                    const msg = t('components:taskDetail.openFolderMessage', {
                      path: task.output_path,
                    });
                    toast.info(msg);
                  }}
                >
                  <FolderOpen size={15} />
                </button>
              </div>
            </div>
          )}

          <div className="detail-section detail-summary-grid">
            <div className="detail-metric">
              <FileVideo size={18} className="metric-icon" />
              <div className="detail-content">
                <span className="metric-label">{t('components:taskDetail.channelName')}</span>
                <span className="metric-value">{channelName || task.channel_id}</span>
              </div>
            </div>

            <div className="detail-metric">
              <Clock size={18} className="metric-icon" />
              <div className="detail-content">
                <span className="metric-label">{t('components:taskDetail.recordedDuration')}</span>
                <span className="metric-value">{formatDuration(task.duration_recorded)}</span>
              </div>
            </div>

            <div className="detail-metric">
              <Calendar size={18} className="metric-icon" />
              <div className="detail-content">
                <span className="metric-label">{t('components:taskDetail.startedAt')}</span>
                <span className="metric-value">{formatDateTime(task.started_at)}</span>
              </div>
            </div>
          </div>

          <div className="detail-section detail-time-grid">
            <div className="detail-metric">
              <Calendar size={18} className="metric-icon" />
              <div className="detail-content">
                <span className="metric-label">{t('components:taskDetail.endedAt')}</span>
                <span className="metric-value">{formatDateTime(task.ended_at)}</span>
              </div>
            </div>

            <div className="detail-metric">
              <HardDrive size={18} className="metric-icon" />
              <div className="detail-content">
                <span className="metric-label">{t('components:taskDetail.fileSize')}</span>
                <span className="metric-value">
                  {formatBytes(task.file_size, i18n.language as AppLanguage)}
                </span>
              </div>
            </div>
          </div>

          {task.status === 'failed' && task.error_message && (
            <div className="error-section">
              <AlertCircle size={16} />
              <span className="error-text">{task.error_message}</span>
            </div>
          )}

          {task.status === 'running' && task.progress_percent != null && (
            <div className="progress-section">
              <div className="progress-bar">
                <div
                  className="progress-bar-fill"
                  style={{ width: `${Math.min(100, task.progress_percent)}%` }}
                />
              </div>
              <span className="progress-percent">{Math.round(task.progress_percent)}%</span>
            </div>
          )}
        </div>

        <div className="modal-footer">
          <button className="btn btn-ghost" onClick={onClose}>
            {t('common:close', '关闭')}
          </button>
        </div>
      </div>
    </div>
  );
};

export default TaskDetailModal;
