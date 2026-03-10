import React from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getTasks, cancelTask } from '@/api/tasks';
import { getChannels } from '@/api/channels';
import { getSchedules } from '@/api/schedules';
import { getUpcoming } from '@/api/system';
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
  onStop?: () => void;
}

const TaskRow: React.FC<TaskRowProps> = ({ task, onStop }) => {
  const isRunning = task.status === 'running';

  return (
    <div className="task-row">
      <div className="task-channel">
        <div className="channel-avatar">
          <Tv size={16} />
        </div>
        <div className="channel-info">
          <div className="channel-name">
            {task.channel_id.slice(0, 8)}...
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
            录制中
          </div>
        ) : task.status === 'completed' ? (
          <div className="badge badge-success">已完成</div>
        ) : task.status === 'failed' ? (
          <div className="badge badge-error">失败</div>
        ) : (
          <div className="badge badge-neutral">等待中</div>
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
        {task.duration_recorded > 0
          ? `${Math.floor(task.duration_recorded / 60)} min`
          : '-'}
      </div>

      <div className="task-size">
        {task.file_size > 0
          ? `${(task.file_size / 1024 / 1024).toFixed(1)} MB`
          : '-'}
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

const formatFileSize = (bytes: number): string => {
  if (bytes === 0) return '0 B';
  if (bytes >= 1024 * 1024 * 1024) {
    return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
  }
  if (bytes >= 1024 * 1024) {
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  }
  return `${(bytes / 1024).toFixed(1)} KB`;
};

export const Dashboard: React.FC = () => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();

  const { data: channels } = useQuery({
    queryKey: ['channels', 'count'],
    queryFn: () => getChannels({ page_size: 1 }), // 只需要总数
  });

  const { data: schedules } = useQuery({
    queryKey: ['schedules'],
    queryFn: getSchedules,
  });

  const { data: tasks, isLoading: tasksLoading, refetch } = useQuery({
    queryKey: ['tasks'],
    queryFn: getTasks,
    refetchInterval: 5000,
  });

  const { data: upcoming, isLoading: upcomingLoading } = useQuery({
    queryKey: ['upcoming'],
    queryFn: getUpcoming,
    refetchInterval: 10000,
  });

  const cancelMutation = useMutation({
    mutationFn: cancelTask,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tasks'] });
    },
  });

  const runningTasks = tasks?.filter((t) => t.status === 'running') || [];
  const completedTasks = tasks?.filter((t) => t.status === 'completed').slice(0, 5) || [];
  const failedCount = tasks?.filter((t) => t.status === 'failed').length || 0;

  // 计算总存储占用
  const totalStorage = tasks?.reduce((sum, t) => sum + (t.file_size || 0), 0) || 0;

  // 启用的计划数
  const enabledSchedules = schedules?.filter((s) => s.enabled).length || 0;

  return (
    <div className="dashboard">
      {/* Page Header */}
      <div className="page-header">
        <div className="page-title">
          <h1>{t('dashboard.title')}</h1>
          <p className="page-subtitle">系统运行状态概览</p>
        </div>
        <button className="btn btn-ghost" onClick={() => refetch()}>
          <RefreshCw size={16} />
          刷新
        </button>
      </div>

      {/* Stats Grid */}
      <div className="stats-grid">
        <StatCard
          icon={<Tv size={24} />}
          label={t('dashboard.totalChannels')}
          value={channels?.total || 0}
          gradient="stat-bg-blue"
        />
        <StatCard
          icon={<CalendarClock size={24} />}
          label={t('dashboard.totalSchedules')}
          value={schedules?.length || 0}
          trend={`${enabledSchedules} 启用`}
          trendUp={enabledSchedules > 0}
          gradient="stat-bg-green"
        />
        <StatCard
          icon={<CircleDot size={24} />}
          label={t('dashboard.todayTasks')}
          value={runningTasks.length}
          gradient="stat-bg-rose"
        />
        <StatCard
          icon={<HardDrive size={24} />}
          label={t('dashboard.storageUsed')}
          value={formatFileSize(totalStorage)}
          trend={failedCount > 0 ? `${failedCount} 失败` : '无失败'}
          trendUp={failedCount === 0}
          gradient="stat-bg-amber"
        />
      </div>

      {/* Main Content Grid */}
      <div className="content-grid">
        {/* Running Tasks */}
        <div className="card recording-card">
          <div className="card-header">
            <h3>
              <CircleDot size={18} className="recording-icon" />
              {t('dashboard.recording')}
            </h3>
            <span className="task-count">{runningTasks.length} 个任务</span>
          </div>
          <div className="card-body">
            {tasksLoading ? (
              <div className="loading-skeleton">
                {[1, 2, 3].map((i) => (
                  <div key={i} className="skeleton-row animate-shimmer" />
                ))}
              </div>
            ) : runningTasks.length === 0 ? (
              <div className="empty-state">
                <div className="empty-icon">
                  <CircleDot size={48} strokeWidth={1} />
                </div>
                <div className="empty-title">暂无录制任务</div>
                <div className="empty-desc">所有录制任务已完成或等待执行</div>
              </div>
            ) : (
              <div className="task-list">
                <div className="task-list-header">
                  <span className="col-channel">频道</span>
                  <span className="col-status">状态</span>
                  <span className="col-progress">进度</span>
                  <span className="col-duration">时长</span>
                  <span className="col-size">大小</span>
                  <span className="col-actions"></span>
                </div>
                {runningTasks.map((task, idx) => (
                  <div key={task.id} className="stagger-item" style={{ animationDelay: `${idx * 0.05}s` }}>
                    <TaskRow task={task} onStop={() => cancelMutation.mutate(task.id)} />
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>

        {/* Upcoming Tasks */}
        <div className="card upcoming-card">
          <div className="card-header">
            <h3>
              <Clock size={18} />
              {t('dashboard.upcoming')}
            </h3>
          </div>
          <div className="card-body">
            {upcomingLoading ? (
              <div className="timeline-skeleton">
                {[1, 2, 3, 4].map((i) => (
                  <div key={i} className="skeleton-timeline animate-shimmer" />
                ))}
              </div>
            ) : upcoming && upcoming.length > 0 ? (
              <div className="timeline">
                {upcoming.slice(0, 5).map((task, idx) => (
                  <div key={task.schedule_id} className="timeline-item stagger-item" style={{ animationDelay: `${idx * 0.08}s` }}>
                    <div className="timeline-dot" />
                    <div className="timeline-content">
                      <div className="timeline-title">{task.schedule_name}</div>
                      <div className="timeline-meta">
                        <span className="channel">{task.channel_id.slice(0, 8)}...</span>
                        <span className="time">
                          {new Date(task.next_run).toLocaleString('zh-CN', {
                            month: 'numeric',
                            day: 'numeric',
                            hour: '2-digit',
                            minute: '2-digit',
                          })}
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
                <div className="empty-title">暂无即将执行的任务</div>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Recent Recordings */}
      <div className="card recent-card">
        <div className="card-header">
          <h3>
            <HardDrive size={18} />
            {t('dashboard.recentCompleted')}
          </h3>
          <button className="btn btn-ghost btn-sm">查看全部</button>
        </div>
        <div className="card-body">
          {completedTasks.length > 0 ? (
            <div className="recent-grid">
              {completedTasks.map((task, idx) => (
                <div key={task.id} className="recent-item stagger-item" style={{ animationDelay: `${idx * 0.05}s` }}>
                  <div className="recent-thumbnail">
                    <Tv size={20} />
                  </div>
                  <div className="recent-info">
                    <div className="recent-name">
                      Channel {task.channel_id.slice(0, 8)}...
                    </div>
                    <div className="recent-meta">
                      <span>{task.duration_recorded ? `${Math.floor(task.duration_recorded / 60)} min` : '-'}</span>
                      <span>{task.file_size ? `${(task.file_size / 1024 / 1024).toFixed(1)} MB` : '-'}</span>
                    </div>
                  </div>
                  <div className="badge badge-success">完成</div>
                </div>
              ))}
            </div>
          ) : (
            <div className="empty-state">
              <div className="empty-icon">
                <HardDrive size={48} strokeWidth={1} />
              </div>
              <div className="empty-title">暂无最近录制</div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

export default Dashboard;
