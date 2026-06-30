import React, { Suspense, lazy, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getSchedules, deleteSchedule, toggleSchedule } from '@/api/schedules';
import { startManualRecord } from '@/api/tasks';
import { upsertTaskCache } from '@/lib/taskRealtime';
import { toast } from '@/stores/toastStore';
import { useI18nNamespace } from '@/i18n/useI18nNamespace';
import { scheduleKeys, taskKeys } from '@/lib/queryKeys';
import {
  Plus,
  CalendarClock,
  Clock,
  ToggleLeft,
  ToggleRight,
  Play,
  Pencil,
  Trash2,
  ChevronDown,
  Loader2,
} from 'lucide-react';
import type { Schedule } from '@/types';
import './Schedules.css';

const ScheduleModal = lazy(() => import('@/components/ScheduleModal'));

const CronDescription: React.FC<{ expression: string }> = ({ expression }) => {
  const { t } = useTranslation(['schedules']);
  const parts = expression.trim().split(/\s+/);
  if (parts.length < 5) return <span>{expression}</span>;

  const [min, hour, dom, , weekday] = parts;
  const dayNames = t('schedules:cron.days', { returnObjects: true }) as string[];
  const pad = (s: string) => s.padStart(2, '0');
  const timeStr = `${pad(hour)}:${pad(min)}`;

  if (min === '*' && hour === '*') return <span>{t('schedules:cron.everyMinute')}</span>;

  const everyMinMatch = min.match(/^\*\/(\d+)$/);
  if (everyMinMatch && hour === '*') {
    return <span>{t('schedules:cron.everyNMinutes', { count: Number(everyMinMatch[1]) })}</span>;
  }

  if (min === '0' && hour === '*') return <span>{t('schedules:cron.everyHour')}</span>;

  const everyHourMatch = hour.match(/^\*\/(\d+)$/);
  if (everyHourMatch && min === '0') {
    return <span>{t('schedules:cron.everyNHours', { count: Number(everyHourMatch[1]) })}</span>;
  }

  if (!/^\d+$/.test(min) || !/^\d+$/.test(hour)) return <span>{expression}</span>;

  if (weekday === '1-5') return <span>{t('schedules:cron.weekday', { time: timeStr })}</span>;
  if (weekday === '0,6' || weekday === '6,0') return <span>{t('schedules:cron.weekend', { time: timeStr })}</span>;
  if (weekday === '*' && /^\d+$/.test(dom)) return <span>{t('schedules:cron.monthly', { day: dom, time: timeStr })}</span>;

  if (/^\d$/.test(weekday)) {
    const dayName = dayNames[parseInt(weekday)];
    if (dayName) return <span>{dayName} {timeStr}</span>;
  }

  if (weekday === '*') return <span>{t('schedules:cron.daily', { time: timeStr })}</span>;

  return <span>{expression}</span>;
};

