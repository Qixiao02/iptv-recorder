import React, { useState, useRef, useEffect } from 'react';
import { Outlet, useNavigate, useLocation } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useUIStore } from '@/stores/uiStore';
import { useAuthStore } from '@/stores/authStore';
import { useThemeStore } from '@/stores/themeStore';
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
  { key: '/dashboard', icon: LayoutDashboard, label: 'menu.dashboard' },
  { key: '/channels', icon: Tv, label: 'menu.channels' },
  { key: '/schedules', icon: CalendarClock, label: 'menu.schedules' },
  { key: '/tasks', icon: Clapperboard, label: 'menu.tasks' },
  { key: '/settings', icon: Settings, label: 'menu.settings' },
];

export const Layout: React.FC = () => {
  const navigate = useNavigate();
  const location = useLocation();
  const { t } = useTranslation();
  const {
    sidebarCollapsed,
    setSidebarCollapsed,
    alerts,
    markAllAlertsRead,
    dismissAlert,
  } = useUIStore();
  const { user, logout } = useAuthStore();
  const { theme, toggleTheme } = useThemeStore();
  const [showUserMenu, setShowUserMenu] = useState(false);
  const [showAlertMenu, setShowAlertMenu] = useState(false);
  const userMenuRef = useRef<HTMLDivElement>(null);
  const alertMenuRef = useRef<HTMLDivElement>(null);
  const unreadAlertCount = alerts.filter((alert) => !alert.read).length;

  // 点击外部关闭用户菜单
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

  const handleLanguageChange = () => {
    const currentLang = localStorage.getItem('language') || 'zh-CN';
    const newLang = currentLang === 'zh-CN' ? 'en-US' : 'zh-CN';
    localStorage.setItem('language', newLang);
    window.location.reload();
  };

  const handleLogout = () => {
    logout();
    navigate('/login');
  };

  const alertLevelText: Record<string, string> = {
    info: '信息',
    warning: '警告',
    error: '错误',
    critical: '严重',
  };

  return (
    <div className="app-layout">
      {/* Sidebar */}
      <aside className={`sidebar ${sidebarCollapsed ? 'collapsed' : ''}`}>
        {/* Logo */}
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

        {/* Navigation */}
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
                {!sidebarCollapsed && <span>{t(item.label as any)}</span>}
              </div>
            );
          })}
        </nav>

        {/* Collapse Toggle */}
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

      {/* Main Area */}
      <div className={`main-area ${sidebarCollapsed ? 'sidebar-collapsed' : ''}`}>
        {/* Header */}
        <header className="header header-glass">
          <div className="header-left">
            <button className="header-btn mobile-menu">
              <Menu size={20} />
            </button>
          </div>
          <div className="header-right">
            {/* Theme Toggle */}
            <button
              className="header-btn theme-toggle-btn"
              onClick={toggleTheme}
              title={theme === 'dark' ? '切换到亮色模式' : '切换到暗色模式'}
            >
              {theme === 'dark' ? <Sun size={18} /> : <Moon size={18} />}
            </button>

            {/* Notifications */}
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
                    <span>系统告警</span>
                    {alerts.length > 0 && (
                      <button className="alert-clear-btn" onClick={markAllAlertsRead}>
                        全部已读
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
                              <span className="alert-level">{alertLevelText[alert.level]}</span>
                              <span className="alert-time">
                                {new Date(alert.created_at).toLocaleTimeString('zh-CN', {
                                  hour: '2-digit',
                                  minute: '2-digit',
                                })}
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
                    <div className="alert-empty">暂无系统告警</div>
                  )}
                </div>
              )}
            </div>

            {/* Language */}
            <button className="header-btn" onClick={handleLanguageChange}>
              <Globe size={18} />
              <span>{localStorage.getItem('language') === 'en-US' ? 'EN' : '中'}</span>
            </button>

            {/* User Menu */}
            <div className="user-menu-container" ref={userMenuRef}>
              <button
                className="user-menu-trigger"
                onClick={() => setShowUserMenu(!showUserMenu)}
              >
                <div className="user-avatar">
                  <User size={16} />
                </div>
                <span className="user-name">{user?.nickname || user?.username || 'User'}</span>
                <ChevronDown size={14} className={showUserMenu ? 'rotate' : ''} />
              </button>

              {showUserMenu && (
                <div className="user-dropdown">
                  <div className="user-dropdown-header">
                    <div className="user-dropdown-avatar">
                      <User size={20} />
                    </div>
                    <div className="user-dropdown-info">
                      <div className="user-dropdown-name">{user?.nickname || user?.username}</div>
                      <div className="user-dropdown-role">{user?.role === 'admin' ? '管理员' : '用户'}</div>
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
                    <span>设置</span>
                  </button>
                  <button className="user-dropdown-item danger" onClick={handleLogout}>
                    <LogOut size={16} />
                    <span>退出登录</span>
                  </button>
                </div>
              )}
            </div>
          </div>
        </header>

        {/* Content */}
        <main className="content">
          <Outlet />
        </main>
      </div>
    </div>
  );
};

export default Layout;
