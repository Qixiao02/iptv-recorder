import React, { Suspense, lazy, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getTasks, cancelTask, clearCompletedTasks, deleteTask } from '@/api/tasks';
import { getAllChannels } from '@/api/channels';
import { wsClient, type ConnectionState } from '@/api/websocket';
import { toast } from '@/stores/toastStore';
import { useI18nNamespace } from '@/i18n/useI18nNamespace';
import { formatBytes, formatShortDateTime } from '@/i18n/format';
import type { AppLanguage } from '@/i18n/types';
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
  Radio,
} from 'lucide-react';
import type { TaskStatus } from '@/types';
import type { Task, TaskProgressData, TaskUpdateData } from '@/types';
import './Tasks.css';

const TaskDetailModal = lazy(() => import('@/components/TaskDetailModal'));
const ConfirmDialog = lazy(() => import('@/components/ConfirmDialog'));

type FilterStatus = 'all' | TaskStatus;

const statusMeta: Record<TaskStatus, { icon: React.ReactNode; color: string }> = {
  pending: { icon: <Clock size={14} />, color: 'neutral' },
  running: { icon: <CircleDot size={14} />, color: 'recording' },
  completed: { icon: <CheckCircle2 size={14} />, color: 'success' },
  failed: { icon: <XCircle size={14} />, color: 'error' },
  cancelled: { icon: <XCircle size={14} />, color: 'neutral' },
};

