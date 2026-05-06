import React, { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getConfig, updateConfig } from '@/api/system';
import { changePassword } from '@/api/auth';
import { useAuthStore } from '@/stores/authStore';
import { useSettingStore } from '@/stores/settingStore';
import type { SystemConfig } from '@/types';
import { buildConfigUpdateRequest } from './configPayload';
import {
  Settings,
  Database,
  Clapperboard,
  Bell,
  Info,
  ChevronRight,
  Save,
  RotateCcw,
  Globe,
  FolderOpen,
  Zap,
  HardDrive,
  Loader2,
  CheckCircle,
  User,
  Lock,
  Eye,
  EyeOff,
} from 'lucide-react';
import './Settings.css';

type SettingsSection = 'general' | 'storage' | 'recording' | 'notification' | 'account' | 'about';

// 默认配置（用于初始化和重置）
const defaultConfig: SystemConfig = {
  server: { host: '127.0.0.1', port: 3000 },
  storage: { recordings_path: './data/recordings', auto_cleanup_days: 30, min_free_space_gb: 10 },
  recording: { default_duration_minutes: 60, n_m3u8dl_re_path: 'N_m3u8DL-RE', max_retry: 3, thread_count: 4 },
  notification: { on_complete: true, on_failure: true, disk_warning: true },
};

