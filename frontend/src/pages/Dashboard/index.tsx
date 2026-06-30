import React from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getTasks, cancelTask } from '@/api/tasks';
import { getChannels } from '@/api/channels';
import { getSchedules } from '@/api/schedules';
import { getUpcoming } from '@/api/system';
import { formatBytes, formatMinutes, formatShortDateTime } from '@/i18n/format';
import { useI18nNamespace } from '@/i18n/useI18nNamespace';
import { channelKeys, scheduleKeys, taskKeys, upcomingKeys } from '@/lib/queryKeys';
import type { AppLanguage } from '@/i18n/types';
import {
  Tv,
  CalendarClock,
  CircleDot,
  HardDrive,
  RefreshCw,
  StopCircle,
  Clock,
  ArrowUpRight,
  MoreHorizontal,
} from 'lucide-react';
import type { Task } from '@/types';
import './Dashboard.css';

interface StatCardProps {
  icon: React.ReactNode;
  label: string;
  value: string | number;
  trend?: string;
  trendUp?: boolean;
  gradient: string;
}

const StatCard: React.FC<StatCardProps> = ({ icon, label, value, trend, trendUp, gradient }) => (
  <div className={`stat-card ${gradient}`}>
    <div className="stat-icon">{icon}</div>
    <div className="stat-content">
      <div className="stat-value">{value}</div>
      <div className="stat-label">{label}</div>
      {trend && (
        <div className={`stat-trend ${trendUp ? 'up' : 'down'}`}>
          <ArrowUpRight size={12} />
          {trend}
        </div>
      )}
    </div>
  </div>
);

interface TaskRowProps {
  task: Task;
  channelName: string;
  onStop?: () => void;
}

const TaskRow: React.FC<TaskRowProps> = ({ task, channelName, onStop }) => {
  const { t, i18n } = useTranslation(['common']);
  const isRunning = task.status === 'running';

  return (
    <div className="task-row">
      <div className="task-channel">
        <div className="channel-avatar">
          <Tv size={16} />
        </div>
        <div className="channel-info">
          <div className="channel-name">
            {channelName}
          </div>
          <div className="channel-meta">
            {isRunning && task.current_speed && (
              <span className="speed">{task.current_speed}</span>
            )}
          </div>
        </div>
      </div>

      <div className="task-status">
        {isRunning ? (
          <div className="badge badge-recording">
            <span className="recording-dot" />
            {t('common:taskStatus.running')}
          </div>
        ) : task.status === 'completed' ? (
          <div className="badge badge-success">{t('common:taskStatus.completed')}</div>
        ) : task.status === 'failed' ? (
          <div className="badge badge-error">{t('common:taskStatus.failed')}</div>
        ) : task.status === 'cancelled' ? (
          <div className="badge badge-neutral">{t('common:taskStatus.cancelled')}</div>
        ) : (
          <div className="badge badge-neutral">{t('common:taskStatus.pending')}</div>
        )}
      </div>

      {isRunning && (
        <div className="task-progress">
          <div className="progress-bar progress-bar-recording">
            <div
              className="progress-bar-fill"
              style={{ width: `${task.progress_percent}%` }}
            />
          </div>
          <span className="progress-text">{task.progress_percent}%</span>
        </div>
      )}

      <div className="task-duration">
        {formatMinutes(task.duration_recorded, t)}
      </div>

      <div className="task-size">
        {task.file_size > 0 ? formatBytes(task.file_size, i18n.language as AppLanguage) : '-'}
      </div>

      <div className="task-actions">
        {isRunning && (
          <button className="action-btn danger" onClick={onStop}>
            <StopCircle size={16} />
          </button>
        )}
        <button className="action-btn">
          <MoreHorizontal size={16} />
        </button>
      </div>
    </div>
  );
};

