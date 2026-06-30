import React, { useState, useRef, useEffect, Suspense, lazy } from 'react';
import { Outlet, useNavigate, useLocation } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useUIStore } from '@/stores/uiStore';
import { useAuthStore } from '@/stores/authStore';
import { useThemeStore } from '@/stores/themeStore';
import { useSettingStore } from '@/stores/settingStore';
import { useNotificationStore } from '@/stores/notificationStore';
import { useToastStore } from '@/stores/toastStore';
import { ToastContainer } from '@/components/ConfirmDialog';
// 悬浮迷你播放器：全局常驻，跨路由保持播放（不遮挡背景页面）
const MiniPlayer = lazy(() => import('@/components/MiniPlayer'));
import { useI18nNamespace } from '@/i18n/useI18nNamespace';
import { formatShortDateTime } from '@/i18n/format';
import type { AppLanguage } from '@/i18n/types';
import { getNotifications } from '@/api/notifications';
import { notificationKeys } from '@/lib/queryKeys';
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
  } = useUIStore();
  const { user, logout } = useAuthStore();
  const { theme, toggleTheme } = useThemeStore();
  const { language, setLanguage } = useSettingStore();
  const { unreadCount: persistentUnreadCount, markRead, markAllRead } = useNotificationStore();
  const { toasts, removeToast } = useToastStore();
  const queryClient = useQueryClient();
  const [showUserMenu, setShowUserMenu] = useState(false);
  const [showAlertMenu, setShowAlertMenu] = useState(false);
  const [notifPage, setNotifPage] = useState(1);
  // 移动端侧边栏抽屉开关(≤768px 才显示汉堡按钮)。桌面端侧边栏始终可见,与此状态无关。
  const [mobileNavOpen, setMobileNavOpen] = useState(false);
  const userMenuRef = useRef<HTMLDivElement>(null);
  const alertMenuRef = useRef<HTMLDivElement>(null);
  const unreadAlertCount = alerts.filter((alert) => !alert.read).length;
  // 角标：持久化未读为主，session 告警作为补充（不丢历史实时流）
  const badgeCount = persistentUnreadCount + unreadAlertCount;

  // 持久化通知分页查询（铃铛下拉打开时展示）
  const { data: notifData, isFetching: notifLoading } = useQuery({
    queryKey: notificationKeys.list(notifPage),
    queryFn: () => getNotifications({ page: notifPage, page_size: 10 }),
    enabled: showAlertMenu,
    placeholderData: (prev) => prev,
  });
  const notifItems = notifData?.items ?? [];
  const notifTotalPages = notifData?.total_pages ?? 1;

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
  // 跳转路由同时收起移动端抽屉(桌面端不受影响)
  const handleNavigate = (key: string) => {
    navigate(key);
    setMobileNavOpen(false);
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
      <aside id="primary-sidebar" className={`sidebar ${sidebarCollapsed ? 'collapsed' : ''} ${mobileNavOpen ? 'mobile-open' : ''}`}>
        <div className="sidebar-logo">
          <div className="logo-icon">
            <img src={`${import.meta.env.BASE_URL}logo.png`} alt="IPTV Recorder" />
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
                onClick={() => handleNavigate(item.key)}
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

      {/* 移动端侧边栏打开时的半透明遮罩:点击关闭抽屉。仅 mobile-open 时渲染。 */}
      {mobileNavOpen && (
        <div
          className="sidebar-backdrop"
          onClick={() => setMobileNavOpen(false)}
          aria-hidden="true"
        />
      )}

      <div className={`main-area ${sidebarCollapsed ? 'sidebar-collapsed' : ''}`}>
        <header className="header header-glass">
          <div className="header-left">
            <button
              className="header-btn mobile-menu"
              onClick={() => setMobileNavOpen(true)}
              aria-label={t('layout:menu.openMenu')}
              aria-expanded={mobileNavOpen}
              aria-controls="primary-sidebar"
            >
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
                  if (nextOpen) {
                    setNotifPage(1);
                    // 打开即视为已查看：标记全部已读（持久化）
                    if (persistentUnreadCount > 0) {
                      markAllRead().then(() => {
                        queryClient.invalidateQueries({ queryKey: notificationKeys.root });
                      });
                    }
                    markAllAlertsRead();
                  }
                }}
              >
                <Bell size={18} />
                {badgeCount > 0 && (
                  <span className="notification-badge">{Math.min(badgeCount, 9)}</span>
                )}
              </button>

              {showAlertMenu && (
                <div className="alert-dropdown">
                  <div className="alert-dropdown-header">
                    <span>{t('layout:alerts.title')}</span>
                    <div className="alert-header-actions">
                      {persistentUnreadCount > 0 && (
                        <button
                          className="alert-clear-btn"
                          onClick={() => {
                            markAllRead().then(() => {
                              queryClient.invalidateQueries({ queryKey: notificationKeys.root });
                            });
                          }}
                        >
                          {t('layout:alerts.markAllRead')}
                        </button>
                      )}
                    </div>
                  </div>
                  <div className="alert-dropdown-divider" />
                  {notifLoading && notifItems.length === 0 ? (
                    <div className="alert-empty">{t('common:loading', { defaultValue: 'Loading…' })}</div>
                  ) : notifItems.length > 0 ? (
                    <>
                      <div className="alert-list">
                        {notifItems.map((n) => (
                          <div
                            key={n.id}
                            className={`alert-item alert-${n.level} ${n.read ? 'alert-read' : 'alert-unread'}`}
                            onClick={() => {
                              if (!n.read) {
                                markRead(n.id).then(() => {
                                  queryClient.invalidateQueries({ queryKey: notificationKeys.root });
                                });
                              }
                            }}
                          >
                            <div className="alert-item-main">
                              <div className="alert-item-top">
                                <span className="alert-level">{t(`layout:alerts.categories.${n.category}`, { defaultValue: n.category })}</span>
                                <span className="alert-time">
                                  {formatShortDateTime(n.created_at, i18n.language as AppLanguage)}
                                </span>
                              </div>
                              <div className="alert-title">{n.title}</div>
                              <div className="alert-message">{n.message}</div>
                            </div>
                          </div>
                        ))}
                      </div>
                      {notifTotalPages > 1 && (
                        <div className="alert-dropdown-footer">
                          <button
                            className="alert-page-btn"
                            disabled={notifPage <= 1}
                            onClick={() => setNotifPage((p) => Math.max(1, p - 1))}
                          >
                            <ChevronLeft size={14} />
                          </button>
                          <span className="alert-page-info">
                            {notifPage} / {notifTotalPages}
                          </span>
                          <button
                            className="alert-page-btn"
                            disabled={notifPage >= notifTotalPages}
                            onClick={() => setNotifPage((p) => Math.min(notifTotalPages, p + 1))}
                          >
                            <ChevronRight size={14} />
                          </button>
                        </div>
                      )}
                    </>
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

      {/* 全局 toast 容器:所有页面/弹窗共享,右上角弹出提示 */}
      <ToastContainer toasts={toasts} onRemove={removeToast} />

      {/* 悬浮迷你播放器:右下角常驻,跨路由保持,不遮挡背景 */}
      <Suspense fallback={null}>
        <MiniPlayer />
      </Suspense>
    </div>
  );
};

export default Layout;
