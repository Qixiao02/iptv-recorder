import React, { useState, useEffect } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { createSchedule, updateSchedule } from '@/api/schedules';
import { getAllChannels } from '@/api/channels';
import { X, Loader2, Settings, HelpCircle } from 'lucide-react';
import type { Schedule, CreateScheduleRequest } from '@/types';
import './Modal.css';

interface ScheduleModalProps {
  isOpen: boolean;
  onClose: () => void;
  schedule?: Schedule | null;
}

const cronPresets = [
  { label: '每天 19:00', value: '0 19 * * *' },
  { label: '每天 20:00', value: '0 20 * * *' },
  { label: '工作日 19:00', value: '0 19 * * 1-5' },
  { label: '周末 20:00', value: '0 20 * * 6,0' },
  { label: '每小时', value: '0 * * * *' },
  { label: '每 30 分钟', value: '*/30 * * * *' },
];

const videoQualityOptions = [
  { label: '最佳质量', value: 'best' },
  { label: '1080p', value: '1080p' },
  { label: '720p', value: '720p' },
  { label: '480p', value: '480p' },
  { label: '360p', value: '360p' },
];

const transcodeModeOptions = [
  { value: 'off', label: '不转码 - 直接保存原始流' },
  { value: 'realtime', label: '实时转码 - 录制时转码（省时省空间）' },
  { value: 'post', label: '后期转码 - 录制后转码（最稳定）' },
];

const transcodePresetOptions = [
  { value: 'high', label: '高质量 (CRF 18)' },
  { value: 'medium', label: '中等质量 (CRF 23, 推荐)' },
  { value: 'low', label: '低质量 (CRF 28, 文件最小)' },
  { value: 'custom', label: '自定义参数' },
];

interface ScheduleFormData extends CreateScheduleRequest {
  video_quality?: string;
  audio_quality?: string;
  max_speed?: string;
  thread_count?: number;
  transcode_mode?: string;
  transcode_preset?: string;
  output_dir?: string;
}

