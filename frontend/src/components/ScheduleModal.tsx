import React, { useState, useEffect, useMemo, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { createSchedule, updateSchedule } from '@/api/schedules';
import { getAllChannels } from '@/api/channels';
import { toast } from '@/stores/toastStore';
import { useI18nNamespace } from '@/i18n/useI18nNamespace';
import { channelKeys, scheduleKeys, upcomingKeys } from '@/lib/queryKeys';
import { useModalA11y } from '@/lib/useModalA11y';
import { buildCron, parseCron, WEEKDAY_ORDER } from '@/lib/cronBuilder';
import { X, Loader2, Settings, HelpCircle, Search, ChevronDown, Check } from 'lucide-react';
import type { Schedule, CreateScheduleRequest, Channel } from '@/types';
import './Modal.css';

interface ScheduleModalProps {
  isOpen: boolean;
  onClose: () => void;
  schedule?: Schedule | null;
}

// 来源标签文案：公网源 / 私有源（内网）。纯函数，放在组件外避免每次渲染重建。
const sourceLabel = (ch: Channel, t: (key: string, opts?: Record<string, unknown>) => string) =>
  ch?.source_visibility === 'private_server_only'
    ? t('components:scheduleModal.sourcePrivate', { defaultValue: '私有源' })
    : t('components:scheduleModal.sourcePublic', { defaultValue: '公网源' });

// 来源对应的 badge 样式类
const sourceBadgeClass = (ch: Channel) =>
  ch?.source_visibility === 'private_server_only' ? 'source-badge private' : 'source-badge public';

// URL 简短显示：去掉协议前缀，超出长度截断，帮助用户辨认内网/外网地址。
const shortUrl = (url: string) => {
  const stripped = url.replace(/^https?:\/\//i, '');
  return stripped.length > 42 ? `${stripped.slice(0, 42)}…` : stripped;
};

interface ScheduleFormData extends CreateScheduleRequest {
  video_quality?: string;
  audio_quality?: string;
  max_speed?: string;
  thread_count?: number;
  transcode_mode?: string;
  transcode_preset?: string;
  output_dir?: string;
}

const defaultForm: ScheduleFormData = {
  name: '',
  channel_id: '',
  cron_expression: '0 19 * * *',
  duration_seconds: 3600,
  output_template: '{channel_name}_{datetime}',
  priority: 5,
  video_quality: 'best',
  audio_quality: 'best',
  max_speed: '',
  thread_count: 20,
  transcode_mode: 'off',
  transcode_preset: 'medium',
};

export const ScheduleModal: React.FC<ScheduleModalProps> = ({ isOpen, onClose, schedule }) => {
  const { t } = useTranslation(['components', 'common']);
  useI18nNamespace(['components', 'common']);
  const queryClient = useQueryClient();
  const isEdit = !!schedule;
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [showCronHelp, setShowCronHelp] = useState(false);
  const [showTranscodeHelp, setShowTranscodeHelp] = useState(false);
  // 时间设置模式:simple(时间+星期+时长,小白友好) / advanced(原始 cron)
  const [scheduleMode, setScheduleMode] = useState<'simple' | 'advanced'>('simple');
  // 简单模式状态:开始时间 + 选中的星期
  const [startTime, setStartTime] = useState('19:00');
  const [weekdays, setWeekdays] = useState<number[]>([]);
  // 简单模式时长:小时 + 分钟(提交时换算成秒)
  const [durationHours, setDurationHours] = useState(1);
  const [durationMinutes, setDurationMinutes] = useState(0);
  const cronPresets = [
    { label: t('components:scheduleModal.cronPresets.daily19'), value: '0 19 * * *' },
    { label: t('components:scheduleModal.cronPresets.daily20'), value: '0 20 * * *' },
    { label: t('components:scheduleModal.cronPresets.workday19'), value: '0 19 * * 1-5' },
    { label: t('components:scheduleModal.cronPresets.weekend20'), value: '0 20 * * 6,0' },
    { label: t('components:scheduleModal.cronPresets.hourly'), value: '0 * * * *' },
    { label: t('components:scheduleModal.cronPresets.every30Minutes'), value: '*/30 * * * *' },
  ];
  const videoQualityOptions = [
    { label: t('components:scheduleModal.bestQuality'), value: 'best' },
    { label: '1080p', value: '1080p' },
    { label: '720p', value: '720p' },
    { label: '480p', value: '480p' },
    { label: '360p', value: '360p' },
  ];
  const transcodeModeOptions = [
    { value: 'off', label: t('components:scheduleModal.transcodeModes.off') },
    { value: 'realtime', label: t('components:scheduleModal.transcodeModes.realtime') },
    { value: 'post', label: t('components:scheduleModal.transcodeModes.post') },
  ];
  const transcodePresetOptions = [
    { value: 'high', label: t('components:scheduleModal.transcodePresets.high') },
    { value: 'medium', label: t('components:scheduleModal.transcodePresets.medium') },
    { value: 'low', label: t('components:scheduleModal.transcodePresets.low') },
    { value: 'custom', label: t('components:scheduleModal.transcodePresets.custom') },
  ];

  const [form, setForm] = useState<ScheduleFormData>(defaultForm);

  // 可搜索频道选择器状态
  const [channelSearchOpen, setChannelSearchOpen] = useState(false);
  const [channelKeyword, setChannelKeyword] = useState('');
  const channelSearchRef = useRef<HTMLDivElement>(null);
  const overlayRef = useRef<HTMLDivElement>(null);
  useModalA11y(overlayRef, isOpen, onClose);

  const { data: channels } = useQuery({
    queryKey: channelKeys.all(),
    queryFn: getAllChannels,
  });

  // 按关键词模糊过滤频道（名称 + 分组 + URL，URL 帮助区分同名不同源的频道）
  const filteredChannels = useMemo(() => {
    const all = channels ?? [];
    const kw = channelKeyword.trim().toLowerCase();
    if (!kw) return all;
    return all.filter((ch) =>
      ch.name.toLowerCase().includes(kw)
      || (ch.group_name || '').toLowerCase().includes(kw)
      || (ch.url || '').toLowerCase().includes(kw)
    );
  }, [channels, channelKeyword]);

  // 选中的频道名（用于显示，附带来源标签以便区分同名不同源）
  const selectedChannelName = useMemo(() => {
    const ch = channels?.find((c) => c.id === form.channel_id);
    if (!ch) return '';
    const src = sourceLabel(ch, t);
    return `${ch.name}${ch.group_name ? ` (${ch.group_name})` : ''} · ${src}`;
  }, [channels, form.channel_id, t]);

  // 点击外部关闭搜索下拉
  useEffect(() => {
    if (!channelSearchOpen) return;
    const handler = (e: MouseEvent) => {
      if (channelSearchRef.current && !channelSearchRef.current.contains(e.target as Node)) {
        setChannelSearchOpen(false);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [channelSearchOpen]);

  useEffect(() => {
    queueMicrotask(() => {
      if (schedule) {
        setForm({
          name: schedule.name,
          channel_id: schedule.channel_id,
          cron_expression: schedule.cron_expression,
          duration_seconds: schedule.duration_seconds,
          output_template: schedule.output_template,
          priority: schedule.priority,
          video_quality: schedule.video_quality || 'best',
          audio_quality: schedule.audio_quality || 'best',
          max_speed: schedule.max_speed || '',
          thread_count: schedule.thread_count || 20,
          transcode_mode: schedule.transcode_mode || 'off',
          transcode_preset: schedule.transcode_preset || 'medium',
        });
        // 智能解析 cron 回填简单 UI:能解析就用简单模式,否则进高级模式
        const parsed = parseCron(schedule.cron_expression);
        if (parsed) {
          setStartTime(parsed.time);
          setWeekdays(parsed.weekdays);
          setScheduleMode('simple');
        } else {
          setScheduleMode('advanced');
        }
        // 时长(秒)拆分成时+分回填
        const secs = schedule.duration_seconds || 3600;
        setDurationHours(Math.floor(secs / 3600));
        setDurationMinutes(Math.floor((secs % 3600) / 60));
      } else {
        setForm({
          ...defaultForm,
          channel_id: channels?.[0]?.id || '',
        });
        // 新建默认值
        setStartTime('19:00');
        setWeekdays([]);
        setScheduleMode('simple');
        setDurationHours(1);
        setDurationMinutes(0);
      }
    });
  }, [schedule, channels]);

  const createMutation = useMutation({
    mutationFn: createSchedule,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: scheduleKeys.root });
      queryClient.invalidateQueries({ queryKey: upcomingKeys.upcoming() });
      toast.success(t('common:toast.scheduleCreated'));
      handleClose();
    },
    onError: (error) => {
      toast.error(t('components:scheduleModal.createFailed', { message: (error as Error).message }));
    },
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: CreateScheduleRequest }) =>
      updateSchedule(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: scheduleKeys.root });
      queryClient.invalidateQueries({ queryKey: upcomingKeys.upcoming() });
      toast.success(t('common:toast.scheduleUpdated'));
      handleClose();
    },
    onError: (error) => {
      toast.error(t('components:scheduleModal.updateFailed', { message: (error as Error).message }));
    },
  });

  const isLoading = createMutation.isPending || updateMutation.isPending;

  const handleSubmit = () => {
    if (!form.name.trim() || !form.channel_id) return;

    // 自定义输出目录已统一收归到系统设置，提交时清空，由后端使用全局录制保存路径
    const { output_dir: _omitted, ...payload } = form;
    void _omitted;

    // 简单模式:从 startTime + weekdays 组装 cron,从时+分换算 duration_seconds
    if (scheduleMode === 'simple') {
      payload.cron_expression = buildCron({ time: startTime, weekdays });
      payload.duration_seconds = durationHours * 3600 + durationMinutes * 60;
    }
    // 高级模式:payload 里的 cron_expression/duration_seconds 已由用户直接编辑

    if (isEdit && schedule) {
      updateMutation.mutate({ id: schedule.id, data: payload });
    } else {
      createMutation.mutate(payload);
    }
  };

  const handleClose = () => {
    onClose();
  };

  if (!isOpen) return null;

  return (
    <div
      className="modal-overlay"
      onClick={handleClose}
      ref={overlayRef}
      role="dialog"
      aria-modal="true"
      aria-labelledby="schedule-modal-title"
      tabIndex={-1}
    >
      <div className="modal-content" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2 id="schedule-modal-title">{isEdit ? t('components:scheduleModal.editTitle') : t('components:scheduleModal.createTitle')}</h2>
          <button className="modal-close" onClick={handleClose} aria-label={t('common:close', { defaultValue: '关闭' })}>
            <X size={20} />
          </button>
        </div>

        <div className="modal-body">
          <div className="form-group">
            <label>{t('components:scheduleModal.name')}</label>
            <input
              type="text"
              className="input"
              placeholder={t('components:scheduleModal.namePlaceholder')}
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
            />
          </div>

          <div className="form-group">
            <label>{t('components:scheduleModal.channel')}</label>
            <div className="channel-search-select" ref={channelSearchRef}>
              {/* 触发器：显示已选频道名，点击展开搜索 */}
              <button
                type="button"
                className="input channel-search-trigger"
                onClick={() => {
                  setChannelSearchOpen((v) => !v);
                  setChannelKeyword('');
                }}
              >
                <span className={selectedChannelName ? '' : 'placeholder'}>
                  {selectedChannelName || t('components:scheduleModal.channelPlaceholder')}
                </span>
                <ChevronDown size={16} className={channelSearchOpen ? 'rotated' : ''} />
              </button>

              {channelSearchOpen && (
                <div className="channel-search-dropdown">
                  <div className="channel-search-input-wrap">
                    <Search size={14} />
                    <input
                      type="text"
                      className="channel-search-input"
                      placeholder={t('components:scheduleModal.channelSearchPlaceholder', { defaultValue: '输入频道名搜索…' })}
                      value={channelKeyword}
                      onChange={(e) => setChannelKeyword(e.target.value)}
                      autoFocus
                    />
                  </div>
                  <div className="channel-search-list">
                    {filteredChannels.length > 0 ? (
                      filteredChannels.map((ch) => (
                        <button
                          type="button"
                          key={ch.id}
                          className={`channel-search-item ${ch.id === form.channel_id ? 'selected' : ''}`}
                          onClick={() => {
                            setForm({ ...form, channel_id: ch.id });
                            setChannelSearchOpen(false);
                            setChannelKeyword('');
                          }}
                        >
                          <div className="channel-item-main">
                            <div className="channel-item-topline">
                              <span className="channel-name">{ch.name}</span>
                              <span className={sourceBadgeClass(ch)}>{sourceLabel(ch, t)}</span>
                            </div>
                            <div className="channel-item-subline">
                              {ch.group_name && <span className="channel-group">{ch.group_name}</span>}
                              <code className="channel-url">{shortUrl(ch.url)}</code>
                            </div>
                          </div>
                          {ch.id === form.channel_id && <Check size={14} className="channel-check" />}
                        </button>
                      ))
                    ) : (
                      <div className="channel-search-empty">
                        {t('components:scheduleModal.channelSearchEmpty', { defaultValue: '未找到匹配的频道' })}
                      </div>
                    )}
                  </div>
                </div>
              )}
            </div>
          </div>

          {/* ===== 时间设置:简单模式(时间+星期) / 高级模式(原始 cron) ===== */}
          <div className="form-group">
            {/* 模式切换 tab */}
            <div className="schedule-mode-tabs">
              <button
                type="button"
                className={`schedule-mode-tab ${scheduleMode === 'simple' ? 'active' : ''}`}
                onClick={() => setScheduleMode('simple')}
              >
                {t('components:scheduleModal.modeSimple')}
              </button>
              <button
                type="button"
                className={`schedule-mode-tab ${scheduleMode === 'advanced' ? 'active' : ''}`}
                onClick={() => setScheduleMode('advanced')}
              >
                {t('components:scheduleModal.modeAdvanced')}
              </button>
            </div>

            {/* ----- 简单模式:时间 + 星期 ----- */}
            {scheduleMode === 'simple' ? (
              <>
                <div className="form-row">
                  <div className="form-group">
                    <label>{t('components:scheduleModal.startTime')}</label>
                    <input
                      type="time"
                      className="input time-input"
                      value={startTime}
                      onChange={(e) => setStartTime(e.target.value)}
                    />
                  </div>
                  <div className="form-group">
                    <label>{t('components:scheduleModal.weekdays')}</label>
                    <div className="weekday-buttons">
                      {WEEKDAY_ORDER.map((day) => {
                        const key = (['sun', 'mon', 'tue', 'wed', 'thu', 'fri', 'sat'] as const)[day];
                        const selected = weekdays.includes(day);
                        return (
                          <button
                            key={day}
                            type="button"
                            className={`preset-btn weekday-btn ${selected ? 'active' : ''}`}
                            onClick={() =>
                              setWeekdays((prev) =>
                                prev.includes(day)
                                  ? prev.filter((d) => d !== day)
                                  : [...prev, day],
                              )
                            }
                            aria-pressed={selected}
                          >
                            {t(`components:scheduleModal.weekdayShort.${key}`)}
                          </button>
                        );
                      })}
                    </div>
                    {weekdays.length === 0 && (
                      <span className="duration-hint">
                        {t('components:scheduleModal.everyDay')}
                      </span>
                    )}
                  </div>
                </div>
                {/* 生成的 cron 预览(让用户对实际规则有感知) */}
                <div className="cron-preview">
                  <span className="cron-preview-label">
                    {t('components:scheduleModal.cronPreview')}:
                  </span>
                  <code className="cron-preview-value">
                    {buildCron({ time: startTime, weekdays })}
                  </code>
                </div>
              </>
            ) : (
              /* ----- 高级模式:原始 cron 输入 + 预设 + 帮助(折叠) ----- */
              <>
                <div className="label-with-help">
                  <label>{t('components:scheduleModal.cron')}</label>
                  <button
                    type="button"
                    className="help-btn"
                    onClick={() => setShowCronHelp(!showCronHelp)}
                    title={t('components:scheduleModal.cronHelpTitle')}
                    aria-label={t('components:scheduleModal.cronHelpTitle')}
                    aria-expanded={showCronHelp}
                  >
                    <HelpCircle size={16} />
                  </button>
                </div>
                {showCronHelp && (
                  <div className="cron-help">
                    <div className="cron-help-header">{t('components:scheduleModal.cronFormat')}</div>
                    <div className="cron-help-table">
                      <div className="cron-help-row">
                        <span className="field">{t('components:scheduleModal.cronFields.minute')}</span>
                        <span>0-59</span>
                        <span>{t('components:scheduleModal.cronExamples.minute')}</span>
                      </div>
                      <div className="cron-help-row">
                        <span className="field">{t('components:scheduleModal.cronFields.hour')}</span>
                        <span>0-23</span>
                        <span>{t('components:scheduleModal.cronExamples.hour')}</span>
                      </div>
                      <div className="cron-help-row">
                        <span className="field">{t('components:scheduleModal.cronFields.day')}</span>
                        <span>1-31</span>
                        <span>{t('components:scheduleModal.cronExamples.day')}</span>
                      </div>
                      <div className="cron-help-row">
                        <span className="field">{t('components:scheduleModal.cronFields.month')}</span>
                        <span>1-12</span>
                        <span>{t('components:scheduleModal.cronExamples.month')}</span>
                      </div>
                      <div className="cron-help-row">
                        <span className="field">{t('components:scheduleModal.cronFields.week')}</span>
                        <span>0-6</span>
                        <span>{t('components:scheduleModal.cronExamples.week')}</span>
                      </div>
                    </div>
                    <div className="cron-help-examples">
                      <div><code>0 19 * * *</code> = {t('components:scheduleModal.cronExamples.daily')}</div>
                      <div><code>30 8 * * 1-5</code> = {t('components:scheduleModal.cronExamples.workday')}</div>
                      <div><code>0 */2 * * *</code> = {t('components:scheduleModal.cronExamples.everyTwoHours')}</div>
                      <div><code>0 20 * * 6,0</code> = {t('components:scheduleModal.cronExamples.weekend')}</div>
                    </div>
                  </div>
                )}
                <input
                  type="text"
                  className="input"
                  placeholder={t('components:scheduleModal.cronPlaceholder')}
                  value={form.cron_expression}
                  onChange={(e) => setForm({ ...form, cron_expression: e.target.value })}
                />
                <div className="cron-presets">
                  {cronPresets.map((preset) => (
                    <button
                      key={preset.value}
                      type="button"
                      className={`preset-btn ${form.cron_expression === preset.value ? 'active' : ''}`}
                      onClick={() => setForm({ ...form, cron_expression: preset.value })}
                    >
                      {preset.label}
                    </button>
                  ))}
                </div>
              </>
            )}
          </div>

          {/* ===== 录制时长:简单模式用时+分,高级模式用秒 ===== */}
          <div className="form-row">
            <div className="form-group">
              <label>{t('components:scheduleModal.durationSecondsLegacy')}</label>
              {scheduleMode === 'simple' ? (
                <div className="duration-inputs">
                  <div className="duration-field">
                    <input
                      type="number"
                      className="input"
                      value={durationHours}
                      onChange={(e) => setDurationHours(Math.max(0, parseInt(e.target.value) || 0))}
                      min={0}
                    />
                    <span className="duration-unit">
                      {t('components:scheduleModal.durationHours')}
                    </span>
                  </div>
                  <div className="duration-field">
                    <input
                      type="number"
                      className="input"
                      value={durationMinutes}
                      onChange={(e) => setDurationMinutes(Math.min(59, Math.max(0, parseInt(e.target.value) || 0)))}
                      min={0}
                      max={59}
                    />
                    <span className="duration-unit">
                      {t('components:scheduleModal.durationMinutes')}
                    </span>
                  </div>
                </div>
              ) : (
                <input
                  type="number"
                  className="input"
                  value={form.duration_seconds}
                  onChange={(e) => setForm({ ...form, duration_seconds: parseInt(e.target.value) || 0 })}
                  min={60}
                />
              )}
              {(() => {
                const secs =
                  scheduleMode === 'simple'
                    ? durationHours * 3600 + durationMinutes * 60
                    : form.duration_seconds;
                return secs > 0 ? (
                  <span className="duration-hint">
                    = {t('components:scheduleModal.durationHint', {
                      hours: Math.floor(secs / 3600),
                      minutes: Math.floor((secs % 3600) / 60),
                    })}
                  </span>
                ) : null;
              })()}
            </div>
            <div className="form-group">
              <label>{t('components:scheduleModal.priority')}</label>
              <input
                type="number"
                className="input"
                value={form.priority}
                onChange={(e) => setForm({ ...form, priority: parseInt(e.target.value) || 5 })}
                min={1}
                max={10}
              />
            </div>
          </div>

          <div className="form-group">
            <label>{t('components:scheduleModal.outputTemplate')}</label>
            <input
              type="text"
              className="input"
              placeholder="{channel_name}_{date}_{time}"
              value={form.output_template}
              onChange={(e) => setForm({ ...form, output_template: e.target.value })}
            />
            <div className="template-hint">
              <span className="form-hint">{t('components:scheduleModal.outputTemplateHint')}</span>
              <ul className="template-variables">
                {(t('components:scheduleModal.outputTemplateVariables', { returnObjects: true }) as Array<{ token: string; desc: string }>)
                  .map((v) => (
                    <li key={v.token}>
                      <code className="template-var">{v.token}</code>
                      <span className="template-var-desc">{v.desc}</span>
                    </li>
                  ))}
              </ul>
            </div>
          </div>

          {/* 自定义输出目录已移除：统一使用系统设置中的「录制保存路径」，避免每个计划重复配置 */}

          {/* 高级设置 */}
          <div className="advanced-section">
            <button
              type="button"
              className="advanced-toggle"
              onClick={() => setShowAdvanced(!showAdvanced)}
            >
              <Settings size={16} />
              {t('components:scheduleModal.advanced')}
              <span className={`toggle-icon ${showAdvanced ? 'open' : ''}`}>▼</span>
            </button>

            {showAdvanced && (
              <div className="advanced-content">
                {/* 转码设置 */}
                <div className="settings-section" style={{ marginBottom: 0, paddingBottom: 0, borderBottom: 'none' }}>
                  <div className="label-with-help">
                    <div className="settings-section-title" style={{ borderLeft: 'none', paddingLeft: 0, margin: 0 }}>
                      {t('components:scheduleModal.transcodeSettings')}
                    </div>
                    <button
                      type="button"
                      className="help-btn"
                      onClick={() => setShowTranscodeHelp(!showTranscodeHelp)}
                      title={t('components:scheduleModal.transcodeHelpTitle')}
                      aria-label={t('components:scheduleModal.transcodeHelpTitle')}
                      aria-expanded={showTranscodeHelp}
                    >
                      <HelpCircle size={16} />
                    </button>
                  </div>

                  {showTranscodeHelp && (
                    <div className="transcode-help">
                      <div className="transcode-help-item">
                        <div className="mode-name off">
                          {t('components:scheduleModal.transcodeHelp.offName')}
                          <span className="tag">{t('components:scheduleModal.transcodeHelp.offTag')}</span>
                        </div>
                        <div className="mode-desc">
                          <div>{t('components:scheduleModal.transcodeHelp.offDesc')}</div>
                          <ul>
                            <li className="pro">{t('components:scheduleModal.transcodeHelp.offPro1')}</li>
                            <li className="pro">{t('components:scheduleModal.transcodeHelp.offPro2')}</li>
                            <li className="con">{t('components:scheduleModal.transcodeHelp.offCon1')}</li>
                            <li className="con">{t('components:scheduleModal.transcodeHelp.offCon2')}</li>
                          </ul>
                        </div>
                      </div>
                      <div className="transcode-help-item">
                        <div className="mode-name realtime">
                          {t('components:scheduleModal.transcodeHelp.realtimeName')}
                          <span className="tag">{t('components:scheduleModal.transcodeHelp.realtimeTag')}</span>
                        </div>
                        <div className="mode-desc">
                          <div>{t('components:scheduleModal.transcodeHelp.realtimeDesc')}</div>
                          <ul>
                            <li className="pro">{t('components:scheduleModal.transcodeHelp.realtimePro1')}</li>
                            <li className="pro">{t('components:scheduleModal.transcodeHelp.realtimePro2')}</li>
                            <li className="con">{t('components:scheduleModal.transcodeHelp.realtimeCon1')}</li>
                            <li className="con">{t('components:scheduleModal.transcodeHelp.realtimeCon2')}</li>
                          </ul>
                        </div>
                      </div>
                      <div className="transcode-help-item">
                        <div className="mode-name post">
                          {t('components:scheduleModal.transcodeHelp.postName')}
                          <span className="tag">{t('components:scheduleModal.transcodeHelp.postTag')}</span>
                        </div>
                        <div className="mode-desc">
                          <div>{t('components:scheduleModal.transcodeHelp.postDesc')}</div>
                          <ul>
                            <li className="pro">{t('components:scheduleModal.transcodeHelp.postPro1')}</li>
                            <li className="pro">{t('components:scheduleModal.transcodeHelp.postPro2')}</li>
                            <li className="con">{t('components:scheduleModal.transcodeHelp.postCon1')}</li>
                            <li className="con">{t('components:scheduleModal.transcodeHelp.postCon2')}</li>
                          </ul>
                        </div>
                      </div>
                    </div>
                  )}

                  <div className="form-group">
                    <label>{t('components:scheduleModal.transcodeMode')}</label>
                    <select
                      className="input"
                      value={form.transcode_mode}
                      onChange={(e) => setForm({ ...form, transcode_mode: e.target.value })}
                    >
                      {transcodeModeOptions.map((opt) => (
                        <option key={opt.value} value={opt.value}>
                          {opt.label}
                        </option>
                      ))}
                    </select>
                  </div>
                  {form.transcode_mode !== 'off' && (
                    <div className="form-group">
                      <label>{t('components:scheduleModal.transcodeQuality')}</label>
                      <select
                        className="input"
                        value={form.transcode_preset}
                        onChange={(e) => setForm({ ...form, transcode_preset: e.target.value })}
                      >
                        {transcodePresetOptions.map((opt) => (
                          <option key={opt.value} value={opt.value}>
                            {opt.label}
                          </option>
                        ))}
                      </select>
                    </div>
                  )}
                </div>

                {/* 下载设置 */}
                <div className="settings-section">
                  <div className="settings-section-title">{t('components:scheduleModal.downloadSettings')}</div>
                  <div className="form-row">
                    <div className="form-group">
                      <label>{t('components:scheduleModal.videoQuality')}</label>
                      <select
                        className="input"
                        value={form.video_quality}
                        onChange={(e) => setForm({ ...form, video_quality: e.target.value })}
                      >
                        {videoQualityOptions.map((opt) => (
                          <option key={opt.value} value={opt.value}>
                            {opt.label}
                          </option>
                        ))}
                      </select>
                    </div>
                    <div className="form-group">
                      <label>{t('components:scheduleModal.audioQuality')}</label>
                      <select
                        className="input"
                        value={form.audio_quality}
                        onChange={(e) => setForm({ ...form, audio_quality: e.target.value })}
                      >
                        <option value="best">{t('components:scheduleModal.bestQuality')}</option>
                      </select>
                    </div>
                  </div>

                  <div className="form-row">
                    <div className="form-group">
                      <label>{t('components:scheduleModal.speedLimit')}</label>
                      <input
                        type="text"
                        className="input"
                        placeholder={t('components:scheduleModal.speedLimitPlaceholder')}
                        value={form.max_speed}
                        onChange={(e) => setForm({ ...form, max_speed: e.target.value })}
                      />
                    </div>
                    <div className="form-group">
                      <label>{t('components:scheduleModal.threadCount')}</label>
                      <input
                        type="number"
                        className="input"
                        value={form.thread_count}
                        onChange={(e) => setForm({ ...form, thread_count: parseInt(e.target.value) || 20 })}
                        min={1}
                        max={100}
                      />
                    </div>
                  </div>
                </div>
              </div>
            )}
          </div>
        </div>

        <div className="modal-footer">
          <button className="btn btn-ghost" onClick={handleClose}>
            {t('common:cancel')}
          </button>
          <button
            className="btn btn-primary"
            onClick={handleSubmit}
            disabled={isLoading || !form.name.trim() || !form.channel_id}
          >
            {isLoading ? (
              <>
                <Loader2 size={16} className="animate-spin" />
                {t('components:channelModal.saving')}
              </>
            ) : (
              isEdit ? t('components:channelModal.save') : t('components:channelModal.create')
            )}
          </button>
        </div>
      </div>
    </div>
  );
};

export default ScheduleModal;
