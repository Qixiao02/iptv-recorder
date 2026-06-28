import React, { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  getConfig,
  updateConfig,
  getSystemHealth,
  getAuditLogs,
  runCleanup,
  reloadScheduler,
  listServerDirectories,
  type ServerDirectoryList,
} from '@/api/system';
import { changePassword } from '@/api/auth';
import { useAuthStore } from '@/stores/authStore';
import { useSettingStore } from '@/stores/settingStore';
import { useUIStore } from '@/stores/uiStore';
import { toast } from '@/stores/toastStore';
import { useI18nNamespace } from '@/i18n/useI18nNamespace';
import { formatShortDateTime } from '@/i18n/format';
import type { AppLanguage } from '@/i18n/types';
import type { SystemConfig } from '@/types';
import { buildConfigUpdateRequest } from './configPayload';
import Markdown from '@/components/Markdown';
// 以纯文本导入 README(构建前由 predev/prebuild 脚本从项目根同步到 src/about-readme.md)
import readmeContent from '@/about-readme.md?raw';
import {
  Settings,
  Database,
  Clapperboard,
  Bell,
  Info,
  ChevronLeft,
  ChevronRight,
  Save,
  RotateCcw,
  FolderOpen,
  Loader2,
  CheckCircle,
  User,
  Lock,
  Eye,
  EyeOff,
  ShieldAlert,
  Activity,
  TimerReset,
  ScrollText,
  RefreshCw,
  Server,
  FileText,
  Calendar,
  X,
} from 'lucide-react';
import '@/components/Modal.css';
import './Settings.css';

type SettingsSection = 'general' | 'storage' | 'recording' | 'notification' | 'operations' | 'account' | 'about';

const defaultConfig: SystemConfig = {
  server: { host: '127.0.0.1', port: 3000 },
  storage: { recordings_path: './data/recordings', auto_cleanup_days: 30, min_free_space_gb: 10 },
  recording: { default_duration_minutes: 60, n_m3u8dl_re_path: 'N_m3u8DL-RE', max_retry: 3, thread_count: 4 },
  notification: { on_complete: true, on_failure: true, disk_warning: true },
};