export const ScheduleModal: React.FC<ScheduleModalProps> = ({ isOpen, onClose, schedule }) => {
  const queryClient = useQueryClient();
  const isEdit = !!schedule;
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [showCronHelp, setShowCronHelp] = useState(false);
  const [showTranscodeHelp, setShowTranscodeHelp] = useState(false);

  const [form, setForm] = useState<ScheduleFormData>({
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
  });

  const { data: channels } = useQuery({
    queryKey: ['channels', 'all'],
    queryFn: getAllChannels,
  });

  useEffect(() => {
    if (schedule) {
      setForm({
        name: schedule.name,
        channel_id: schedule.channel_id,
        cron_expression: schedule.cron_expression,
        duration_seconds: schedule.duration_seconds,
        output_template: schedule.output_template,
        output_dir: schedule.output_dir || '',
        priority: schedule.priority,
        video_quality: (schedule as any).video_quality || 'best',
        audio_quality: (schedule as any).audio_quality || 'best',
        max_speed: (schedule as any).max_speed || '',
        thread_count: (schedule as any).thread_count || 20,
        transcode_mode: (schedule as any).transcode_mode || 'off',
        transcode_preset: (schedule as any).transcode_preset || 'medium',
      });
    } else {
      setForm({
        name: '',
        channel_id: channels?.[0]?.id || '',
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
      });
    }
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
      alert('创建计划失败: ' + (error as Error).message);
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
      alert('更新计划失败: ' + (error as Error).message);
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
          <h2>{isEdit ? '编辑计划' : '新建计划'}</h2>
          <button className="modal-close" onClick={handleClose}>
            <X size={20} />
          </button>
        </div>

        <div className="modal-body">
          <div className="form-group">
            <label>计划名称 *</label>
            <input
              type="text"
              className="input"
              placeholder="例如：新闻联播"
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
            />
          </div>

          <div className="form-group">
            <label>选择频道 *</label>
            <select
              className="input channel-select"
              value={form.channel_id}
              onChange={(e) => setForm({ ...form, channel_id: e.target.value })}
            >
              <option value="">请选择频道</option>
              {channels?.map((ch) => (
                <option key={ch.id} value={ch.id}>
                  {ch.name} ({ch.group_name})
                </option>
              ))}
            </select>
          </div>

          <div className="form-group">
            <div className="label-with-help">
              <label>Cron 表达式 *</label>
              <button
                type="button"
                className="help-btn"
                onClick={() => setShowCronHelp(!showCronHelp)}
                title="查看 Cron 表达式帮助"
              >
                <HelpCircle size={16} />
              </button>
            </div>
            {showCronHelp && (
              <div className="cron-help">
                <div className="cron-help-header">Cron 表达式格式：分 时 日 月 周</div>
                <div className="cron-help-table">
                  <div className="cron-help-row">
                    <span className="field">分</span>
                    <span>0-59</span>
                    <span>例: */15 = 每15分钟</span>
                  </div>
                  <div className="cron-help-row">
                    <span className="field">时</span>
                    <span>0-23</span>
                    <span>例: 19 = 晚上7点</span>
                  </div>
                  <div className="cron-help-row">
                    <span className="field">日</span>
                    <span>1-31</span>
                    <span>例: * = 每天</span>
                  </div>
                  <div className="cron-help-row">
                    <span className="field">月</span>
                    <span>1-12</span>
                    <span>例: * = 每月</span>
                  </div>
                  <div className="cron-help-row">
                    <span className="field">周</span>
                    <span>0-6</span>
                    <span>0=周日, 1-5=工作日, 6=周六</span>
                  </div>
                </div>
                <div className="cron-help-examples">
                  <div><code>0 19 * * *</code> = 每天 19:00</div>
                  <div><code>30 8 * * 1-5</code> = 工作日 8:30</div>
                  <div><code>0 */2 * * *</code> = 每2小时</div>
                  <div><code>0 20 * * 6,0</code> = 周末 20:00</div>
                </div>
              </div>
            )}
            <input
              type="text"
              className="input"
              placeholder="分 时 日 月 周"
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
              <label>录制时长（秒）</label>
              <input
                type="number"
                className="input"
                value={form.duration_seconds}
                onChange={(e) => setForm({ ...form, duration_seconds: parseInt(e.target.value) || 0 })}
                min={60}
              />
              {form.duration_seconds > 0 && (
                <span className="duration-hint">
                  = {Math.floor(form.duration_seconds / 3600)}小时{Math.floor((form.duration_seconds % 3600) / 60)}分钟
                </span>
              )}
            </div>
            <div className="form-group">
              <label>优先级 (1-10)</label>
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
            <label>输出文件名模板</label>
            <input
              type="text"
              className="input"
              placeholder="{channel_name}_{date}_{time}"
              value={form.output_template}
              onChange={(e) => setForm({ ...form, output_template: e.target.value })}
            />
            <span className="form-hint">
              可用变量: {'{channel_name}'}, {'{date}'}, {'{time}'}, {'{datetime}'}（无需写后缀，自动识别）
            </span>
          </div>

          <div className="form-group">
            <label>自定义输出目录</label>
            <input
              type="text"
              className="input"
              placeholder="留空则使用系统默认路径"
              value={form.output_dir || ''}
              onChange={(e) => setForm({ ...form, output_dir: e.target.value })}
            />
            <span className="form-hint">
              例如: D:\Recordings\新闻 (留空使用系统设置的默认路径)
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
              录制参数设置
              <span className={`toggle-icon ${showAdvanced ? 'open' : ''}`}>▼</span>
            </button>

            {showAdvanced && (
              <div className="advanced-content">
                {/* 转码设置 */}
                <div className="settings-section" style={{ marginBottom: 0, paddingBottom: 0, borderBottom: 'none' }}>
                  <div className="label-with-help">
                    <div className="settings-section-title" style={{ borderLeft: 'none', paddingLeft: 0, margin: 0 }}>转码设置</div>
                    <button
                      type="button"
                      className="help-btn"
                      onClick={() => setShowTranscodeHelp(!showTranscodeHelp)}
                      title="查看转码模式说明"
                    >
                      <HelpCircle size={16} />
                    </button>
                  </div>

                  {showTranscodeHelp && (
                    <div className="transcode-help">
                      <div className="transcode-help-item">
                        <div className="mode-name off">
                          🔴 不转码
                          <span className="tag">速度快 | 体积大 | 原始画质</span>
                        </div>
                        <div className="mode-desc">
                          <div>直接保存原始流文件（TS格式），不做任何处理</div>
                          <ul>
                            <li className="pro">速度最快，无CPU占用</li>
                            <li className="pro">100%原始画质，无损</li>
                            <li className="con">文件体积最大（1小时约3-4GB）</li>
                            <li className="con">TS格式，部分播放器兼容性差</li>
                          </ul>
                        </div>
                      </div>
                      <div className="transcode-help-item">
                        <div className="mode-name realtime">
                          🟡 实时转码
                          <span className="tag">省时间 | 省空间 | 需CPU</span>
                        </div>
                        <div className="mode-desc">
                          <div>录制的同时进行转码，录完直接得到MP4文件</div>
                          <ul>
                            <li className="pro">一步到位，录制完成即可播放</li>
                            <li className="pro">节省磁盘空间（1小时约500MB-1GB）</li>
                            <li className="con">CPU占用高，需要性能好的电脑</li>
                            <li className="con">CPU不足时可能丢帧卡顿</li>
                          </ul>
                        </div>
                      </div>
                      <div className="transcode-help-item">
                        <div className="mode-name post">
                          🟢 后期转码
                          <span className="tag">最稳定 | 可控画质 | 双倍时间</span>
                        </div>
                        <div className="mode-desc">
                          <div>先录制原始文件，录制完成后再自动转码压缩</div>
                          <ul>
                            <li className="pro">录制过程最稳定，不会丢帧</li>
                            <li className="pro">转码质量可控，可调整参数</li>
                            <li className="con">需要双倍时间（先录后转）</li>
                            <li className="con">临时占用更多磁盘（原始+转码）</li>
                          </ul>
                        </div>
                      </div>
                    </div>
                  )}

                  <div className="form-group">
                    <label>转码模式</label>
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
                      <label>转码质量</label>
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
                  <div className="settings-section-title">下载设置</div>
                  <div className="form-row">
                    <div className="form-group">
                      <label>视频质量</label>
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
                      <label>音频质量</label>
                      <select
                        className="input"
                        value={form.audio_quality}
                        onChange={(e) => setForm({ ...form, audio_quality: e.target.value })}
                      >
                        <option value="best">最佳质量</option>
                      </select>
                    </div>
                  </div>

                  <div className="form-row">
                    <div className="form-group">
                      <label>下载限速</label>
                      <input
                        type="text"
                        className="input"
                        placeholder="例如: 10M, 500K (留空不限速)"
                        value={form.max_speed}
                        onChange={(e) => setForm({ ...form, max_speed: e.target.value })}
                      />
                    </div>
                    <div className="form-group">
                      <label>下载线程数</label>
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
            取消
          </button>
          <button
            className="btn btn-primary"
            onClick={handleSubmit}
            disabled={isLoading || !form.name.trim() || !form.channel_id}
          >
            {isLoading ? (
              <>
                <Loader2 size={16} className="animate-spin" />
                保存中...
              </>
            ) : (
              isEdit ? '保存' : '创建'
            )}
          </button>
        </div>
      </div>
    </div>
  );
};

export default ScheduleModal;
