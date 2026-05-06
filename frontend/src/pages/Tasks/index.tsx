import React, { Suspense, lazy, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getTasks, cancelTask, clearCompletedTasks, deleteTask } from '@/api/tasks';
import { getAllChannels } from '@/api/channels';
import {
  Clapperboard,
  CircleDot,
  CheckCircle2,
  XCircle,
  Clock,
  RefreshCw,
  StopCircle,
  Eye,
  ChevronRight,
  AlertCircle,
  Trash2,
} from 'lucide-react';
import type { TaskStatus, Task } from '@/types';
import './Tasks.css';

const TaskDetailModal = lazy(() => import('@/components/TaskDetailModal'));
const ConfirmDialog = lazy(() => import('@/components/ConfirmDialog'));

type FilterStatus = 'all' | TaskStatus;

const statusConfig: Record<TaskStatus, { label: string; icon: React.ReactNode; color: string }> = {
  pending: { label: '等待中', icon: <Clock size={14} />, color: 'neutral' },
  running: { label: '录制中', icon: <CircleDot size={14} />, color: 'recording' },
  completed: { label: '已完成', icon: <CheckCircle2 size={14} />, color: 'success' },
  failed: { label: '失败', icon: <XCircle size={14} />, color: 'error' },
  cancelled: { label: '已取消', icon: <XCircle size={14} />, color: 'neutral' },
};

