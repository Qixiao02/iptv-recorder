import { useEffect } from 'react';
import { RouterProvider, createBrowserRouter, Navigate } from 'react-router-dom';
import { ConfigProvider, App as AntdApp } from 'antd';
import zhCN from 'antd/locale/zh_CN';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import Layout from '@/components/Layout';
import ProtectedRoute from '@/components/ProtectedRoute';
import ErrorBoundary from '@/components/ErrorBoundary';
import Dashboard from '@/pages/Dashboard';
import Channels from '@/pages/Channels';
import Schedules from '@/pages/Schedules';
import Tasks from '@/pages/Tasks';
import Settings from '@/pages/Settings';
import Login from '@/pages/Login';
import { wsClient } from '@/api/websocket';
import { initTheme } from '@/stores/themeStore';
import { useAuthStore } from '@/stores/authStore';
import { useUIStore } from '@/stores/uiStore';
import type { Channel, Task, TaskProgressData, TaskUpdateData, ChannelStatusData, SystemAlertData } from '@/types';
import '@/locales/index';

// 初始化主题
initTheme();

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      refetchOnWindowFocus: false,
    },
  },
});

const router = createBrowserRouter([
  {
    path: '/login',
    element: <Login />,
  },
  {
    path: '/',
    element: (
      <ProtectedRoute>
        <Layout />
      </ProtectedRoute>
    ),
    children: [
      { index: true, element: <Navigate to="/dashboard" replace /> },
      { path: 'dashboard', element: <Dashboard /> },
      { path: 'channels', element: <Channels /> },
      { path: 'schedules', element: <Schedules /> },
      { path: 'tasks', element: <Tasks /> },
      { path: 'settings', element: <Settings /> },
    ],
  },
]);

function App() {
  const isAuthenticated = useAuthStore((state) => state.isAuthenticated);
  const addAlert = useUIStore((state) => state.addAlert);

  useEffect(() => {
    if (isAuthenticated) {
      wsClient.connect();
      return () => wsClient.disconnect();
    }

    wsClient.disconnect();
    return undefined;
  }, [isAuthenticated]);

  useEffect(() => {
    if (!isAuthenticated) {
      return undefined;
    }

    const updateTaskCache = (updater: (tasks: Task[]) => Task[]) => {
      queryClient.setQueryData<Task[]>(['tasks'], (current) => {
        if (!current) {
          return current;
        }
        return updater(current);
      });
    };

    const unsubscribeProgress = wsClient.onTaskProgress((progress: TaskProgressData) => {
      updateTaskCache((tasks) =>
        tasks.map((task) =>
          task.id === progress.task_id
            ? {
                ...task,
                progress_percent: progress.percent,
                current_speed: progress.speed || task.current_speed,
                file_size: Number(progress.downloaded_bytes) || task.file_size,
              }
            : task
        )
      );
    });

    const unsubscribeUpdate = wsClient.onTaskUpdate((update: TaskUpdateData) => {
      updateTaskCache((tasks) =>
        tasks.map((task) =>
          task.id === update.task_id
            ? {
                ...task,
                status: update.status,
                error_message: update.error_message,
                progress_percent: update.status === 'completed' ? 100 : task.progress_percent,
              }
            : task
        )
      );

      queryClient.invalidateQueries({ queryKey: ['tasks'] });
    });

    const unsubscribeChannelStatus = wsClient.onChannelStatus((update: ChannelStatusData) => {
      queryClient.setQueriesData({ queryKey: ['channels'] }, (current: unknown) => {
        if (Array.isArray(current)) {
          return current.map((channel) =>
            channel && typeof channel === 'object' && 'id' in channel && (channel as Channel).id === update.channel_id
              ? { ...(channel as Channel), status: update.status as Channel['status'] }
              : channel
          );
        }

        if (
          current &&
          typeof current === 'object' &&
          'items' in current &&
          Array.isArray((current as { items: Channel[] }).items)
        ) {
          return {
            ...(current as { items: Channel[] }),
            items: (current as { items: Channel[] }).items.map((channel) =>
              channel.id === update.channel_id
                ? { ...channel, status: update.status as Channel['status'] }
                : channel
            ),
          };
        }

        return current;
      });
    });

    const unsubscribeAlert = wsClient.onSystemAlert((alert: SystemAlertData) => {
      addAlert(alert);
    });

    return () => {
      unsubscribeProgress();
      unsubscribeUpdate();
      unsubscribeChannelStatus();
      unsubscribeAlert();
    };
  }, [isAuthenticated, addAlert]);

  return (
    <ErrorBoundary>
      <QueryClientProvider client={queryClient}>
        <ConfigProvider locale={zhCN}>
          <AntdApp>
            <RouterProvider router={router} />
          </AntdApp>
        </ConfigProvider>
      </QueryClientProvider>
    </ErrorBoundary>
  );
}

export default App;
