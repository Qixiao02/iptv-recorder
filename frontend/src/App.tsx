import { Suspense, lazy, useEffect, type ReactNode } from 'react';
import { RouterProvider, createBrowserRouter, Navigate } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import Layout from '@/components/Layout';
import ProtectedRoute from '@/components/ProtectedRoute';
import ErrorBoundary from '@/components/ErrorBoundary';
import { wsClient } from '@/api/websocket';
import { initTheme } from '@/stores/themeStore';
import { useAuthStore } from '@/stores/authStore';
import { useUIStore } from '@/stores/uiStore';
import type { Channel, Task, TaskProgressData, TaskUpdateData, ChannelStatusData, SystemAlertData } from '@/types';
import { applyTaskProgressUpdate, applyTaskStatusUpdate, patchTaskCache } from '@/lib/taskRealtime';
import '@/locales/index';

const Dashboard = lazy(() => import('@/pages/Dashboard'));
const Channels = lazy(() => import('@/pages/Channels'));
const Schedules = lazy(() => import('@/pages/Schedules'));
const Tasks = lazy(() => import('@/pages/Tasks'));
const Settings = lazy(() => import('@/pages/Settings'));
const Login = lazy(() => import('@/pages/Login'));

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

const RouteFallback = () => <div className="page-loading">Loading...</div>;

const withSuspense = (element: ReactNode) => (
  <Suspense fallback={<RouteFallback />}>
    {element}
  </Suspense>
);

const router = createBrowserRouter([
  {
    path: '/login',
    element: withSuspense(<Login />),
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
      { path: 'dashboard', element: withSuspense(<Dashboard />) },
      { path: 'channels', element: withSuspense(<Channels />) },
      { path: 'schedules', element: withSuspense(<Schedules />) },
      { path: 'tasks', element: withSuspense(<Tasks />) },
      { path: 'settings', element: withSuspense(<Settings />) },
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
      return patchTaskCache(queryClient, updater);
    };

    const unsubscribeProgress = wsClient.onTaskProgress((progress: TaskProgressData) => {
      const changed = updateTaskCache((tasks) => applyTaskProgressUpdate(tasks, progress));
      if (!changed) {
        queryClient.invalidateQueries({ queryKey: ['tasks'] });
      }
    });

    const unsubscribeUpdate = wsClient.onTaskUpdate((update: TaskUpdateData) => {
      const changed = updateTaskCache((tasks) => applyTaskStatusUpdate(tasks, update));
      if (!changed || update.status !== 'running') {
        queryClient.invalidateQueries({ queryKey: ['tasks'] });
      }
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

    const unsubscribeConnection = wsClient.onConnectionStateChange((state) => {
      if (state === 'connected') {
        queryClient.invalidateQueries({ queryKey: ['tasks'] });
      }
    });

    return () => {
      unsubscribeProgress();
      unsubscribeUpdate();
      unsubscribeChannelStatus();
      unsubscribeAlert();
      unsubscribeConnection();
    };
  }, [isAuthenticated, addAlert]);

  return (
    <ErrorBoundary>
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
      </QueryClientProvider>
    </ErrorBoundary>
  );
}

export default App;
