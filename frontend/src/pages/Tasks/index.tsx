import React, { Suspense, lazy, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getTasks, cancelTask, clearCompletedTasks, deleteTask } from '@/api/tasks';
import { wsClient, type ConnectionState } from '@/api/websocket';
import { toast } from '@/stores/toastStore';
import { useI18nNamespace } from '@/i18n/useI18nNamespace';
import { taskKeys } from '@/lib/queryKeys';
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
  ChevronLeft,
} from 'lucide-react';
import type { TaskStatus } from '@/types';
import type { Task } from '@/types';
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
  // 状态筛选 + 分页都下推到后端(GET /tasks?status=&page=&page_size=)。
  // 之前是全量拉取 + 前端过滤 + 维护本地 liveProgress Map,现改为信任 React Query
  // 缓存(由 App.tsx 全局 WS 处理器经 taskRealtime 实时补丁)。
  const [filterStatus, setFilterStatus] = useState<FilterStatus>('all');
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const [selectedTaskId, setSelectedTaskId] = useState<string | null>(null);
  const [deleteConfirm, setDeleteConfirm] = useState<{ isOpen: boolean; taskId: string | null }>({
    isOpen: false,
    taskId: null,
  });
  const [clearConfirm, setClearConfirm] = useState(false);
  const [connectionState, setConnectionState] = useState<ConnectionState>(
    wsClient.getConnectionState(),
  );

  // 列表查询:状态筛选/分页参数化缓存。filterStatus/page/pageSize 任一变化即独立缓存。
  const tasksQueryParams = useMemo(
    () => ({
      status: filterStatus === 'all' ? undefined : filterStatus,
      page,
      page_size: pageSize,
    }),
    [filterStatus, page, pageSize],
  );

  const { data: tasksData, isLoading, refetch } = useQuery({
    queryKey: taskKeys.list(tasksQueryParams),
    queryFn: () => getTasks(tasksQueryParams),
    refetchInterval: (query) => {
      const data = query.state.data;
      return data?.items.some((task) => task.status === 'running') ? 3000 : false;
    },
    refetchIntervalInBackground: true,
  });

  const tasks = tasksData?.items;
  const totalPages = tasksData?.total_pages ?? 1;
  const total = tasksData?.total ?? 0;

  useEffect(() => wsClient.onConnectionStateChange(setConnectionState), []);

  // 注:不再订阅 onTaskProgress/onTaskUpdate——App.tsx 已全局订阅并通过 taskRealtime
  // (setQueriesData 按 taskKeys.root 前缀遍历所有任务缓存)实时补丁,这里直接读缓存即可。

  const selectedTask = useMemo(
    () => tasks?.find((task) => task.id === selectedTaskId) ?? null,
    [tasks, selectedTaskId],
  );

  const cancelMutation = useMutation({
    mutationFn: cancelTask,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: taskKeys.root });
      toast.success(t('common:toast.taskCancelled'));
    },
    onError: (error) => {
      toast.error(t('common:toast.operationFailed', { message: (error as Error).message }));
    },
  });

  const clearMutation = useMutation({
    mutationFn: clearCompletedTasks,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: taskKeys.root });
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
      queryClient.invalidateQueries({ queryKey: taskKeys.root });
      setDeleteConfirm({ isOpen: false, taskId: null });
      toast.success(t('common:toast.taskDeleted'));
    },
    onError: (error) => {
      toast.error(t('common:toast.operationFailed', { message: (error as Error).message }));
    },
  });

  // 顶部计数:基于当前筛选下的总数(total 来自后端,无需前端再算)。
  // running/completed/failed 计数仅对"全部"视图有意义(按状态筛选时 total 即该状态数)。
  const runningCount = filterStatus === 'all' ? tasks?.filter((task) => task.status === 'running').length ?? 0 : 0;

  const statusLabel = (status: TaskStatus) => t(`common:taskStatus.${status}`);
  // 频道名直接读 task.channel_name(列表接口已 JOIN channels 带),省去全量频道拉取。
  const getChannelName = (task: Task) =>
    task.channel_name || t('common:channelFallback', { id: task.channel_id.slice(0, 8) });

  const handleFilterChange = (status: FilterStatus) => {
    setFilterStatus(status);
    setPage(1); // 切换状态筛选回到第一页
  };

  const handlePageChange = (newPage: number) => {
    setPage(newPage);
  };

  const renderPagination = () => {
    if (!tasksData || totalPages <= 1) return null;

    const pages = [];
    const maxVisiblePages = 5;
    let startPage = Math.max(1, page - Math.floor(maxVisiblePages / 2));
    const endPage = Math.min(totalPages, startPage + maxVisiblePages - 1);

    if (endPage - startPage + 1 < maxVisiblePages) {
      startPage = Math.max(1, endPage - maxVisiblePages + 1);
    }

    for (let i = startPage; i <= endPage; i++) {
      pages.push(i);
    }

    return (
      <div className="pagination">
        <button
          className="pagination-btn"
          onClick={() => handlePageChange(page - 1)}
          disabled={page <= 1}
          aria-label={t('common:previousPage', { defaultValue: '上一页' })}
        >
          <ChevronLeft size={16} />
        </button>

        {startPage > 1 && (
          <>
            <button className="pagination-btn" onClick={() => handlePageChange(1)}>1</button>
            {startPage > 2 && <span className="pagination-ellipsis">...</span>}
          </>
        )}

        {pages.map((pageNumber) => (
          <button
            key={pageNumber}
            className={`pagination-btn ${pageNumber === page ? 'active' : ''}`}
            onClick={() => handlePageChange(pageNumber)}
          >
            {pageNumber}
          </button>
        ))}

        {endPage < totalPages && (
          <>
            {endPage < totalPages - 1 && <span className="pagination-ellipsis">...</span>}
            <button className="pagination-btn" onClick={() => handlePageChange(totalPages)}>
              {totalPages}
            </button>
          </>
        )}

        <button
          className="pagination-btn"
          onClick={() => handlePageChange(page + 1)}
          disabled={page >= totalPages}
          aria-label={t('common:nextPage', { defaultValue: '下一页' })}
        >
          <ChevronRight size={16} />
        </button>

        <select
          className="pagination-size"
          value={pageSize}
          onChange={(e) => {
            setPageSize(Number(e.target.value));
            setPage(1);
          }}
        >
          {[10, 20, 50, 100].map((size) => (
            <option key={size} value={size}>{t('tasks:pageSize', { count: size })}</option>
          ))}
        </select>
        <span className="pagination-total">{t('tasks:total', { count: total })}</span>
      </div>
    );
  };

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
          <div className="stat-number">{total}</div>
          <div className="stat-label">{filterStatus === 'all' ? t('tasks:all') : statusLabel(filterStatus as TaskStatus)}</div>
        </div>
      </div>

      <div className="filter-tabs">
        <button
          className={`tab-btn ${filterStatus === 'all' ? 'active' : ''}`}
          onClick={() => handleFilterChange('all')}
        >
          {t('tasks:all')}
        </button>
        {Object.entries(statusMeta).map(([status, meta]) => (
          <button
            key={status}
            className={`tab-btn ${filterStatus === status ? 'active' : ''}`}
            onClick={() => handleFilterChange(status as TaskStatus)}
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
      ) : tasks && tasks.length > 0 ? (
        <div className="tasks-list">
          {tasks.map((task, index) => {
            const meta = statusMeta[task.status];
            const isRunning = task.status === 'running';
            const channelName = getChannelName(task);
            const displayProgress = task.progress_percent;
            const displaySpeed = task.current_speed;

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
                        aria-label={t('tasks:deleteRecord')}
                      >
                        <Trash2 size={14} />
                      </button>
                    </>
                  )}
                  <button
                    className="action-btn"
                    onClick={() => setSelectedTaskId(task.id)}
                    aria-label={t('tasks:view', { defaultValue: '查看详情' })}
                    title={t('tasks:view', { defaultValue: '查看详情' })}
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

      {renderPagination()}

      <Suspense fallback={null}>
        {shouldRenderTaskDetail && (
          <TaskDetailModal
            isOpen
            onClose={() => setSelectedTaskId(null)}
            task={selectedTask}
            channelName={selectedTask ? getChannelName(selectedTask) : undefined}
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
