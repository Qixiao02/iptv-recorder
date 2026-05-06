import React, { Suspense, lazy, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getSchedules, deleteSchedule, toggleSchedule } from '@/api/schedules';
import { startManualRecord } from '@/api/tasks';
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
  const parts = expression.trim().split(/\s+/);
  if (parts.length < 5) return <span>{expression}</span>;

  const [min, hour, dom, , weekday] = parts;
  const weekDayNames = ['周日', '周一', '周二', '周三', '周四', '周五', '周六'];

  const pad = (s: string) => s.padStart(2, '0');
  const timeStr = `${pad(hour)}:${pad(min)}`;

  // 每分钟
  if (min === '*' && hour === '*') return <span>每分钟</span>;

  // 每 N 分钟
  const everyMinMatch = min.match(/^\*\/(\d+)$/);
  if (everyMinMatch && hour === '*') return <span>每 {everyMinMatch[1]} 分钟</span>;

  // 每小时整点
  if (min === '0' && hour === '*') return <span>每小时</span>;

  // 每 N 小时
  const everyHourMatch = hour.match(/^\*\/(\d+)$/);
  if (everyHourMatch && min === '0') return <span>每 {everyHourMatch[1]} 小时</span>;

  // 以下均要求 min/hour 为固定数字
  if (!/^\d+$/.test(min) || !/^\d+$/.test(hour)) return <span>{expression}</span>;

  // 工作日
  if (weekday === '1-5') return <span>工作日 {timeStr}</span>;

  // 周末
  if (weekday === '0,6' || weekday === '6,0') return <span>周末 {timeStr}</span>;

  // 每月某日
  if (weekday === '*' && /^\d+$/.test(dom)) return <span>每月 {dom} 日 {timeStr}</span>;

  // 每周某天（单个数字）
  if (/^\d$/.test(weekday)) {
    const dayName = weekDayNames[parseInt(weekday)];
    if (dayName) return <span>{dayName} {timeStr}</span>;
  }

  // 每天
  if (weekday === '*') return <span>每天 {timeStr}</span>;

  // fallback
  return <span>{expression}</span>;
};

export const Schedules: React.FC = () => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [showModal, setShowModal] = useState(false);
  const [editingSchedule, setEditingSchedule] = useState<Schedule | null>(null);
  const [executingId, setExecutingId] = useState<string | null>(null);

  const { data: schedules, isLoading } = useQuery({
    queryKey: ['schedules'],
    queryFn: getSchedules,
  });

  const toggleMutation = useMutation({
    mutationFn: (id: string) => toggleSchedule(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['schedules'] });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: deleteSchedule,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['schedules'] });
    },
  });

  const executeMutation = useMutation({
    mutationFn: startManualRecord,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tasks'] });
      setExecutingId(null);
    },
    onError: () => {
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
      return `${hours}小时${minutes > 0 ? ` ${minutes}分钟` : ''}`;
    }
    return `${minutes}分钟`;
  };

  const shouldRenderScheduleModal = showModal || editingSchedule !== null;

  return (
    <div className="schedules-page">
      {/* Page Header */}
      <div className="page-header">
        <div className="page-title">
          <h1>{t('menu.schedules')}</h1>
          <p className="page-subtitle">
            {schedules?.filter((s) => s.enabled).length || 0} 个计划启用中
          </p>
        </div>
        <button className="btn btn-primary" onClick={() => setShowModal(true)}>
          <Plus size={16} />
          {t('common.add')}
        </button>
      </div>

      {/* Schedule List */}
      {isLoading ? (
        <div className="loading-list">
          {[1, 2, 3, 4].map((i) => (
            <div key={i} className="schedule-skeleton card animate-shimmer" />
          ))}
        </div>
      ) : schedules && schedules.length > 0 ? (
        <div className="schedule-list">
          {schedules.map((schedule, idx) => (
            <div
              key={schedule.id}
              className={`schedule-card card stagger-item ${!schedule.enabled ? 'disabled' : ''}`}
              style={{ animationDelay: `${idx * 0.05}s` }}
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

              {/* Expanded Details */}
              {expandedId === schedule.id && (
                <div className="schedule-details animate-fade-in">
                  <div className="detail-row">
                    <span className="detail-label">Cron 表达式</span>
                    <code className="detail-value">{schedule.cron_expression}</code>
                  </div>
                  <div className="detail-row">
                    <span className="detail-label">输出模板</span>
                    <code className="detail-value">{schedule.output_template}</code>
                  </div>
                  <div className="detail-row">
                    <span className="detail-label">输出目录</span>
                    <code className="detail-value">{schedule.output_dir || '使用系统默认'}</code>
                  </div>
                  <div className="detail-row">
                    <span className="detail-label">优先级</span>
                    <span className="detail-value">{schedule.priority}</span>
                  </div>
                  <div className="detail-row">
                    <span className="detail-label">重试次数</span>
                    <span className="detail-value">{schedule.max_retry}</span>
                  </div>

                  <div className="detail-actions">
                    <button
                      className="btn btn-ghost btn-sm"
                      onClick={() => handleExecute(schedule)}
                      disabled={executingId === schedule.id}
                    >
                      {executingId === schedule.id ? (
                        <Loader2 size={14} className="animate-spin" />
                      ) : (
                        <Play size={14} />
                      )}
                      立即执行
                    </button>
                    <button
                      className="btn btn-ghost btn-sm"
                      onClick={() => handleEdit(schedule)}
                    >
                      <Pencil size={14} />
                      编辑
                    </button>
                    <button
                      className="btn btn-ghost btn-sm danger"
                      onClick={() => deleteMutation.mutate(schedule.id)}
                    >
                      <Trash2 size={14} />
                      删除
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
          <div className="empty-title">暂无录制计划</div>
          <div className="empty-desc">创建定时录制计划，自动录制您喜爱的节目</div>
          <button className="btn btn-primary" onClick={() => setShowModal(true)}>
            <Plus size={16} />
            创建计划
          </button>
        </div>
      )}

      {/* Schedule Modal */}
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