export const Schedules: React.FC = () => {
  const { t } = useTranslation(['schedules', 'common']);
  const isI18nReady = useI18nNamespace(['schedules', 'common']);
  const queryClient = useQueryClient();
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [showModal, setShowModal] = useState(false);
  const [editingSchedule, setEditingSchedule] = useState<Schedule | null>(null);
  const [executingId, setExecutingId] = useState<string | null>(null);

  const { data: schedules, isLoading } = useQuery({
    queryKey: scheduleKeys.all(),
    queryFn: getSchedules,
  });

  const toggleMutation = useMutation({
    mutationFn: (id: string) => toggleSchedule(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: scheduleKeys.root });
      toast.success(t('common:toast.scheduleToggled'));
    },
    onError: (error) => {
      toast.error(t('common:toast.operationFailed', { message: (error as Error).message }));
    },
  });

  const deleteMutation = useMutation({
    mutationFn: deleteSchedule,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: scheduleKeys.root });
      toast.success(t('common:toast.scheduleDeleted'));
    },
    onError: (error) => {
      toast.error(t('common:toast.operationFailed', { message: (error as Error).message }));
    },
  });

  const executeMutation = useMutation({
    mutationFn: startManualRecord,
    onSuccess: (task) => {
      upsertTaskCache(queryClient, task);
      queryClient.invalidateQueries({ queryKey: taskKeys.root });
      toast.success(t('schedules:created'));
      setExecutingId(null);
    },
    onError: (error) => {
      toast.error(t('schedules:executeFailed', { message: (error as Error).message }));
      setExecutingId(null);
    },
  });

  const handleEdit = (schedule: Schedule) => {
    setEditingSchedule(schedule);
    setShowModal(true);
  };

  const handleCloseModal = () => {
    setShowModal(false);
    setEditingSchedule(null);
  };

  const handleExecute = (schedule: Schedule) => {
    setExecutingId(schedule.id);
    executeMutation.mutate({
      channel_id: schedule.channel_id,
      schedule_id: schedule.id,
      duration_seconds: schedule.duration_seconds,
      output_name: schedule.name,
      output_dir: schedule.output_dir || undefined,
      output_template: schedule.output_template || undefined,
      video_quality: schedule.video_quality,
      audio_quality: schedule.audio_quality,
      max_speed: schedule.max_speed || undefined,
      thread_count: schedule.thread_count,
      transcode_mode: schedule.transcode_mode,
      transcode_preset: schedule.transcode_preset,
    });
  };

  const formatDuration = (seconds: number) => {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    if (hours > 0) {
      return `${hours}h${minutes > 0 ? ` ${minutes}m` : ''}`;
    }
    return `${minutes}m`;
  };

  if (!isI18nReady) {
    return <div className="page-loading">{t('common:loading')}</div>;
  }

  const shouldRenderScheduleModal = showModal || editingSchedule !== null;

  return (
    <div className="schedules-page">
      <div className="page-header">
        <div className="page-title">
          <h1>{t('schedules:title')}</h1>
          <p className="page-subtitle">
            {t('schedules:enabledCount', { count: schedules?.filter((schedule) => schedule.enabled).length || 0 })}
          </p>
        </div>
        <button className="btn btn-primary" onClick={() => setShowModal(true)}>
          <Plus size={16} />
          {t('schedules:add')}
        </button>
      </div>

      {isLoading ? (
        <div className="loading-list">
          {[1, 2, 3, 4].map((item) => (
            <div key={item} className="schedule-skeleton card animate-shimmer" />
          ))}
        </div>
      ) : schedules && schedules.length > 0 ? (
        <div className="schedule-list">
          {schedules.map((schedule, index) => (
            <div
              key={schedule.id}
              className={`schedule-card card stagger-item ${!schedule.enabled ? 'disabled' : ''}`}
              style={{ animationDelay: `${index * 0.05}s` }}
            >
              <div className="schedule-main">
                <div className="schedule-icon">
                  <CalendarClock size={20} />
                </div>

                <div className="schedule-info">
                  <div className="schedule-header">
                    <h3 className="schedule-name">{schedule.name}</h3>
                    <span className={`status-dot ${schedule.enabled ? 'active' : ''}`} />
                  </div>

                  <div className="schedule-meta">
                    <div className="meta-item">
                      <Clock size={14} />
                      <CronDescription expression={schedule.cron_expression} />
                    </div>
                    <div className="meta-divider" />
                    <div className="meta-item">
                      {formatDuration(schedule.duration_seconds)}
                    </div>
                  </div>
                </div>

                <div className="schedule-actions">
                  {/* 立即执行：常驻卡片外层，无需展开即可触发 */}
                  <button
                    className="btn btn-ghost btn-sm"
                    onClick={() => handleExecute(schedule)}
                    disabled={executingId === schedule.id}
                    title={t('schedules:actions.execute')}
                  >
                    {executingId === schedule.id ? (
                      <Loader2 size={16} className="animate-spin" />
                    ) : (
                      <Play size={16} />
                    )}
                  </button>

                  <button
                    className={`toggle-switch ${schedule.enabled ? 'active' : ''}`}
                    onClick={() => toggleMutation.mutate(schedule.id)}
                  >
                    {schedule.enabled ? <ToggleRight size={24} /> : <ToggleLeft size={24} />}
                  </button>

                  <button
                    className="expand-btn"
                    onClick={() => setExpandedId(expandedId === schedule.id ? null : schedule.id)}
                  >
                    <ChevronDown
                      size={18}
                      className={expandedId === schedule.id ? 'rotated' : ''}
                    />
                  </button>
                </div>
              </div>

              {expandedId === schedule.id && (
                <div className="schedule-details animate-fade-in">
                  <div className="detail-row">
                    <span className="detail-label">{t('schedules:details.cron')}</span>
                    <code className="detail-value">{schedule.cron_expression}</code>
                  </div>
                  <div className="detail-row">
                    <span className="detail-label">{t('schedules:details.outputTemplate')}</span>
                    <code className="detail-value">{schedule.output_template}</code>
                  </div>
                  <div className="detail-row">
                    <span className="detail-label">{t('schedules:details.outputDir')}</span>
                    <code className="detail-value">{schedule.output_dir || t('schedules:details.systemDefault')}</code>
                  </div>
                  <div className="detail-row">
                    <span className="detail-label">{t('schedules:details.priority')}</span>
                    <span className="detail-value">{schedule.priority}</span>
                  </div>
                  <div className="detail-row">
                    <span className="detail-label">{t('schedules:details.retry')}</span>
                    <span className="detail-value">{schedule.max_retry}</span>
                  </div>

                  <div className="detail-actions">
                    <button
                      className="btn btn-ghost btn-sm"
                      onClick={() => handleEdit(schedule)}
                    >
                      <Pencil size={14} />
                      {t('schedules:actions.edit')}
                    </button>
                    <button
                      className="btn btn-ghost btn-sm danger"
                      onClick={() => deleteMutation.mutate(schedule.id)}
                    >
                      <Trash2 size={14} />
                      {t('schedules:actions.delete')}
                    </button>
                  </div>
                </div>
              )}
            </div>
          ))}
        </div>
      ) : (
        <div className="empty-state card">
          <div className="empty-icon">
            <CalendarClock size={48} strokeWidth={1} />
          </div>
          <div className="empty-title">{t('schedules:empty.title')}</div>
          <div className="empty-desc">{t('schedules:empty.desc')}</div>
          <button className="btn btn-primary" onClick={() => setShowModal(true)}>
            <Plus size={16} />
            {t('schedules:create')}
          </button>
        </div>
      )}

      <Suspense fallback={null}>
        {shouldRenderScheduleModal && (
          <ScheduleModal
            isOpen={showModal}
            onClose={handleCloseModal}
            schedule={editingSchedule}
          />
        )}
      </Suspense>
    </div>
  );
};

export default Schedules;