export const Dashboard: React.FC = () => {
  const { t, i18n } = useTranslation(['dashboard', 'common']);
  const isI18nReady = useI18nNamespace(['dashboard', 'common']);
  const queryClient = useQueryClient();

  // 频道总数(getChannels 分页查询,只取 .total 给 stat 卡片)。
  // 此前还额外发一次 getAllChannels 全量拉取只为建 channelMap 查 channel_id→name,
  // 现任务列表 JOIN 带 channel_name,全量拉取已删除。
  const { data: channels } = useQuery({
    queryKey: channelKeys.count(),
    queryFn: () => getChannels({ page_size: 1 }),
  });

  const { data: schedules } = useQuery({
    queryKey: scheduleKeys.all(),
    queryFn: getSchedules,
  });

  const { data: tasksData, isLoading: tasksLoading, refetch } = useQuery({
    queryKey: taskKeys.list({ page: 1, page_size: 100 }),
    queryFn: () => getTasks({ page: 1, page_size: 100 }),
  });
  const tasks = tasksData?.items;

  const { data: upcoming, isLoading: upcomingLoading } = useQuery({
    queryKey: upcomingKeys.upcoming(),
    queryFn: getUpcoming,
    refetchInterval: 10000,
  });

  const cancelMutation = useMutation({
    mutationFn: cancelTask,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: taskKeys.root });
    },
  });

  const runningTasks = tasks?.filter((task) => task.status === 'running') || [];
  const completedTasks = tasks?.filter((task) => task.status === 'completed').slice(0, 5) || [];
  const failedCount = tasks?.filter((task) => task.status === 'failed').length || 0;
  const totalStorage = tasks?.reduce((sum, task) => sum + (task.file_size || 0), 0) || 0;
  const enabledSchedules = schedules?.filter((schedule) => schedule.enabled).length || 0;

  // 频道名直接读 task.channel_name(列表接口 JOIN channels 带),省去全量频道拉取。
  const getChannelName = (task: { channel_id: string; channel_name?: string }) =>
    task.channel_name || t('common:channelFallback', { id: task.channel_id.slice(0, 8) });

  if (!isI18nReady) {
    return <div className="page-loading">{t('common:loading')}</div>;
  }

  return (
    <div className="dashboard">
      <div className="page-header">
        <div className="page-title">
          <h1>{t('dashboard:title')}</h1>
          <p className="page-subtitle">{t('dashboard:subtitle')}</p>
        </div>
        <button className="btn btn-ghost" onClick={() => refetch()}>
          <RefreshCw size={16} />
          {t('common:refresh')}
        </button>
      </div>

      <div className="stats-grid">
        <StatCard
          icon={<Tv size={24} />}
          label={t('dashboard:totalChannels')}
          value={channels?.total || 0}
          gradient="stat-bg-blue"
        />
        <StatCard
          icon={<CalendarClock size={24} />}
          label={t('dashboard:totalSchedules')}
          value={schedules?.length || 0}
          trend={`${enabledSchedules} ${t('common:enabled')}`}
          trendUp={enabledSchedules > 0}
          gradient="stat-bg-green"
        />
        <StatCard
          icon={<CircleDot size={24} />}
          label={t('dashboard:todayTasks')}
          value={runningTasks.length}
          gradient="stat-bg-rose"
        />
        <StatCard
          icon={<HardDrive size={24} />}
          label={t('dashboard:storageUsed')}
          value={formatBytes(totalStorage, i18n.language as AppLanguage)}
          trend={failedCount > 0 ? t('dashboard:failedTasks', { count: failedCount }) : t('dashboard:noFailedTasks')}
          trendUp={failedCount === 0}
          gradient="stat-bg-amber"
        />
      </div>

      <div className="content-grid">
        <div className="card recording-card">
          <div className="card-header">
            <h3>
              <CircleDot size={18} className="recording-icon" />
              {t('dashboard:recording')}
            </h3>
            <span className="task-count">{t('dashboard:taskCount', { count: runningTasks.length })}</span>
          </div>
          <div className="card-body">
            {tasksLoading ? (
              <div className="loading-skeleton">
                {[1, 2, 3].map((item) => (
                  <div key={item} className="skeleton-row animate-shimmer" />
                ))}
              </div>
            ) : runningTasks.length === 0 ? (
              <div className="empty-state">
                <div className="empty-icon">
                  <CircleDot size={48} strokeWidth={1} />
                </div>
                <div className="empty-title">{t('dashboard:noRecordingTasks')}</div>
                <div className="empty-desc">{t('dashboard:allTasksCompleted')}</div>
              </div>
            ) : (
              <div className="task-list">
                <div className="task-list-header">
                  <span className="col-channel">{t('common:channel')}</span>
                  <span className="col-status">{t('common:status')}</span>
                  <span className="col-progress">{t('common:progress')}</span>
                  <span className="col-duration">{t('common:duration')}</span>
                  <span className="col-size">{t('common:size')}</span>
                  <span className="col-actions"></span>
                </div>
                {runningTasks.map((task, index) => (
                  <div key={task.id} className="stagger-item" style={{ animationDelay: `${index * 0.05}s` }}>
                    <TaskRow
                      task={task}
                      channelName={getChannelName(task)}
                      onStop={() => cancelMutation.mutate(task.id)}
                    />
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>

        <div className="card upcoming-card">
          <div className="card-header">
            <h3>
              <Clock size={18} />
              {t('dashboard:upcoming')}
            </h3>
          </div>
          <div className="card-body">
            {upcomingLoading ? (
              <div className="timeline-skeleton">
                {[1, 2, 3, 4].map((item) => (
                  <div key={item} className="skeleton-timeline animate-shimmer" />
                ))}
              </div>
            ) : upcoming && upcoming.length > 0 ? (
              <div className="timeline">
                {upcoming.slice(0, 5).map((task, index) => (
                  <div key={task.schedule_id} className="timeline-item stagger-item" style={{ animationDelay: `${index * 0.08}s` }}>
                    <div className="timeline-dot" />
                    <div className="timeline-content">
                      <div className="timeline-title">{task.schedule_name}</div>
                      <div className="timeline-meta">
                        <span className="channel">{getChannelName(task)}</span>
                        <span className="time">
                          {formatShortDateTime(task.next_run, i18n.language as AppLanguage)}
                        </span>
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <div className="empty-state compact">
                <div className="empty-icon">
                  <Clock size={36} strokeWidth={1} />
                </div>
                <div className="empty-title">{t('dashboard:noUpcomingTasks')}</div>
              </div>
            )}
          </div>
        </div>
      </div>

      <div className="card recent-card">
        <div className="card-header">
          <h3>
            <HardDrive size={18} />
            {t('dashboard:recentCompleted')}
          </h3>
          <button className="btn btn-ghost btn-sm">{t('common:viewAll')}</button>
        </div>
        <div className="card-body">
          {completedTasks.length > 0 ? (
            <div className="recent-grid">
              {completedTasks.map((task, index) => (
                <div key={task.id} className="recent-item stagger-item" style={{ animationDelay: `${index * 0.05}s` }}>
                  <div className="recent-thumbnail">
                    <Tv size={20} />
                  </div>
                  <div className="recent-info">
                    <div className="recent-name">
                      {getChannelName(task)}
                    </div>
                    <div className="recent-meta">
                      <span>{formatMinutes(task.duration_recorded, t)}</span>
                      <span>{task.file_size ? formatBytes(task.file_size, i18n.language as AppLanguage) : '-'}</span>
                    </div>
                  </div>
                  <div className="badge badge-success">{t('common:taskStatus.completed')}</div>
                </div>
              ))}
            </div>
          ) : (
            <div className="empty-state">
              <div className="empty-icon">
                <HardDrive size={48} strokeWidth={1} />
              </div>
              <div className="empty-title">{t('dashboard:noRecentRecordings')}</div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

export default Dashboard;