export const Tasks: React.FC = () => {
  const { t, i18n } = useTranslation(['tasks', 'common']);
  const isI18nReady = useI18nNamespace(['tasks', 'common']);
  const queryClient = useQueryClient();
  const [filterStatus, setFilterStatus] = useState<FilterStatus>('all');
  const [selectedTaskId, setSelectedTaskId] = useState<string | null>(null);
  const [liveProgress, setLiveProgress] = useState<Map<string, { percent: number; speed: string; downloaded: number }>>(new Map());
  const [deleteConfirm, setDeleteConfirm] = useState<{ isOpen: boolean; taskId: string | null }>({
    isOpen: false,
    taskId: null,
  });
  const [clearConfirm, setClearConfirm] = useState(false);
  const [connectionState, setConnectionState] = useState<ConnectionState>(
    wsClient.getConnectionState(),
  );

  const { data: tasks, isLoading, refetch } = useQuery({
    queryKey: ['tasks'],
    queryFn: getTasks,
    refetchInterval: (query) => {
      const currentTasks = query.state.data as Task[] | undefined;
      return currentTasks?.some((task) => task.status === 'running') ? 3000 : false;
    },
    refetchIntervalInBackground: true,
  });

  const { data: channels } = useQuery({
    queryKey: ['channels', 'all'],
    queryFn: getAllChannels,
  });

  useEffect(() => wsClient.onConnectionStateChange(setConnectionState), []);

  useEffect(() => {
    wsClient.connect();

    const patchTask = (taskId: string, patch: Partial<Task>) => {
      queryClient.setQueryData<Task[]>(['tasks'], (oldTasks) => {
        if (!oldTasks) return oldTasks;
        return oldTasks.map((task) => (
          task.id === taskId
            ? { ...task, ...patch, updated_at: patch.updated_at ?? new Date().toISOString() }
            : task
        ));
      });
    };

    const unsubscribeProgress = wsClient.onTaskProgress((data: TaskProgressData) => {
      setLiveProgress((prev) => {
        const next = new Map(prev);
        next.set(data.task_id, { percent: data.percent, speed: data.speed, downloaded: data.downloaded_bytes });
        return next;
      });
      patchTask(data.task_id, {
        progress_percent: data.percent,
        file_size: data.downloaded_bytes,
        current_speed: data.speed,
      });
    });

    const unsubscribeUpdate = wsClient.onTaskUpdate((data: TaskUpdateData) => {
      setLiveProgress((prev) => {
        const next = new Map(prev);
        next.delete(data.task_id);
        return next;
      });
      patchTask(data.task_id, {
        status: data.status,
        error_message: data.error_message,
        ...(data.status === 'completed' ? { progress_percent: 100 } : {}),
        ...(data.status === 'running' ? {} : { current_speed: null }),
      });
      void queryClient.invalidateQueries({ queryKey: ['tasks'] });
    });

    return () => {
      unsubscribeProgress();
      unsubscribeUpdate();
    };
  }, [queryClient]);

  const channelMap = useMemo(() => {
    const map = new Map<string, string>();
    channels?.forEach((channel) => {
      map.set(channel.id, channel.name);
    });
    return map;
  }, [channels]);

  const selectedTask = useMemo(
    () => tasks?.find((task) => task.id === selectedTaskId) ?? null,
    [tasks, selectedTaskId],
  );

  const cancelMutation = useMutation({
    mutationFn: cancelTask,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tasks'] });
      toast.success(t('common:toast.taskCancelled'));
    },
    onError: (error) => {
      toast.error(t('common:toast.operationFailed', { message: (error as Error).message }));
    },
  });

  const clearMutation = useMutation({
    mutationFn: clearCompletedTasks,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tasks'] });
      setClearConfirm(false);
      toast.success(t('common:toast.tasksCleared'));
    },
    onError: (error) => {
      toast.error(t('common:toast.operationFailed', { message: (error as Error).message }));
    },
  });

  const deleteMutation = useMutation({
    mutationFn: deleteTask,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tasks'] });
      setDeleteConfirm({ isOpen: false, taskId: null });
      toast.success(t('common:toast.taskDeleted'));
    },
    onError: (error) => {
      toast.error(t('common:toast.operationFailed', { message: (error as Error).message }));
    },
  });

  // Merge live progress into tasks for real-time updates
  const tasksWithLiveProgress = useMemo(() => {
    if (!tasks) return undefined;
    if (liveProgress.size === 0) return tasks;
    return tasks.map((task) => {
      const live = liveProgress.get(task.id);
      if (!live) return task;
      return { ...task, progress_percent: live.percent, current_speed: live.speed, file_size: live.downloaded };
    });
  }, [tasks, liveProgress]);

  const filteredTasks = tasksWithLiveProgress?.filter((task) => {
    if (filterStatus === 'all') return true;
    return task.status === filterStatus;
  });

  const runningCount = tasksWithLiveProgress?.filter((task) => task.status === 'running').length || 0;
  const completedCount = tasksWithLiveProgress?.filter((task) => task.status === 'completed').length || 0;
  const failedCount = tasksWithLiveProgress?.filter((task) => task.status === 'failed').length || 0;

  const statusLabel = (status: TaskStatus) => t(`common:taskStatus.${status}`);
  const getChannelName = (channelId: string) =>
    channelMap.get(channelId) || t('common:channelFallback', { id: channelId.slice(0, 8) });

  const formatDuration = (seconds: number) => {
    if (!seconds) return '-';
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = seconds % 60;
    if (hours > 0) return `${hours}:${String(minutes).padStart(2, '0')}:${String(secs).padStart(2, '0')}`;
    return `${minutes}:${String(secs).padStart(2, '0')}`;
  };

  if (!isI18nReady) {
    return <div className="page-loading">{t('common:loading')}</div>;
  }

  const shouldRenderTaskDetail = selectedTask !== null;
  const shouldRenderDeleteConfirm = deleteConfirm.isOpen;
  const shouldRenderClearConfirm = clearConfirm;
  const isLive = connectionState === 'connected';
  const liveLabel = t(`tasks:live.${connectionState}`);

  return (
    <div className="tasks-page">
      <div className="page-header">
        <div className="page-title">
          <h1>{t('tasks:title')}</h1>
          <p className="page-subtitle">{t('tasks:subtitle')}</p>
        </div>
        <div className="page-actions">
          <div className={`live-pill ${isLive ? 'connected' : 'offline'}`}>
            <Radio size={14} />
            {liveLabel}
          </div>
          <button
            className="btn btn-ghost"
            onClick={() => setClearConfirm(true)}
            disabled={clearMutation.isPending}
            title={t('tasks:clearTooltip')}
          >
            <Trash2 size={16} />
            {t('tasks:clearRecords')}
          </button>
          <button className="btn btn-ghost" onClick={() => refetch()}>
            <RefreshCw size={16} />
            {t('common:refresh')}
          </button>
        </div>
      </div>

      <div className="stats-bar">
        <div className="stat-item">
          <div className="stat-number recording">{runningCount}</div>
          <div className="stat-label">{t('tasks:running')}</div>
        </div>
        <div className="stat-item">
          <div className="stat-number success">{completedCount}</div>
          <div className="stat-label">{t('tasks:completed')}</div>
        </div>
        <div className="stat-item">
          <div className="stat-number error">{failedCount}</div>
          <div className="stat-label">{t('tasks:failed')}</div>
        </div>
      </div>

      <div className="filter-tabs">
        <button
          className={`tab-btn ${filterStatus === 'all' ? 'active' : ''}`}
          onClick={() => setFilterStatus('all')}
        >
          {t('tasks:all')}
        </button>
        {Object.entries(statusMeta).map(([status, meta]) => (
          <button
            key={status}
            className={`tab-btn ${filterStatus === status ? 'active' : ''}`}
            onClick={() => setFilterStatus(status as TaskStatus)}
          >
            {meta.icon}
            {statusLabel(status as TaskStatus)}
          </button>
        ))}
      </div>

      {isLoading ? (
        <div className="loading-list">
          {[1, 2, 3, 4, 5].map((item) => (
            <div key={item} className="task-skeleton card animate-shimmer" />
          ))}
        </div>
      ) : filteredTasks && filteredTasks.length > 0 ? (
        <div className="tasks-list">
          {filteredTasks.map((task, index) => {
            const meta = statusMeta[task.status];
            const isRunning = task.status === 'running';
            const channelName = getChannelName(task.channel_id);
            const live = liveProgress.get(task.id);
            const displayProgress = isRunning && live ? live.percent : task.progress_percent;
            const displaySpeed = isRunning && live ? live.speed : task.current_speed;

            return (
              <div
                key={task.id}
                className={`task-card card stagger-item ${isRunning ? 'recording' : ''}`}
                style={{ animationDelay: `${index * 0.03}s` }}
              >
                <div className="task-left">
                  <div className={`task-status-indicator ${meta.color}`}>
                    {meta.icon}
                  </div>

                  <div className="task-info">
                    <div className="task-title">
                      {channelName}
                    </div>
                    <div className="task-meta">
                      <span className={`badge badge-${meta.color}`}>
                        {statusLabel(task.status)}
                      </span>
                      {isRunning && displaySpeed && (
                        <span className="speed-indicator">{displaySpeed}</span>
                      )}
                      {task.started_at && (
                        <span className="task-time">
                          {formatShortDateTime(task.started_at, i18n.language as AppLanguage)}
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
                          style={{ width: `${displayProgress}%` }}
                        />
                      </div>
                      <span className="progress-percent">{Math.round(displayProgress)}%</span>
                    </div>
                  ) : (
                    <div className="task-stats">
                      <div className="stat">
                        <span className="stat-value">{formatDuration(task.duration_recorded)}</span>
                        <span className="stat-label">{t('tasks:duration')}</span>
                      </div>
                      <div className="stat">
                        <span className="stat-value">{task.file_size ? formatBytes(task.file_size, i18n.language as AppLanguage) : '-'}</span>
                        <span className="stat-label">{t('tasks:size')}</span>
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
                      {t('tasks:stop')}
                    </button>
                  ) : (
                    <>
                      {task.status === 'completed' && (
                        <button
                          className="btn btn-ghost btn-sm"
                          onClick={() => setSelectedTaskId(task.id)}
                        >
                          <Eye size={14} />
                          {t('tasks:view')}
                        </button>
                      )}
                      {task.status === 'failed' && task.error_message && (
                        <div
                          className="error-info clickable"
                          title={task.error_message}
                          onClick={() => setSelectedTaskId(task.id)}
                        >
                          <AlertCircle size={16} className="error-icon" />
                        </div>
                      )}
                      <button
                        className="btn btn-ghost btn-sm btn-delete"
                        onClick={() => setDeleteConfirm({ isOpen: true, taskId: task.id })}
                        title={t('tasks:deleteRecord')}
                      >
                        <Trash2 size={14} />
                      </button>
                    </>
                  )}
                  <button
                    className="action-btn"
                    onClick={() => setSelectedTaskId(task.id)}
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
          <div className="empty-title">{t('tasks:emptyTitle')}</div>
          <div className="empty-desc">
            {filterStatus === 'all'
              ? t('tasks:emptyAll')
              : t('tasks:emptyFiltered', { status: statusLabel(filterStatus as TaskStatus) })}
          </div>
        </div>
      )}

      <Suspense fallback={null}>
        {shouldRenderTaskDetail && (
          <TaskDetailModal
            isOpen
            onClose={() => setSelectedTaskId(null)}
            task={selectedTask}
            channelName={selectedTask ? channelMap.get(selectedTask.channel_id) : undefined}
          />
        )}

        {shouldRenderDeleteConfirm && (
          <ConfirmDialog
            isOpen={deleteConfirm.isOpen}
            onClose={() => setDeleteConfirm({ isOpen: false, taskId: null })}
            onConfirm={() => deleteConfirm.taskId && deleteMutation.mutate(deleteConfirm.taskId)}
            title={t('tasks:deleteRecordTitle')}
            message={t('tasks:deleteRecordMessage')}
            confirmText={t('common:delete')}
            type="danger"
            isLoading={deleteMutation.isPending}
          />
        )}

        {shouldRenderClearConfirm && (
          <ConfirmDialog
            isOpen={clearConfirm}
            onClose={() => setClearConfirm(false)}
            onConfirm={() => clearMutation.mutate()}
            title={t('tasks:clearRecordsTitle')}
            message={t('tasks:clearRecordsMessage')}
            confirmText={t('common:clear')}
            type="warning"
            isLoading={clearMutation.isPending}
          />
        )}
      </Suspense>
    </div>
  );
};

export default Tasks;
