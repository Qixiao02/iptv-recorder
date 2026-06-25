import React, { useState, useRef, useEffect } from 'react';
import { Outlet, useNavigate, useLocation } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useUIStore } from '@/stores/uiStore';
import { useAuthStore } from '@/stores/authStore';
import { useThemeStore } from '@/stores/themeStore';
import { useSettingStore } from '@/stores/settingStore';
import { useI18nNamespace } from '@/i18n/useI18nNamespace';
import { formatShortDateTime } from '@/i18n/format';
import type { AppLanguage } from '@/i18n/types';
import {
  LayoutDashboard,
  Tv,
  CalendarClock,
  Clapperboard,
  Settings,
  ChevronLeft,
  ChevronRight,
  Globe,
  Bell,
  Menu,
  LogOut,
  User,
  ChevronDown,
  Sun,
  Moon,
  X,
} from 'lucide-react';
import './Layout.css';

const navItems = [
  { key: '/dashboard', icon: LayoutDashboard, label: 'layout:menu.dashboard' },
  { key: '/channels', icon: Tv, label: 'layout:menu.channels' },
  { key: '/schedules', icon: CalendarClock, label: 'layout:menu.schedules' },
  { key: '/tasks', icon: Clapperboard, label: 'layout:menu.tasks' },
  { key: '/settings', icon: Settings, label: 'layout:menu.settings' },
];

