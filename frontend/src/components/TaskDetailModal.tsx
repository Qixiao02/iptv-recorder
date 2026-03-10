import React, { useState } from 'react';
import { X, Copy, FolderOpen, Clock, HardDrive, FileVideo, Calendar, AlertCircle, CheckCircle } from 'lucide-react';
import type { Task } from '@/types';
import './TaskDetailModal.css';

interface TaskDetailModalProps {
  isOpen: boolean;
  onClose: () => void;
  task: Task | null;
  channelName?: string;
}

// 简单的 Toast 组件
const Toast: React.FC<{ message: string; onClose: () => void }> = ({ message, onClose }) => (
  <div className="detail-toast">
    <span>{message}</span>
    <button onClick={onClose}><X size={14} /></button>
  </div>
);

// 简单的信息弹窗
const InfoDialog: React.FC<{ message: string; onClose: () => void }> = ({ message, onClose }) => (
  <div className="info-dialog-overlay" onClick={onClose}>
    <div className="info-dialog" onClick={(e) => e.stopPropagation()}>
      <div className="info-dialog-icon">
        <FolderOpen size={24} />
      </div>
      <p className="info-dialog-message">{message}</p>
      <button className="btn btn-primary" onClick={onClose}>确定</button>
    </div>
  </div>
);

const statusLabels: Record<string, { label: string; color: string }> = {
  pending: { label: '等待中', color: 'neutral' },
  running: { label: '录制中', color: 'recording' },
  completed: { label: '已完成', color: 'success' },
  failed: { label: '失败', color: 'error' },
  cancelled: { label: '已取消', color: 'neutral' },
};

export const TaskDetailModal: React.FC<TaskDetailModalProps> = ({
  isOpen,
  onClose,
  task,
  channelName,
}) => {
  const [toast, setToast] = useState<string | null>(null);
  const [infoDialog, setInfoDialog] = useState<string | null>(null);

  if (!isOpen || !task) return null;

  const statusInfo = statusLabels[task.status] || { label: task.status, color: 'neutral' };

  const formatDuration = (seconds: number) => {
    if (!seconds) return '-';
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = seconds % 60;
    if (hours > 0) {
      return `${hours}小时${minutes}分钟${secs}秒`;
    }
    if (minutes > 0) {
      return `${minutes}分钟${secs}秒`;
    }
    return `${secs}秒`;
  };

  const formatFileSize = (bytes: number) => {
    if (!bytes) return '-';
    if (bytes >= 1024 * 1024 * 1024) {
      return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
    }
    if (bytes >= 1024 * 1024) {
      return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
    }
    return `${(bytes / 1024).toFixed(1)} KB`;
  };

  const formatDateTime = (dateStr: string | null | undefined) => {
    if (!dateStr) return '-';
    return new Date(dateStr).toLocaleString('zh-CN', {
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    });
  };

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text).then(() => {
      setToast('已复制到剪贴板');
      setTimeout(() => setToast(null), 2000);
    }).catch(() => {
      setToast('复制失败');
      setTimeout(() => setToast(null), 2000);
    });
  };

  const openFolder = (filePath: string) => {
    setInfoDialog(`文件路径:\n${filePath}\n\n请在文件资源管理器中打开此路径。`);
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-content task-detail-modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2>任务详情</h2>
          <button className="modal-close" onClick={onClose}>
            <X size={20} />
          </button>
        </div>

        <div className="modal-body">
          {/* 状态卡片 */}
          <div className={`status-card ${statusInfo.color}`}>
            {task.status === 'completed' ? (
              <CheckCircle size={24} />
            ) : task.status === 'failed' ? (
              <AlertCircle size={24} />
            ) : (
              <Clock size={24} />
            )}
            <span className="status-label">{statusInfo.label}</span>
          </div>

          {/* 基本信息 */}
          <div className="detail-section">
            <div className="detail-row">
              <div className="detail-icon">
                <FileVideo size={18} />
              </div>
              <div className="detail-content">
                <span className="detail-label">频道名称</span>
                <span className="detail-value">{channelName || task.channel_id}</span>
              </div>
            </div>

            <div className="detail-row">
              <div className="detail-icon">
                <Clock size={18} />
              </div>
              <div className="detail-content">
                <span className="detail-label">录制时长</span>
                <span className="detail-value">{formatDuration(task.duration_recorded)}</span>
              </div>
            </div>

            <div className="detail-row">
              <div className="detail-icon">
                <HardDrive size={18} />
              </div>
              <div className="detail-content">
                <span className="detail-label">文件大小</span>
                <span className="detail-value">{formatFileSize(task.file_size)}</span>
              </div>
            </div>
          </div>

          {/* 时间信息 */}
          <div className="detail-section">
            <h3 className="section-title">时间信息</h3>

            <div className="detail-row">
              <div className="detail-icon">
                <Calendar size={18} />
              </div>
              <div className="detail-content">
                <span className="detail-label">开始时间</span>
                <span className="detail-value">{formatDateTime(task.started_at)}</span>
              </div>
            </div>

            <div className="detail-row">
              <div className="detail-icon">
                <Calendar size={18} />
              </div>
              <div className="detail-content">
                <span className="detail-label">结束时间</span>
                <span className="detail-value">{formatDateTime(task.ended_at)}</span>
              </div>
            </div>
          </div>

          {/* 文件路径 */}
          <div className="detail-section">
            <h3 className="section-title">输出文件</h3>

            <div className="file-path-box">
              <code className="file-path">{task.output_path || '-'}</code>
              <div className="file-actions">
                <button
                  className="file-action-btn"
                  onClick={() => task.output_path && copyToClipboard(task.output_path)}
                  title="复制路径"
                  disabled={!task.output_path}
                >
                  <Copy size={16} />
                </button>
                <button
                  className="file-action-btn"
                  onClick={() => task.output_path && openFolder(task.output_path)}
                  title="打开所在目录"
                  disabled={!task.output_path}
                >
                  <FolderOpen size={16} />
                </button>
              </div>
            </div>
          </div>

          {/* 错误信息 */}
          {task.status === 'failed' && task.error_message && (
            <div className="detail-section error-section">
              <h3 className="section-title">错误信息</h3>
              <div className="error-message">
                <AlertCircle size={18} className="error-icon" />
                <span>{task.error_message}</span>
              </div>
            </div>
          )}

          {/* 进度信息（录制中） */}
          {task.status === 'running' && (
            <div className="detail-section">
              <h3 className="section-title">录制进度</h3>
              <div className="progress-section">
                <div className="progress-bar progress-bar-recording">
                  <div
                    className="progress-bar-fill"
                    style={{ width: `${task.progress_percent}%` }}
                  />
                </div>
                <span className="progress-percent">{task.progress_percent}%</span>
              </div>
              {task.current_speed && (
                <div className="current-speed">
                  下载速度: {task.current_speed}
                </div>
              )}
            </div>
          )}
        </div>

        <div className="modal-footer">
          <button className="btn btn-ghost" onClick={onClose}>
            关闭
          </button>
        </div>
      </div>

      {/* Toast */}
      {toast && <Toast message={toast} onClose={() => setToast(null)} />}

      {/* Info Dialog */}
      {infoDialog && <InfoDialog message={infoDialog} onClose={() => setInfoDialog(null)} />}
    </div>
  );
};

export default TaskDetailModal;