export const Tasks: React.FC = () => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [filterStatus, setFilterStatus] = useState<FilterStatus>('all');
  const [selectedTask, setSelectedTask] = useState<Task | null>(null);
  const [deleteConfirm, setDeleteConfirm] = useState<{ isOpen: boolean; taskId: string | null }>({
    isOpen: false,
    taskId: null,
  });
  const [clearConfirm, setClearConfirm] = useState(false);

  const { data: tasks, isLoading, refetch } = useQuery({
    queryKey: ['tasks'],
    queryFn: getTasks,
    refetchInterval: 5000,
  });

  const { data: channels } = useQuery({
    queryKey: ['channels', 'all'],
    queryFn: getAllChannels,
  });

  // 创建 channel_id -> channel_name 映射
  const channelMap = React.useMemo(() => {
    const map = new Map<string, string>();
    channels?.forEach((ch) => {
      map.set(ch.id, ch.name);
    });
    return map;
  }, [channels]);

  const cancelMutation = useMutation({
    mutationFn: cancelTask,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tasks'] });
    },
  });

  const clearMutation = useMutation({
    mutationFn: clearCompletedTasks,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tasks'] });
      setClearConfirm(false);
    },
  });

  const deleteMutation = useMutation({
    mutationFn: deleteTask,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tasks'] });
      setDeleteConfirm({ isOpen: false, taskId: null });
    },
  });

  const filteredTasks = tasks?.filter((task) => {
    if (filterStatus === 'all') return true;
    return task.status === filterStatus;
  });

  const runningCount = tasks?.filter((t) => t.status === 'running').length || 0;
  const completedCount = tasks?.filter((t) => t.status === 'completed').length || 0;
  const failedCount = tasks?.filter((t) => t.status === 'failed').length || 0;

  const formatDuration = (seconds: number) => {
    if (!seconds) return '-';
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = seconds % 60;
    if (hours > 0) return `${hours}:${String(minutes).padStart(2, '0')}:${String(secs).padStart(2, '0')}`;
    return `${minutes}:${String(secs).padStart(2, '0')}`;
  };

  const formatFileSize = (bytes: number) => {
    if (!bytes) return '-';
    if (bytes >= 1024 * 1024 * 1024) {
      return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
    }
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  };

  const shouldRenderTaskDetail = selectedTask !== null;
  const shouldRenderDeleteConfirm = deleteConfirm.isOpen;
  const shouldRenderClearConfirm = clearConfirm;

  return (
    <div className="tasks-page">
      {/* Page Header */}
      <div className="page-header">
        <div className="page-title">
          <h1>{t('menu.tasks')}</h1>
          <p className="page-subtitle">录制任务执行历史</p>
        </div>
        <div className="page-actions">
          <button
            className="btn btn-ghost"
            onClick={() => setClearConfirm(true)}
            disabled={clearMutation.isPending}
            title="清除已完成/失败/取消的任务记录"
          >
            <Trash2 size={16} />
            清除记录
          </button>
          <button className="btn btn-ghost" onClick={() => refetch()}>
            <RefreshCw size={16} />
            刷新
          </button>
        </div>
      </div>

      {/* Stats Bar */}
      <div className="stats-bar">
        <div className="stat-item">
          <div className="stat-number recording">{runningCount}</div>
          <div className="stat-label">录制中</div>
        </div>
        <div className="stat-item">
          <div className="stat-number success">{completedCount}</div>
          <div className="stat-label">已完成</div>
        </div>
        <div className="stat-item">
          <div className="stat-number error">{failedCount}</div>
          <div className="stat-label">失败</div>
        </div>
      </div>

      {/* Filter Tabs */}
      <div className="filter-tabs">
        <button
          className={`tab-btn ${filterStatus === 'all' ? 'active' : ''}`}
          onClick={() => setFilterStatus('all')}
        >
          全部
        </button>
        {Object.entries(statusConfig).map(([status, config]) => (
          <button
            key={status}
            className={`tab-btn ${filterStatus === status ? 'active' : ''}`}
            onClick={() => setFilterStatus(status as TaskStatus)}
          >
            {config.icon}
            {config.label}
          </button>
        ))}
      </div>

      {/* Tasks List */}
      {isLoading ? (
        <div className="loading-list">
          {[1, 2, 3, 4, 5].map((i) => (
            <div key={i} className="task-skeleton card animate-shimmer" />
          ))}
        </div>
      ) : filteredTasks && filteredTasks.length > 0 ? (
        <div className="tasks-list">
          {filteredTasks.map((task, idx) => {
            const config = statusConfig[task.status];
            const isRunning = task.status === 'running';
            const channelName = channelMap.get(task.channel_id) || `频道 ${task.channel_id.slice(0, 8)}...`;

            return (
              <div
                key={task.id}
                className={`task-card card stagger-item ${isRunning ? 'recording' : ''}`}
                style={{ animationDelay: `${idx * 0.03}s` }}
              >
                <div className="task-left">
                  <div className={`task-status-indicator ${config.color}`}>
                    {config.icon}
                  </div>

                  <div className="task-info">
                    <div className="task-title">
                      {channelName}
                    </div>
                    <div className="task-meta">
                      <span className={`badge badge-${config.color}`}>
                        {config.label}
                      </span>
                      {isRunning && task.current_speed && (
                        <span className="speed-indicator">{task.current_speed}</span>
                      )}
                      {task.started_at && (
                        <span className="task-time">
                          {new Date(task.started_at).toLocaleString('zh-CN', {
                            month: 'numeric',
                            day: 'numeric',
                            hour: '2-digit',
                            minute: '2-digit',
                          })}
                        </span>
                      )}
                    </div>
                  </div>
                </div>

                <div className="task-center">
                  {isRunning ? (
                    <div className="progress-section">
                      <div className="progress-bar progress-bar-recording">
                        <div
                          className="progress-bar-fill"
                          style={{ width: `${task.progress_percent}%` }}
                        />
                      </div>
                      <span className="progress-percent">{task.progress_percent}%</span>
                    </div>
                  ) : (
                    <div className="task-stats">
                      <div className="stat">
                        <span className="stat-value">{formatDuration(task.duration_recorded)}</span>
                        <span className="stat-label">时长</span>
                      </div>
                      <div className="stat">
                        <span className="stat-value">{formatFileSize(task.file_size)}</span>
                        <span className="stat-label">大小</span>
                      </div>
                    </div>
                  )}
                </div>

                <div className="task-right">
                  {isRunning ? (
                    <button
                      className="btn btn-danger btn-sm"
                      onClick={() => cancelMutation.mutate(task.id)}
                    >
                      <StopCircle size={14} />
                      停止
                    </button>
                  ) : (
                    <>
                      {task.status === 'completed' && (
                        <button
                          className="btn btn-ghost btn-sm"
                          onClick={() => setSelectedTask(task)}
                        >
                          <Eye size={14} />
                          查看
                        </button>
                      )}
                      {task.status === 'failed' && task.error_message && (
                        <div
                          className="error-info clickable"
                          title={task.error_message}
                          onClick={() => setSelectedTask(task)}
                        >
                          <AlertCircle size={16} className="error-icon" />
                        </div>
                      )}
                      <button
                        className="btn btn-ghost btn-sm btn-delete"
                        onClick={() => setDeleteConfirm({ isOpen: true, taskId: task.id })}
                        title="删除记录"
                      >
                        <Trash2 size={14} />
                      </button>
                    </>
                  )}
                  <button
                    className="action-btn"
                    onClick={() => setSelectedTask(task)}
                  >
                    <ChevronRight size={16} />
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      ) : (
        <div className="empty-state card">
          <div className="empty-icon">
            <Clapperboard size={48} strokeWidth={1} />
          </div>
          <div className="empty-title">没有任务记录</div>
          <div className="empty-desc">
            {filterStatus === 'all'
              ? '开始录制后会在这里显示任务'
              : `没有${statusConfig[filterStatus as TaskStatus]?.label || ''}的任务`}
          </div>
        </div>
      )}

      <Suspense fallback={null}>
        {shouldRenderTaskDetail && (
          <TaskDetailModal
            isOpen
            onClose={() => setSelectedTask(null)}
            task={selectedTask}
            channelName={selectedTask ? channelMap.get(selectedTask.channel_id) : undefined}
          />
        )}

        {shouldRenderDeleteConfirm && (
          <ConfirmDialog
            isOpen={deleteConfirm.isOpen}
            onClose={() => setDeleteConfirm({ isOpen: false, taskId: null })}
            onConfirm={() => deleteConfirm.taskId && deleteMutation.mutate(deleteConfirm.taskId)}
            title="删除任务记录"
            message="确定要删除此任务记录吗？此操作无法撤销。"
            confirmText="删除"
            type="danger"
            isLoading={deleteMutation.isPending}
          />
        )}

        {shouldRenderClearConfirm && (
          <ConfirmDialog
            isOpen={clearConfirm}
            onClose={() => setClearConfirm(false)}
            onConfirm={() => clearMutation.mutate()}
            title="清除任务记录"
            message="确定要清除所有已完成、失败和取消的任务记录吗？此操作无法撤销。"
            confirmText="清除"
            type="warning"
            isLoading={clearMutation.isPending}
          />
        )}
      </Suspense>
    </div>
  );
};

export default Tasks;