export const Layout: React.FC = () => {
  const navigate = useNavigate();
  const location = useLocation();
  const { t, i18n } = useTranslation(['layout', 'common']);
  const isI18nReady = useI18nNamespace(['layout', 'common']);
  const {
    sidebarCollapsed,
    setSidebarCollapsed,
    alerts,
    markAllAlertsRead,
    dismissAlert,
  } = useUIStore();
  const { user, logout } = useAuthStore();
  const { theme, toggleTheme } = useThemeStore();
  const { language, setLanguage } = useSettingStore();
  const [showUserMenu, setShowUserMenu] = useState(false);
  const [showAlertMenu, setShowAlertMenu] = useState(false);
  const userMenuRef = useRef<HTMLDivElement>(null);
  const alertMenuRef = useRef<HTMLDivElement>(null);
  const unreadAlertCount = alerts.filter((alert) => !alert.read).length;

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (userMenuRef.current && !userMenuRef.current.contains(event.target as Node)) {
        setShowUserMenu(false);
      }
      if (alertMenuRef.current && !alertMenuRef.current.contains(event.target as Node)) {
        setShowAlertMenu(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const handleLanguageChange = async () => {
    await setLanguage(language === 'zh-CN' ? 'en-US' : 'zh-CN');
  };

  const handleLogout = () => {
    logout();
    navigate('/login');
  };
  const displayName =
    user?.nickname === '管理员'
      ? t(user.role === 'admin' ? 'common:admin' : 'common:user')
      : user?.nickname || user?.username || 'User';

  if (!isI18nReady) {
    return <div className="page-loading">{t('common:loading')}</div>;
  }

  return (
    <div className="app-layout">
      <aside className={`sidebar ${sidebarCollapsed ? 'collapsed' : ''}`}>
        <div className="sidebar-logo">
          <div className="logo-icon">
            <svg viewBox="0 0 32 32" fill="none" xmlns="http://www.w3.org/2000/svg">
              <rect width="32" height="32" rx="8" fill="url(#logo-gradient)" />
              <path
                d="M8 10H24M8 16H20M8 22H16"
                stroke="white"
                strokeWidth="2.5"
                strokeLinecap="round"
              />
              <circle cx="24" cy="22" r="4" fill="white" fillOpacity="0.9" />
              <defs>
                <linearGradient id="logo-gradient" x1="0" y1="0" x2="32" y2="32" gradientUnits="userSpaceOnUse">
                  <stop stopColor="#3B82F6" />
                  <stop offset="1" stopColor="#8B5CF6" />
                </linearGradient>
              </defs>
            </svg>
          </div>
          {!sidebarCollapsed && (
            <span className="logo-text">IPTV Recorder</span>
          )}
        </div>

        <nav className="sidebar-nav">
          {navItems.map((item) => {
            const Icon = item.icon;
            const isActive = location.pathname === item.key;
            return (
              <div
                key={item.key}
                className={`sidebar-item ${isActive ? 'active' : ''}`}
                onClick={() => navigate(item.key)}
              >
                <Icon size={20} strokeWidth={1.75} />
                {!sidebarCollapsed && <span>{t(item.label)}</span>}
              </div>
            );
          })}
        </nav>

        <button
          className="sidebar-toggle"
          onClick={() => setSidebarCollapsed(!sidebarCollapsed)}
        >
          {sidebarCollapsed ? (
            <ChevronRight size={16} />
          ) : (
            <ChevronLeft size={16} />
          )}
        </button>
      </aside>

      <div className={`main-area ${sidebarCollapsed ? 'sidebar-collapsed' : ''}`}>
        <header className="header header-glass">
          <div className="header-left">
            <button className="header-btn mobile-menu">
              <Menu size={20} />
            </button>
          </div>
          <div className="header-right">
            <button
              className="header-btn theme-toggle-btn"
              onClick={toggleTheme}
              title={theme === 'dark' ? t('layout:theme.switchToLight') : t('layout:theme.switchToDark')}
            >
              {theme === 'dark' ? <Sun size={18} /> : <Moon size={18} />}
            </button>

            <div className="alert-menu-container" ref={alertMenuRef}>
              <button
                className="header-btn"
                onClick={() => {
                  const nextOpen = !showAlertMenu;
                  setShowAlertMenu(nextOpen);
                  if (!showAlertMenu) {
                    markAllAlertsRead();
                  }
                }}
              >
                <Bell size={18} />
                {unreadAlertCount > 0 && (
                  <span className="notification-badge">{Math.min(unreadAlertCount, 9)}</span>
                )}
              </button>

              {showAlertMenu && (
                <div className="alert-dropdown">
                  <div className="alert-dropdown-header">
                    <span>{t('layout:alerts.title')}</span>
                    {alerts.length > 0 && (
                      <button className="alert-clear-btn" onClick={markAllAlertsRead}>
                        {t('layout:alerts.markAllRead')}
                      </button>
                    )}
                  </div>
                  <div className="alert-dropdown-divider" />
                  {alerts.length > 0 ? (
                    <div className="alert-list">
                      {alerts.map((alert) => (
                        <div key={alert.id} className={`alert-item alert-${alert.level}`}>
                          <div className="alert-item-main">
                            <div className="alert-item-top">
                              <span className="alert-level">{t(`layout:alerts.levels.${alert.level}`)}</span>
                              <span className="alert-time">
                                {formatShortDateTime(alert.created_at, i18n.language as AppLanguage)}
                              </span>
                            </div>
                            <div className="alert-message">{alert.message}</div>
                            {alert.details && (
                              <div className="alert-details">{alert.details}</div>
                            )}
                          </div>
                          <button
                            className="alert-dismiss-btn"
                            onClick={() => dismissAlert(alert.id)}
                          >
                            <X size={14} />
                          </button>
                        </div>
                      ))}
                    </div>
                  ) : (
                    <div className="alert-empty">{t('layout:alerts.empty')}</div>
                  )}
                </div>
              )}
            </div>

            <button className="header-btn" onClick={handleLanguageChange} title={t('layout:language.label')}>
              <Globe size={18} />
              <span>{t('layout:language.short')}</span>
            </button>

            <div className="user-menu-container" ref={userMenuRef}>
              <button
                className="user-menu-trigger"
                onClick={() => setShowUserMenu(!showUserMenu)}
              >
                <div className="user-avatar">
                  <User size={16} />
                </div>
                <span className="user-name">{displayName}</span>
                <ChevronDown size={14} className={showUserMenu ? 'rotate' : ''} />
              </button>

              {showUserMenu && (
                <div className="user-dropdown">
                  <div className="user-dropdown-header">
                    <div className="user-dropdown-avatar">
                      <User size={20} />
                    </div>
                    <div className="user-dropdown-info">
                      <div className="user-dropdown-name">{displayName}</div>
                      <div className="user-dropdown-role">{user?.role === 'admin' ? t('common:admin') : t('common:user')}</div>
                    </div>
                  </div>
                  <div className="user-dropdown-divider" />
                  <button
                    className="user-dropdown-item"
                    onClick={() => {
                      setShowUserMenu(false);
                      navigate('/settings');
                    }}
                  >
                    <Settings size={16} />
                    <span>{t('common:settings')}</span>
                  </button>
                  <button className="user-dropdown-item danger" onClick={handleLogout}>
                    <LogOut size={16} />
                    <span>{t('common:logout')}</span>
                  </button>
                </div>
              )}
            </div>
          </div>
        </header>

        <main className="content">
          <Outlet />
        </main>
      </div>
    </div>
  );
};

export default Layout;