export const SettingsPage: React.FC = () => {
  const { t, i18n } = useTranslation(['settings', 'common']);
  const isI18nReady = useI18nNamespace(['settings', 'common']);
  const queryClient = useQueryClient();
  const { language, setLanguage } = useSettingStore();
  const [activeSection, setActiveSection] = useState<SettingsSection>('general');
  const [localConfig, setLocalConfig] = useState<SystemConfig>(defaultConfig);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const [saveError, setSaveError] = useState('');
  const [directoryPickerOpen, setDirectoryPickerOpen] = useState(false);
  const [directoryList, setDirectoryList] = useState<ServerDirectoryList | null>(null);
  const [directoryLoading, setDirectoryLoading] = useState(false);
  const [directoryError, setDirectoryError] = useState('');
  // 手动输入路径(用于网络路径 UNC / 挂载点直接粘贴导航)
  const [manualPath, setManualPath] = useState('');
  const { user } = useAuthStore();
  const alerts = useUIStore((state) => state.alerts);
  const addAlert = useUIStore((state) => state.addAlert);
  const isAdmin = user?.role === 'admin';

  const sections: { key: SettingsSection; icon: React.ReactNode; label: string }[] = [
    { key: 'general', icon: <Settings size={18} />, label: t('settings:sections.general') },
    { key: 'storage', icon: <Database size={18} />, label: t('settings:sections.storage') },
    { key: 'recording', icon: <Clapperboard size={18} />, label: t('settings:sections.recording') },
    { key: 'notification', icon: <Bell size={18} />, label: t('settings:sections.notification') },
    ...(isAdmin ? [{ key: 'operations' as const, icon: <ShieldAlert size={18} />, label: t('settings:sections.operations') }] : []),
    { key: 'account', icon: <User size={18} />, label: t('settings:sections.account') },
    { key: 'about', icon: <Info size={18} />, label: t('settings:sections.about') },
  ];

  const [passwordForm, setPasswordForm] = useState({
    old_password: '',
    new_password: '',
    confirm_password: '',
  });
  const [showOldPassword, setShowOldPassword] = useState(false);
  const [showNewPassword, setShowNewPassword] = useState(false);
  const [passwordSuccess, setPasswordSuccess] = useState(false);
  const [passwordError, setPasswordError] = useState('');

  const { data: config, isLoading, refetch: refetchConfig } = useQuery({
    queryKey: ['config'],
    queryFn: getConfig,
  });

  const {
    data: systemHealth,
    isLoading: isHealthLoading,
    refetch: refetchSystemHealth,
  } = useQuery({
    queryKey: ['system', 'health'],
    queryFn: getSystemHealth,
    enabled: isAdmin,
    refetchInterval: isAdmin ? 30000 : false,
  });

  const [auditPage, setAuditPage] = useState(1);
  const [auditPageSize, setAuditPageSize] = useState(20);

  const {
    data: auditData,
    isLoading: isAuditLoading,
    refetch: refetchAuditLogs,
  } = useQuery({
    queryKey: ['audit', 'logs', auditPage, auditPageSize],
    queryFn: () => getAuditLogs({ page: auditPage, page_size: auditPageSize }),
    enabled: isAdmin,
    placeholderData: (prev) => prev,
  });
  const auditLogs = auditData?.items ?? [];
  const auditTotalPages = auditData?.total_pages ?? 1;
  const auditTotal = auditData?.total ?? 0;

  useEffect(() => {
    if (config) {
      queueMicrotask(() => setLocalConfig(config));
    }
  }, [config]);

  useEffect(() => {
    if (!isAdmin && activeSection === 'operations') {
      queueMicrotask(() => setActiveSection('general'));
    }
  }, [activeSection, isAdmin]);

  const saveMutation = useMutation({
    mutationFn: updateConfig,
    onSuccess: (data) => {
      setLocalConfig(data);
      queryClient.setQueryData(['config'], data);
      setSaveSuccess(true);
      setSaveError('');
      setTimeout(() => setSaveSuccess(false), 2000);
      toast.success(t('common:toast.configSaved'));
    },
    onError: (error) => {
      setSaveError(error instanceof Error ? error.message : t('settings:saveFailed'));
      toast.error(t('common:toast.operationFailed', { message: error instanceof Error ? error.message : '' }));
    },
  });

  const passwordMutation = useMutation({
    mutationFn: () => changePassword({
      old_password: passwordForm.old_password,
      new_password: passwordForm.new_password,
    }),
    onSuccess: () => {
      setPasswordSuccess(true);
      setPasswordError('');
      setPasswordForm({ old_password: '', new_password: '', confirm_password: '' });
      setTimeout(() => setPasswordSuccess(false), 3000);
      toast.success(t('common:toast.passwordChanged'));
    },
    onError: (error) => {
      setPasswordError(error instanceof Error ? error.message : t('settings:account.failed'));
      toast.error(t('common:toast.operationFailed', { message: error instanceof Error ? error.message : '' }));
    },
  });

  const cleanupMutation = useMutation({
    mutationFn: runCleanup,
    onSuccess: (result) => {
      queryClient.invalidateQueries({ queryKey: ['tasks'] });
      refetchSystemHealth();
      refetchAuditLogs();
      addAlert({
        level: 'info',
        message: t('settings:ops.cleanupSuccess'),
        details: result.message,
      });
      toast.success(t('common:toast.cleanupDone', { count: result.deleted ?? 0 }));
    },
    onError: (error) => {
      addAlert({
        level: 'error',
        message: t('settings:ops.cleanupFailed'),
        details: error instanceof Error ? error.message : t('common:unknownError'),
      });
      toast.error(t('common:toast.operationFailed', { message: error instanceof Error ? error.message : '' }));
    },
  });

  const reloadMutation = useMutation({
    mutationFn: reloadScheduler,
    onSuccess: (result) => {
      refetchSystemHealth();
      refetchAuditLogs();
      addAlert({
        level: 'info',
        message: t('settings:ops.schedulerReloaded'),
        details: result.message,
      });
      toast.success(t('common:toast.schedulerReloaded'));
    },
    onError: (error) => {
      addAlert({
        level: 'error',
        message: t('settings:ops.schedulerReloadFailed'),
        details: error instanceof Error ? error.message : t('common:unknownError'),
      });
      toast.error(t('common:toast.operationFailed', { message: error instanceof Error ? error.message : '' }));
    },
  });

  const handlePasswordSubmit = () => {
    setPasswordError('');

    if (!passwordForm.old_password || !passwordForm.new_password || !passwordForm.confirm_password) {
      setPasswordError(t('settings:account.required'));
      return;
    }

    if (passwordForm.new_password !== passwordForm.confirm_password) {
      setPasswordError(t('settings:account.mismatch'));
      return;
    }

    if (passwordForm.new_password.length < 6) {
      setPasswordError(t('settings:account.tooShort'));
      return;
    }

    passwordMutation.mutate();
  };

  const hasChanges = JSON.stringify(localConfig) !== JSON.stringify(config);

  const updateLocalConfig = (path: string, value: string | number | boolean) => {
    setLocalConfig((prev) => {
      const newConfig = { ...prev };
      const keys = path.split('.');
      let obj: Record<string, unknown> = newConfig;

      for (let i = 0; i < keys.length - 1; i++) {
        const clone = { ...(obj[keys[i]] as Record<string, unknown>) };
        obj[keys[i]] = clone;
        obj = clone;
      }

      obj[keys[keys.length - 1]] = value;
      return newConfig;
    });
  };

  const handleSave = () => {
    saveMutation.mutate(buildConfigUpdateRequest(localConfig));
  };

  const handleReset = () => {
    if (config) {
      setLocalConfig(config);
      setSaveError('');
    }
  };

  const loadServerDirectory = async (path?: string) => {
    setDirectoryLoading(true);
    setDirectoryError('');
    try {
      const result = await listServerDirectories(path);
      setDirectoryList(result);
    } catch (error) {
      setDirectoryError(error instanceof Error ? error.message : t('settings:storage.browserLoadFailed', { defaultValue: 'Failed to load server directories' }));
    } finally {
      setDirectoryLoading(false);
    }
  };

  const openDirectoryPicker = () => {
    setDirectoryPickerOpen(true);
    setDirectoryList(null);
    void loadServerDirectory(localConfig.storage.recordings_path);
  };

  const closeDirectoryPicker = () => {
    setDirectoryPickerOpen(false);
    setDirectoryError('');
    setManualPath('');
  };

  // 手动输入路径后导航进去(用于网络路径 UNC / 挂载点直接粘贴)
  const navigateToManualPath = () => {
    const p = manualPath.trim();
    if (p) {
      void loadServerDirectory(p);
    }
  };

  const selectCurrentDirectory = () => {
    if (!directoryList?.current_path) {
      return;
    }
    updateLocalConfig('storage.recordings_path', directoryList.current_path);
    closeDirectoryPicker();
  };

  const handleRefreshOperations = () => {
    refetchConfig();
    refetchSystemHealth();
    refetchAuditLogs();
  };

  const latestAlerts = alerts.slice(0, 6);

  const healthStatus = (() => {
    if (!systemHealth) return { label: t('settings:ops.health.loading'), tone: 'neutral' };
    if (systemHealth.failed_tasks_24h > 0) return { label: t('settings:ops.health.attention'), tone: 'warning' };
    if (systemHealth.running_tasks > 0) return { label: t('settings:ops.health.running'), tone: 'success' };
    return { label: t('settings:ops.health.stable'), tone: 'success' };
  })();

  const formatDateTime = (value: string | null | undefined) => {
    if (!value) return '-';
    return formatShortDateTime(value, i18n.language as AppLanguage);
  };

  if (!isI18nReady) {
    return <div className="page-loading">{t('common:loading')}</div>;
  }

  const renderSection = () => {
    switch (activeSection) {
      case 'general':
        return (
          <div className="settings-section">
            <h2>{t('settings:sections.general')}</h2>
            <div className="setting-item">
              <div className="setting-info">
                <label>{t('settings:general.language')}</label>
                <span className="setting-desc">{t('settings:general.languageDesc')}</span>
              </div>
              <select
                className="input setting-control"
                value={language}
                onChange={(e) => setLanguage(e.target.value as AppLanguage)}
              >
                <option value="zh-CN">{t('settings:general.zh')}</option>
                <option value="en-US">{t('settings:general.en')}</option>
              </select>
            </div>
            <div className="setting-item">
              <div className="setting-info">
                <label>{t('settings:general.serverHost')}</label>
                <span className="setting-desc">{t('settings:general.serverHostDesc')}</span>
              </div>
              <input type="text" className="input setting-control" value={localConfig.server.host} readOnly />
            </div>
            <div className="setting-item">
              <div className="setting-info">
                <label>{t('settings:general.serverPort')}</label>
                <span className="setting-desc">{t('settings:general.serverPortDesc')}</span>
              </div>
              <input type="number" className="input setting-control" value={localConfig.server.port} readOnly />
            </div>
          </div>
        );

      case 'storage':
        return (
          <div className="settings-section">
            <h2>{t('settings:sections.storage')}</h2>
            <div className="setting-item">
              <div className="setting-info">
                <label>{t('settings:storage.path')}</label>
                <span className="setting-desc">{t('settings:storage.pathDesc')}</span>
              </div>
              <div className="setting-control-group">
                <div className="input-with-btn">
                  <input
                    type="text"
                    className="input"
                    value={localConfig.storage.recordings_path}
                    onChange={(e) => updateLocalConfig('storage.recordings_path', e.target.value)}
                  />
                  <button
                    className="btn btn-ghost"
                    type="button"
                    onClick={openDirectoryPicker}
                    title={t('settings:storage.browseServer', { defaultValue: 'Browse server directories' })}
                  >
                    <FolderOpen size={16} />
                  </button>
                </div>
                <span className="setting-hint">
                  {t('settings:storage.recordingsPathHint', { defaultValue: '支持本地路径或网络路径(如 \\\\nas\\media\\recordings、/mnt/nas/recordings)。Docker 部署需先将网络盘挂载到容器。' })}
                </span>
              </div>
            </div>
            <div className="setting-item">
              <div className="setting-info">
                <label>{t('settings:storage.autoCleanup')}</label>
                <span className="setting-desc">{t('settings:storage.autoCleanupDesc')}</span>
              </div>
              <div className="input-with-suffix">
                <input
                  type="number"
                  className="input"
                  value={localConfig.storage.auto_cleanup_days}
                  onChange={(e) => updateLocalConfig('storage.auto_cleanup_days', parseInt(e.target.value) || 0)}
                  min={0}
                  max={365}
                />
                <span className="input-suffix">{t('settings:storage.days')}</span>
              </div>
            </div>
            <div className="setting-item">
              <div className="setting-info">
                <label>{t('settings:storage.minFreeSpace')}</label>
                <span className="setting-desc">{t('settings:storage.minFreeSpaceDesc')}</span>
              </div>
              <div className="input-with-suffix">
                <input
                  type="number"
                  className="input"
                  value={localConfig.storage.min_free_space_gb}
                  onChange={(e) => updateLocalConfig('storage.min_free_space_gb', parseInt(e.target.value) || 1)}
                  min={1}
                />
                <span className="input-suffix">GB</span>
              </div>
            </div>
          </div>
        );

      case 'recording':
        return (
          <div className="settings-section">
            <h2>{t('settings:sections.recording')}</h2>
            <div className="setting-item">
              <div className="setting-info">
                <label>{t('settings:recording.defaultDuration')}</label>
                <span className="setting-desc">{t('settings:recording.defaultDurationDesc')}</span>
              </div>
              <div className="input-with-suffix">
                <input
                  type="number"
                  className="input"
                  value={localConfig.recording.default_duration_minutes}
                  onChange={(e) => updateLocalConfig('recording.default_duration_minutes', parseInt(e.target.value) || 60)}
                  min={1}
                  max={1440}
                />
                <span className="input-suffix">{t('settings:recording.minutes')}</span>
              </div>
            </div>
            <div className="setting-item">
              <div className="setting-info">
                <label>{t('settings:recording.maxRetry')}</label>
                <span className="setting-desc">{t('settings:recording.maxRetryDesc')}</span>
              </div>
              <input
                type="number"
                className="input setting-control"
                value={localConfig.recording.max_retry}
                onChange={(e) => updateLocalConfig('recording.max_retry', parseInt(e.target.value) || 0)}
                min={0}
                max={10}
              />
            </div>
            <div className="setting-item">
              <div className="setting-info">
                <label>{t('settings:recording.threadCount')}</label>
                <span className="setting-desc">{t('settings:recording.threadCountDesc')}</span>
              </div>
              <input
                type="number"
                className="input setting-control"
                value={localConfig.recording.thread_count}
                onChange={(e) => updateLocalConfig('recording.thread_count', parseInt(e.target.value) || 1)}
                min={1}
                max={32}
              />
            </div>
          </div>
        );

      case 'notification':
        return (
          <div className="settings-section">
            <h2>{t('settings:sections.notification')}</h2>
            {[
              ['notification.on_complete', localConfig.notification.on_complete, 'complete', 'completeDesc'],
              ['notification.on_failure', localConfig.notification.on_failure, 'failure', 'failureDesc'],
              ['notification.disk_warning', localConfig.notification.disk_warning, 'disk', 'diskDesc'],
            ].map(([path, checked, labelKey, descKey]) => (
              <div className="setting-item" key={path as string}>
                <div className="setting-info">
                  <label>{t(`settings:notification.${labelKey}`)}</label>
                  <span className="setting-desc">{t(`settings:notification.${descKey}`)}</span>
                </div>
                <label className="toggle">
                  <input
                    type="checkbox"
                    checked={checked as boolean}
                    onChange={(e) => updateLocalConfig(path as string, e.target.checked)}
                  />
                  <span className="toggle-slider" />
                </label>
              </div>
            ))}
          </div>
        );

      case 'account':
        return (
          <div className="settings-section">
            <h2>{t('settings:sections.account')}</h2>

            <div className="account-info-card">
              <div className="account-avatar">
                <User size={24} />
              </div>
              <div className="account-details">
                <div className="account-name">{user?.nickname || user?.username}</div>
                <div className="account-meta">
                  <span className="account-role">{user?.role === 'admin' ? t('common:admin') : t('common:user')}</span>
                  <span className="account-divider">|</span>
                  <span className="account-username">@{user?.username}</span>
                </div>
              </div>
            </div>

            <div className="password-section">
              <h3 className="section-subtitle">
                <Lock size={18} />
                {t('settings:account.changePassword')}
              </h3>

              {passwordSuccess && (
                <div className="password-success">
                  <CheckCircle size={18} />
                  {t('settings:account.passwordChanged')}
                </div>
              )}

              {passwordError && <div className="password-error">{passwordError}</div>}

              <div className="form-group">
                <label>{t('settings:account.currentPassword')}</label>
                <div className="password-input-wrapper">
                  <input
                    type={showOldPassword ? 'text' : 'password'}
                    className="input"
                    value={passwordForm.old_password}
                    onChange={(e) => setPasswordForm({ ...passwordForm, old_password: e.target.value })}
                    placeholder={t('settings:account.currentPasswordPlaceholder')}
                  />
                  <button type="button" className="password-toggle" onClick={() => setShowOldPassword(!showOldPassword)}>
                    {showOldPassword ? <EyeOff size={18} /> : <Eye size={18} />}
                  </button>
                </div>
              </div>

              <div className="form-group">
                <label>{t('settings:account.newPassword')}</label>
                <div className="password-input-wrapper">
                  <input
                    type={showNewPassword ? 'text' : 'password'}
                    className="input"
                    value={passwordForm.new_password}
                    onChange={(e) => setPasswordForm({ ...passwordForm, new_password: e.target.value })}
                    placeholder={t('settings:account.newPasswordPlaceholder')}
                  />
                  <button type="button" className="password-toggle" onClick={() => setShowNewPassword(!showNewPassword)}>
                    {showNewPassword ? <EyeOff size={18} /> : <Eye size={18} />}
                  </button>
                </div>
              </div>

              <div className="form-group">
                <label>{t('settings:account.confirmPassword')}</label>
                <input
                  type="password"
                  className="input"
                  value={passwordForm.confirm_password}
                  onChange={(e) => setPasswordForm({ ...passwordForm, confirm_password: e.target.value })}
                  placeholder={t('settings:account.confirmPasswordPlaceholder')}
                />
              </div>

              <button className="btn btn-primary" onClick={handlePasswordSubmit} disabled={passwordMutation.isPending}>
                {passwordMutation.isPending ? (
                  <>
                    <Loader2 size={16} className="animate-spin" />
                    {t('settings:account.changing')}
                  </>
                ) : (
                  <>
                    <Lock size={16} />
                    {t('settings:account.change')}
                  </>
                )}
              </button>
            </div>
          </div>
        );

      case 'operations':
        return (
          <div className="settings-section">
            <div className="section-header-row">
              <div>
                <h2>{t('settings:ops.title')}</h2>
                <p className="section-subtext">{t('settings:ops.subtitle')}</p>
              </div>
              <div className="section-header-actions">
                <button className="btn btn-ghost" onClick={handleRefreshOperations}>
                  <RefreshCw size={16} />
                  {t('settings:ops.refresh')}
                </button>
                <button className="btn btn-ghost" onClick={() => reloadMutation.mutate()} disabled={reloadMutation.isPending}>
                  {reloadMutation.isPending ? <Loader2 size={16} className="animate-spin" /> : <Activity size={16} />}
                  {t('settings:ops.reloadScheduler')}
                </button>
                <button className="btn btn-primary" onClick={() => cleanupMutation.mutate()} disabled={cleanupMutation.isPending}>
                  {cleanupMutation.isPending ? <Loader2 size={16} className="animate-spin" /> : <TimerReset size={16} />}
                  {t('settings:ops.cleanup')}
                </button>
              </div>
            </div>

            <div className="ops-summary-grid">
              <div className={`ops-summary-card ${healthStatus.tone}`}>
                <div className="ops-summary-title"><Server size={18} />{t('settings:ops.status')}</div>
                <div className="ops-summary-value">{healthStatus.label}</div>
                <div className="ops-summary-meta">
                  {isHealthLoading ? t('settings:ops.syncing') : t('settings:ops.lastAudit', { time: formatDateTime(systemHealth?.last_audit_at) })}
                </div>
              </div>
              <div className="ops-summary-card">
                <div className="ops-summary-title"><Clapperboard size={18} />{t('settings:ops.recordingTasks')}</div>
                <div className="ops-summary-value">{systemHealth?.running_tasks ?? '-'}</div>
                <div className="ops-summary-meta">{t('settings:ops.runningFailed', { value: systemHealth?.failed_tasks_24h ?? '-' })}</div>
              </div>
              <div className="ops-summary-card">
                <div className="ops-summary-title"><Calendar size={18} />{t('settings:ops.schedules')}</div>
                <div className="ops-summary-value">{systemHealth?.enabled_schedules ?? '-'}/{systemHealth?.schedules_total ?? '-'}</div>
                <div className="ops-summary-meta">{t('settings:ops.enabledTotal')}</div>
              </div>
              <div className="ops-summary-card">
                <div className="ops-summary-title"><User size={18} />{t('settings:ops.access')}</div>
                <div className="ops-summary-value">{systemHealth?.users_total ?? '-'}</div>
                <div className="ops-summary-meta">{t('settings:ops.usersChannels', { value: systemHealth?.channels_total ?? '-' })}</div>
              </div>
            </div>

            <div className="ops-grid">
              <section className="ops-panel">
                <div className="ops-panel-header">
                  <h3><ShieldAlert size={18} />{t('settings:ops.alerts')}</h3>
                  <span className="ops-panel-meta">{t('settings:ops.alertsMeta')}</span>
                </div>
                {latestAlerts.length > 0 ? (
                  <div className="ops-alert-list">
                    {latestAlerts.map((alert) => (
                      <div key={alert.id} className={`ops-alert-item ${alert.level}`}>
                        <div className="ops-alert-top">
                          <span className="ops-alert-level">{alert.level.toUpperCase()}</span>
                          <span className="ops-alert-time">{formatDateTime(alert.created_at)}</span>
                        </div>
                        <div className="ops-alert-message">{alert.message}</div>
                        {alert.details && <div className="ops-alert-details">{alert.details}</div>}
                      </div>
                    ))}
                  </div>
                ) : (
                  <div className="ops-empty">{t('settings:ops.alertsEmpty')}</div>
                )}
              </section>

              <section className="ops-panel">
                <div className="ops-panel-header">
                  <h3><FileText size={18} />{t('settings:ops.runbook')}</h3>
                  <span className="ops-panel-meta">{t('settings:ops.runbookMeta')}</span>
                </div>
                <div className="ops-runbook-list">
                  <div className="ops-runbook-item">
                    <div className="ops-runbook-title">{t('settings:ops.firstDeploy')}</div>
                    <div className="ops-runbook-desc">{t('settings:ops.firstDeployDesc')}</div>
                  </div>
                  <div className="ops-runbook-item">
                    <div className="ops-runbook-title">{t('settings:ops.preRelease')}</div>
                    <div className="ops-runbook-desc">{t('settings:ops.preReleaseDesc')}</div>
                  </div>
                  <div className="ops-runbook-item">
                    <div className="ops-runbook-title">{t('settings:ops.incident')}</div>
                    <div className="ops-runbook-desc">{t('settings:ops.incidentDesc')}</div>
                  </div>
                </div>
                <div className="ops-doc-hint">{t('settings:ops.docs')}</div>
              </section>
            </div>

            <section className="ops-panel">
              <div className="ops-panel-header">
                <h3><ScrollText size={18} />{t('settings:ops.audit')}</h3>
                <span className="ops-panel-meta">{t('settings:ops.auditMeta')}</span>
              </div>
              {isAuditLoading ? (
                <div className="ops-empty">{t('settings:ops.auditLoading')}</div>
              ) : auditLogs && auditLogs.length > 0 ? (
                <>
                  <div className="audit-table-wrap">
                    <table className="audit-table">
                      <thead>
                        <tr>
                          <th>{t('settings:ops.columns.time')}</th>
                          <th>{t('settings:ops.columns.user')}</th>
                          <th>{t('settings:ops.columns.role')}</th>
                          <th>{t('settings:ops.columns.action')}</th>
                          <th>{t('settings:ops.columns.resource')}</th>
                          <th>{t('settings:ops.columns.details')}</th>
                        </tr>
                      </thead>
                      <tbody>
                        {auditLogs.map((log) => (
                          <tr key={log.id}>
                            <td>{formatDateTime(log.created_at)}</td>
                            <td>{log.username || '-'}</td>
                            <td>{log.role || '-'}</td>
                            <td><code>{log.action}</code></td>
                            <td>{log.resource_type}{log.resource_id ? `:${log.resource_id.slice(0, 8)}` : ''}</td>
                            <td className="audit-details-cell" title={log.details || ''}>{log.details || '-'}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                  <div className="pagination pagination-audit">
                    <span className="pagination-total">
                      {t('settings:ops.totalRecords', { total: auditTotal, defaultValue: `共 ${auditTotal} 条` })}
                    </span>
                    <div className="pagination-controls">
                      <button
                        className="pagination-btn"
                        disabled={auditPage <= 1}
                        onClick={() => setAuditPage((p) => Math.max(1, p - 1))}
                      >
                        <ChevronLeft size={16} />
                      </button>
                      <span className="pagination-info">
                        {auditPage} / {auditTotalPages}
                      </span>
                      <button
                        className="pagination-btn"
                        disabled={auditPage >= auditTotalPages}
                        onClick={() => setAuditPage((p) => Math.min(auditTotalPages, p + 1))}
                      >
                        <ChevronRight size={16} />
                      </button>
                    </div>
                    <select
                      className="pagination-size"
                      value={auditPageSize}
                      onChange={(e) => {
                        setAuditPageSize(Number(e.target.value));
                        setAuditPage(1);
                      }}
                    >
                      {[20, 50, 100].map((size) => (
                        <option key={size} value={size}>
                          {t('settings:ops.pageSize', { count: size, defaultValue: `${size} 条/页` })}
                        </option>
                      ))}
                    </select>
                  </div>
                </>
              ) : (
                <div className="ops-empty">{t('settings:ops.auditEmpty')}</div>
              )}
            </section>
          </div>
        );

      case 'about':
        return (
          <div className="settings-section">
            <h2>{t('settings:sections.about')}</h2>
            <div className="about-card">
              <div className="about-logo">
                <svg viewBox="0 0 48 48" fill="none" xmlns="http://www.w3.org/2000/svg">
                  <rect width="48" height="48" rx="12" fill="url(#about-gradient)" />
                  <path d="M12 15H36M12 24H30M12 33H24" stroke="white" strokeWidth="3" strokeLinecap="round" />
                  <circle cx="36" cy="33" r="6" fill="white" fillOpacity="0.9" />
                  <defs>
                    <linearGradient id="about-gradient" x1="0" y1="0" x2="48" y2="48" gradientUnits="userSpaceOnUse">
                      <stop stopColor="#3B82F6" />
                      <stop offset="1" stopColor="#8B5CF6" />
                    </linearGradient>
                  </defs>
                </svg>
              </div>
              <div className="about-info">
                <h3>IPTV Recorder</h3>
                <p className="version">{t('settings:about.version')}</p>
                <p className="description">{t('settings:about.desc')}</p>
              </div>
            </div>

            {/* README 内容渲染：替代原先占位的官方网站/检查更新/GitHub/技术栈标签 */}
            <div className="about-readme">
              <Markdown content={readmeContent} />
            </div>
          </div>
        );
    }
  };

  return (
    <div className="settings-page">
      <div className="page-header">
        <div className="page-title">
          <h1>{t('settings:title')}</h1>
          <p className="page-subtitle">{t('settings:subtitle')}</p>
        </div>
      </div>

      <div className="settings-layout">
        <nav className="settings-nav card">
          {sections.map((section) => (
            <button
              key={section.key}
              className={`nav-item ${activeSection === section.key ? 'active' : ''}`}
              onClick={() => setActiveSection(section.key)}
            >
              <span className="nav-icon">{section.icon}</span>
              <span className="nav-label">{section.label}</span>
              {activeSection === section.key && <ChevronRight size={16} className="nav-arrow" />}
            </button>
          ))}
        </nav>

        <div className="settings-content card">
          {isLoading ? (
            <div className="loading-state">
              <div className="skeleton-block animate-shimmer" />
              <div className="skeleton-block animate-shimmer" />
              <div className="skeleton-block animate-shimmer" />
            </div>
          ) : (
            <>
              {renderSection()}

              {activeSection !== 'about' && activeSection !== 'account' && activeSection !== 'operations' && (
                <div className="settings-actions">
                  {saveError && <div className="password-error">{saveError}</div>}
                  <div className="settings-actions-buttons">
                    <button className="btn btn-primary" onClick={handleSave} disabled={!hasChanges || saveMutation.isPending}>
                      {saveMutation.isPending ? (
                        <Loader2 size={16} className="animate-spin" />
                      ) : saveSuccess ? (
                        <CheckCircle size={16} />
                      ) : (
                        <Save size={16} />
                      )}
                      {saveMutation.isPending ? t('settings:saving') : saveSuccess ? t('settings:saved') : t('settings:saveSettings')}
                    </button>
                    <button className="btn btn-ghost" onClick={handleReset} disabled={!hasChanges}>
                      <RotateCcw size={16} />
                      {t('settings:resetDefaults')}
                    </button>
                  </div>
                </div>
              )}
            </>
          )}
        </div>
      </div>

      {directoryPickerOpen && (
        <div className="modal-overlay" onClick={closeDirectoryPicker}>
          <div className="modal-content directory-picker-modal" onClick={(event) => event.stopPropagation()}>
            <div className="modal-header">
              <h2>{t('settings:storage.browserTitle', { defaultValue: 'Select Server Directory' })}</h2>
              <button className="modal-close" type="button" onClick={closeDirectoryPicker}>
                <X size={20} />
              </button>
            </div>
            <div className="modal-body directory-picker-body">
              <div className="directory-current-path">
                <span>{t('settings:storage.currentServerPath', { defaultValue: 'Current server path' })}</span>
                <code>{directoryList?.current_path || t('settings:storage.serverRoots', { defaultValue: 'Server roots' })}</code>
              </div>

              <div className="directory-toolbar">
                <button
                  className="btn btn-ghost btn-sm"
                  type="button"
                  onClick={() => directoryList?.parent_path && loadServerDirectory(directoryList.parent_path)}
                  disabled={directoryLoading || !directoryList?.parent_path}
                >
                  {t('settings:storage.upDirectory', { defaultValue: 'Up' })}
                </button>
                <button
                  className="btn btn-ghost btn-sm"
                  type="button"
                  onClick={() => loadServerDirectory()}
                  disabled={directoryLoading}
                >
                  {t('settings:storage.rootDirectory', { defaultValue: 'Roots' })}
                </button>
                <button
                  className="btn btn-ghost btn-sm"
                  type="button"
                  onClick={() => loadServerDirectory('/mnt/host')}
                  disabled={directoryLoading}
                  title={t('settings:storage.hostRootHint', { defaultValue: '浏览宿主机路径(需在 docker-compose 挂载 /mnt/host)' })}
                >
                  {t('settings:storage.hostRoot', { defaultValue: '宿主机' })}
                </button>
                <div className="directory-manual">
                  <input
                    type="text"
                    className="input input-sm"
                    placeholder={t('settings:storage.manualPathPlaceholder', { defaultValue: '粘贴网络路径(如 \\\\server\\share)后回车导航' })}
                    value={manualPath}
                    onChange={(e) => setManualPath(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') {
                        e.preventDefault();
                        navigateToManualPath();
                      }
                    }}
                  />
                  <button
                    className="btn btn-ghost btn-sm"
                    type="button"
                    onClick={navigateToManualPath}
                    disabled={directoryLoading || !manualPath.trim()}
                  >
                    {t('settings:storage.go', { defaultValue: '前往' })}
                  </button>
                </div>
              </div>

              {directoryError && <div className="directory-error">{directoryError}</div>}
              {directoryLoading ? (
                <div className="directory-loading">
                  <Loader2 size={18} className="animate-spin" />
                  {t('common:loading')}
                </div>
              ) : (
                <div className="directory-list">
                  {directoryList?.entries.length ? (
                    directoryList.entries.map((entry) => (
                      <button
                        className="directory-row"
                        type="button"
                        key={entry.path}
                        onClick={() => loadServerDirectory(entry.path)}
                        title={entry.path}
                      >
                        <FolderOpen size={16} />
                        <span>{entry.name}</span>
                      </button>
                    ))
                  ) : (
                    <div className="directory-empty">{t('settings:storage.noDirectories', { defaultValue: 'No child directories' })}</div>
                  )}
                </div>
              )}
            </div>
            <div className="modal-footer">
              <button className="btn btn-ghost" type="button" onClick={closeDirectoryPicker}>
                {t('common:cancel')}
              </button>
              <button
                className="btn btn-primary"
                type="button"
                onClick={selectCurrentDirectory}
                disabled={directoryLoading || !directoryList?.current_path}
              >
                {t('settings:storage.useCurrentDirectory', { defaultValue: 'Use Current Directory' })}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default SettingsPage;
