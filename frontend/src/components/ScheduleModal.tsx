import React, { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { createSchedule, updateSchedule } from '@/api/schedules';
import { getAllChannels } from '@/api/channels';
import { useI18nNamespace } from '@/i18n/useI18nNamespace';
import { X, Loader2, Settings, HelpCircle } from 'lucide-react';
import type { Schedule, CreateScheduleRequest } from '@/types';
import './Modal.css';

interface ScheduleModalProps {
  isOpen: boolean;
  onClose: () => void;
  schedule?: Schedule | null;
}

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
  output_dir: '',
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

  const { data: channels } = useQuery({
    queryKey: ['channels', 'all'],
    queryFn: getAllChannels,
  });

  useEffect(() => {
    queueMicrotask(() => {
      if (schedule) {
        setForm({
          name: schedule.name,
          channel_id: schedule.channel_id,
          cron_expression: schedule.cron_expression,
          duration_seconds: schedule.duration_seconds,
          output_template: schedule.output_template,
          output_dir: schedule.output_dir || '',
          priority: schedule.priority,
          video_quality: schedule.video_quality || 'best',
          audio_quality: schedule.audio_quality || 'best',
          max_speed: schedule.max_speed || '',
          thread_count: schedule.thread_count || 20,
          transcode_mode: schedule.transcode_mode || 'off',
          transcode_preset: schedule.transcode_preset || 'medium',
        });
      } else {
        setForm({
          ...defaultForm,
          channel_id: channels?.[0]?.id || '',
        });
      }
    });
  }, [schedule, channels]);

  const createMutation = useMutation({
    mutationFn: createSchedule,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['schedules'] });
      queryClient.invalidateQueries({ queryKey: ['upcoming'] });
      handleClose();
    },
    onError: (error) => {
      console.error('创建计划失败:', error);
      alert(t('components:scheduleModal.createFailed', { message: (error as Error).message }));
    },
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: CreateScheduleRequest }) =>
      updateSchedule(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['schedules'] });
      queryClient.invalidateQueries({ queryKey: ['upcoming'] });
      handleClose();
    },
    onError: (error) => {
      console.error('更新计划失败:', error);
      alert(t('components:scheduleModal.updateFailed', { message: (error as Error).message }));
    },
  });

  const isLoading = createMutation.isPending || updateMutation.isPending;

  const handleSubmit = () => {
    if (!form.name.trim() || !form.channel_id) return;

    if (isEdit && schedule) {
      updateMutation.mutate({ id: schedule.id, data: form });
    } else {
      createMutation.mutate(form);
    }
  };

  const handleClose = () => {
    onClose();
  };

  if (!isOpen) return null;

  return (
    <div className="modal-overlay" onClick={handleClose}>
      <div className="modal-content" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2>{isEdit ? t('components:scheduleModal.editTitle') : t('components:scheduleModal.createTitle')}</h2>
          <button className="modal-close" onClick={handleClose}>
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
            <select
              className="input channel-select"
              value={form.channel_id}
              onChange={(e) => setForm({ ...form, channel_id: e.target.value })}
            >
              <option value="">{t('components:scheduleModal.channelPlaceholder')}</option>
              {channels?.map((ch) => (
                <option key={ch.id} value={ch.id}>
                  {ch.name} ({ch.group_name})
                </option>
              ))}
            </select>
          </div>

          <div className="form-group">
            <div className="label-with-help">
              <label>{t('components:scheduleModal.cron')}</label>
              <button
                type="button"
                className="help-btn"
                onClick={() => setShowCronHelp(!showCronHelp)}
                title={t('components:scheduleModal.cronHelpTitle')}
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
          </div>

          <div className="form-row">
            <div className="form-group">
              <label>{t('components:scheduleModal.durationSeconds')}</label>
              <input
                type="number"
                className="input"
                value={form.duration_seconds}
                onChange={(e) => setForm({ ...form, duration_seconds: parseInt(e.target.value) || 0 })}
                min={60}
              />
              {form.duration_seconds > 0 && (
                <span className="duration-hint">
                  = {t('components:scheduleModal.durationHint', {
                    hours: Math.floor(form.duration_seconds / 3600),
                    minutes: Math.floor((form.duration_seconds % 3600) / 60),
                  })}
                </span>
              )}
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
            <span className="form-hint">
              {t('components:scheduleModal.outputTemplateHint')}
            </span>
          </div>

          <div className="form-group">
            <label>{t('components:scheduleModal.outputDir')}</label>
            <input
              type="text"
              className="input"
              placeholder={t('components:scheduleModal.outputDirPlaceholder')}
              value={form.output_dir || ''}
              onChange={(e) => setForm({ ...form, output_dir: e.target.value })}
            />
            <span className="form-hint">
              {t('components:scheduleModal.outputDirHint')}
            </span>
          </div>

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