export const SettingsPage: React.FC = () => {
  const { t, i18n } = useTranslation();
  const queryClient = useQueryClient();
  const { language, setLanguage } = useSettingStore();
  const [activeSection, setActiveSection] = useState<SettingsSection>('general');
  const [localConfig, setLocalConfig] = useState<SystemConfig>(defaultConfig);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const [saveError, setSaveError] = useState('');

  const sections: { key: SettingsSection; icon: React.ReactNode; label: string }[] = [
    { key: 'general', icon: <Settings size={18} />, label: t('settings.general') },
    { key: 'storage', icon: <Database size={18} />, label: t('settings.storage') },
    { key: 'recording', icon: <Clapperboard size={18} />, label: t('settings.recording') },
    { key: 'notification', icon: <Bell size={18} />, label: t('settings.notification') },
    { key: 'account', icon: <User size={18} />, label: t('settings.account') },
    { key: 'about', icon: <Info size={18} />, label: t('settings.about') },
  ];

  // 密码修改状态
  const [passwordForm, setPasswordForm] = useState({
    old_password: '',
    new_password: '',
    confirm_password: '',
  });
  const [showOldPassword, setShowOldPassword] = useState(false);
  const [showNewPassword, setShowNewPassword] = useState(false);
  const [passwordSuccess, setPasswordSuccess] = useState(false);
  const [passwordError, setPasswordError] = useState('');

  const { user } = useAuthStore();

  // 获取配置
  const { data: config, isLoading } = useQuery({
    queryKey: ['config'],
    queryFn: getConfig,
  });

  // 当从服务器获取到配置时，更新本地状态
  useEffect(() => {
    if (config) {
      setLocalConfig(config);
    }
  }, [config]);

  // 保存配置
  const saveMutation = useMutation({
    mutationFn: updateConfig,
    onSuccess: (data) => {
      setLocalConfig(data);
      queryClient.setQueryData(['config'], data);
      setSaveSuccess(true);
      setSaveError('');
      setTimeout(() => setSaveSuccess(false), 2000);
    },
    onError: (error) => {
      setSaveError(error instanceof Error ? error.message : t('settings.settingsSaveFailed'));
    },
  });

  // 修改密码
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
    },
    onError: (error) => {
      setPasswordError(error instanceof Error ? error.message : '修改密码失败');
    },
  });

  // 修改密码提交
  const handlePasswordSubmit = () => {
    setPasswordError('');

    if (!passwordForm.old_password || !passwordForm.new_password || !passwordForm.confirm_password) {
      setPasswordError('请填写所有密码字段');
      return;
    }

    if (passwordForm.new_password !== passwordForm.confirm_password) {
      setPasswordError('两次输入的新密码不一致');
      return;
    }

    if (passwordForm.new_password.length < 6) {
      setPasswordError('新密码长度至少为6位');
      return;
    }

    passwordMutation.mutate();
  };

  // 检查是否有变更
  const hasChanges = JSON.stringify(localConfig) !== JSON.stringify(config);

  // 更新本地配置（深拷贝路径上的每一层，避免修改原始引用）
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

  // 保存
  const handleSave = () => {
    saveMutation.mutate(buildConfigUpdateRequest(localConfig));
  };

  // 重置
  const handleReset = () => {
    if (config) {
      setLocalConfig(config);
      setSaveError('');
    }
  };

  const renderSection = () => {
    switch (activeSection) {
      case 'general':
        return (
          <div className="settings-section">
            <h2>{t('settings.general')}</h2>
            <div className="setting-item">
              <div className="setting-info">
                <label>语言</label>
                <span className="setting-desc">选择界面显示语言</span>
              </div>
              <select
                className="input setting-control"
                value={language}
                onChange={(e) => {
                  const lang = e.target.value as 'zh-CN' | 'en-US';
                  setLanguage(lang);
                  i18n.changeLanguage(lang);
                }}
              >
                <option value="zh-CN">简体中文</option>
                <option value="en-US">English</option>
              </select>
            </div>
            <div className="setting-item">
              <div className="setting-info">
                <label>服务器地址</label>
                <span className="setting-desc">Web 服务器监听地址（只读）</span>
              </div>
              <input
                type="text"
                className="input setting-control"
                value={localConfig.server.host}
                readOnly
              />
            </div>
            <div className="setting-item">
              <div className="setting-info">
                <label>服务器端口</label>
                <span className="setting-desc">Web 服务器监听端口（只读）</span>
              </div>
              <input
                type="number"
                className="input setting-control"
                value={localConfig.server.port}
                readOnly
              />
            </div>
          </div>
        );

      case 'storage':
        return (
          <div className="settings-section">
            <h2>{t('settings.storage')}</h2>
            <div className="setting-item">
              <div className="setting-info">
                <label>录制保存路径</label>
                <span className="setting-desc">录制文件的存储目录</span>
              </div>
              <div className="input-with-btn">
                <input
                  type="text"
                  className="input"
                  value={localConfig.storage.recordings_path}
                  onChange={(e) => updateLocalConfig('storage.recordings_path', e.target.value)}
                />
                <button className="btn btn-ghost" type="button">
                  <FolderOpen size={16} />
                </button>
              </div>
            </div>
            <div className="setting-item">
              <div className="setting-info">
                <label>自动清理</label>
                <span className="setting-desc">自动删除超过指定天数的录制文件（0 = 禁用）</span>
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
                <span className="input-suffix">天</span>
              </div>
            </div>
            <div className="setting-item">
              <div className="setting-info">
                <label>最小剩余空间</label>
                <span className="setting-desc">当磁盘空间低于此值时发出警告</span>
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
            <h2>{t('settings.recording')}</h2>
            <div className="setting-item">
              <div className="setting-info">
                <label>默认录制时长</label>
                <span className="setting-desc">手动录制时的默认时长</span>
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
                <span className="input-suffix">分钟</span>
              </div>
            </div>
            <div className="setting-item">
              <div className="setting-info">
                <label>录制工具路径</label>
                <span className="setting-desc">N_m3u8DL-RE 可执行文件路径</span>
              </div>
              <input
                type="text"
                className="input setting-control"
                value={localConfig.recording.n_m3u8dl_re_path}
                onChange={(e) => updateLocalConfig('recording.n_m3u8dl_re_path', e.target.value)}
              />
            </div>
            <div className="setting-item">
              <div className="setting-info">
                <label>最大重试次数</label>
                <span className="setting-desc">录制失败时的最大重试次数</span>
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
                <label>下载线程数</label>
                <span className="setting-desc">并发下载线程数量</span>
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
            <h2>{t('settings.notification')}</h2>
            <div className="setting-item">
              <div className="setting-info">
                <label>录制完成通知</label>
                <span className="setting-desc">当录制任务完成时发送通知</span>
              </div>
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={localConfig.notification.on_complete}
                  onChange={(e) => updateLocalConfig('notification.on_complete', e.target.checked)}
                />
                <span className="toggle-slider" />
              </label>
            </div>
            <div className="setting-item">
              <div className="setting-info">
                <label>录制失败通知</label>
                <span className="setting-desc">当录制任务失败时发送通知</span>
              </div>
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={localConfig.notification.on_failure}
                  onChange={(e) => updateLocalConfig('notification.on_failure', e.target.checked)}
                />
                <span className="toggle-slider" />
              </label>
            </div>
            <div className="setting-item">
              <div className="setting-info">
                <label>磁盘空间警告</label>
                <span className="setting-desc">当磁盘空间不足时发送警告</span>
              </div>
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={localConfig.notification.disk_warning}
                  onChange={(e) => updateLocalConfig('notification.disk_warning', e.target.checked)}
                />
                <span className="toggle-slider" />
              </label>
            </div>
          </div>
        );

      case 'account':
        return (
          <div className="settings-section">
            <h2>{t('settings.account')}</h2>

            {/* 用户信息 */}
            <div className="account-info-card">
              <div className="account-avatar">
                <User size={24} />
              </div>
              <div className="account-details">
                <div className="account-name">{user?.nickname || user?.username}</div>
                <div className="account-meta">
                  <span className="account-role">{user?.role === 'admin' ? '管理员' : '用户'}</span>
                  <span className="account-divider">|</span>
                  <span className="account-username">@{user?.username}</span>
                </div>
              </div>
            </div>

            {/* 修改密码 */}
            <div className="password-section">
              <h3 className="section-subtitle">
                <Lock size={18} />
                修改密码
              </h3>

              {passwordSuccess && (
                <div className="password-success">
                  <CheckCircle size={18} />
                  密码修改成功
                </div>
              )}

              {passwordError && (
                <div className="password-error">
                  {passwordError}
                </div>
              )}

              <div className="form-group">
                <label>当前密码</label>
                <div className="password-input-wrapper">
                  <input
                    type={showOldPassword ? 'text' : 'password'}
                    className="input"
                    value={passwordForm.old_password}
                    onChange={(e) => setPasswordForm({ ...passwordForm, old_password: e.target.value })}
                    placeholder="请输入当前密码"
                  />
                  <button
                    type="button"
                    className="password-toggle"
                    onClick={() => setShowOldPassword(!showOldPassword)}
                  >
                    {showOldPassword ? <EyeOff size={18} /> : <Eye size={18} />}
                  </button>
                </div>
              </div>

              <div className="form-group">
                <label>新密码</label>
                <div className="password-input-wrapper">
                  <input
                    type={showNewPassword ? 'text' : 'password'}
                    className="input"
                    value={passwordForm.new_password}
                    onChange={(e) => setPasswordForm({ ...passwordForm, new_password: e.target.value })}
                    placeholder="请输入新密码（至少6位）"
                  />
                  <button
                    type="button"
                    className="password-toggle"
                    onClick={() => setShowNewPassword(!showNewPassword)}
                  >
                    {showNewPassword ? <EyeOff size={18} /> : <Eye size={18} />}
                  </button>
                </div>
              </div>

              <div className="form-group">
                <label>确认新密码</label>
                <input
                  type="password"
                  className="input"
                  value={passwordForm.confirm_password}
                  onChange={(e) => setPasswordForm({ ...passwordForm, confirm_password: e.target.value })}
                  placeholder="请再次输入新密码"
                />
              </div>

              <button
                className="btn btn-primary"
                onClick={handlePasswordSubmit}
                disabled={passwordMutation.isPending}
              >
                {passwordMutation.isPending ? (
                  <>
                    <Loader2 size={16} className="animate-spin" />
                    修改中...
                  </>
                ) : (
                  <>
                    <Lock size={16} />
                    修改密码
                  </>
                )}
              </button>
            </div>
          </div>
        );

      case 'about':
        return (
          <div className="settings-section">
            <h2>{t('settings.about')}</h2>
            <div className="about-card">
              <div className="about-logo">
                <svg viewBox="0 0 48 48" fill="none" xmlns="http://www.w3.org/2000/svg">
                  <rect width="48" height="48" rx="12" fill="url(#about-gradient)" />
                  <path
                    d="M12 15H36M12 24H30M12 33H24"
                    stroke="white"
                    strokeWidth="3"
                    strokeLinecap="round"
                  />
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
                <p className="version">版本 0.1.0</p>
                <p className="description">
                  基于 Rust 的 IPTV M3U 管理与定时录制系统
                </p>
              </div>
            </div>
            <div className="about-links">
              <span className="about-link disabled">
                <Globe size={18} />
                <span>官方网站</span>
              </span>
              <span className="about-link disabled">
                <Zap size={18} />
                <span>检查更新</span>
              </span>
              <span className="about-link disabled">
                <HardDrive size={18} />
                <span>GitHub</span>
              </span>
            </div>
            <div className="tech-stack">
              <h4>技术栈</h4>
              <div className="tech-tags">
                <span className="tech-tag">Rust</span>
                <span className="tech-tag">Axum</span>
                <span className="tech-tag">SQLite</span>
                <span className="tech-tag">React</span>
                <span className="tech-tag">TypeScript</span>
              </div>
            </div>
          </div>
        );
    }
  };

  return (
    <div className="settings-page">
      {/* Page Header */}
      <div className="page-header">
        <div className="page-title">
          <h1>{t('menu.settings')}</h1>
          <p className="page-subtitle">系统配置与偏好设置</p>
        </div>
      </div>

      {/* Settings Layout */}
      <div className="settings-layout">
        {/* Sidebar Navigation */}
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

        {/* Content Area */}
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

              {/* Save Actions */}
              {activeSection !== 'about' && activeSection !== 'account' && (
                <div className="settings-actions">
                  {saveError && (
                    <div className="password-error">{saveError}</div>
                  )}
                  <div className="settings-actions-buttons">
                    <button
                      className="btn btn-primary"
                      onClick={handleSave}
                      disabled={!hasChanges || saveMutation.isPending}
                    >
                      {saveMutation.isPending ? (
                        <Loader2 size={16} className="animate-spin" />
                      ) : saveSuccess ? (
                        <CheckCircle size={16} />
                      ) : (
                        <Save size={16} />
                      )}
                      {saveMutation.isPending ? t('settings.saving') : saveSuccess ? t('settings.saved') : t('settings.saveSettings')}
                    </button>
                    <button
                      className="btn btn-ghost"
                      onClick={handleReset}
                      disabled={!hasChanges}
                    >
                      <RotateCcw size={16} />
                      {t('settings.resetDefaults')}
                    </button>
                  </div>
                </div>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
};

export default SettingsPage;
