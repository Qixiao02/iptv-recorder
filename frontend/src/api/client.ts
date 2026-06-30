import axios, { AxiosError } from 'axios';
import type { InternalAxiosRequestConfig } from 'axios';
import type { ErrorResponse } from '@/types';
import { getStoredAuthToken, useAuthStore } from '@/stores/authStore';
import i18n from '@/i18n';

const BASE_URL = import.meta.env.VITE_API_BASE_URL || '/api';

export const apiClient = axios.create({
  baseURL: BASE_URL,
  timeout: 30000,
  headers: {
    'Content-Type': 'application/json',
  },
});

// 请求拦截器 - 添加 Token
apiClient.interceptors.request.use(
  (config: InternalAxiosRequestConfig) => {
    const token = getStoredAuthToken();
    if (token) {
      config.headers.Authorization = `Bearer ${token}`;
    }
    return config;
  },
  (error) => Promise.reject(error)
);

/**
 * 401 时的 SPA 导航回调注册。
 *
 * 背景:axios 拦截器运行在 React 组件树之外,拿不到 router 的 navigate hook。
 * 之前用 window.location.href 硬跳,会整页刷新、丢失内存状态(未保存表单/WS 连接),
 * 且绕过 ProtectedRoute 的 from 回跳机制——重新登录后回不到原页面。
 *
 * 解决:App 启动时调用 setAuthNavigator(router.navigate) 注册 SPA 导航函数,
 * 拦截器在 401 时优先调用它(带 state.from 记录来源,供登录后回跳)。未注册时
 * (极早期 401,如 App 尚未挂载)回退到 location.replace,仍保证跳转生效。
 */
let authNavigator: ((to: string, opts?: { replace?: boolean; state?: unknown }) => void) | null = null;

export const setAuthNavigator = (
  navigate: ((to: string, opts?: { replace?: boolean; state?: unknown }) => void) | null,
) => {
  authNavigator = navigate;
};

// 响应拦截器 - 统一错误处理
apiClient.interceptors.response.use(
  (response) => response,
  (error: AxiosError<ErrorResponse>) => {
    // 401 错误 - Token 过期或无效
    if (error.response?.status === 401) {
      // 走 store 正确清理(触发 onRehydrateStorage 等钩子 + 让 isAuthenticated
      // 立即变 false,WS 等订阅者随之断开),而非直接抠 localStorage。
      useAuthStore.getState().logout();

      // SPA 路由跳转,记录当前路径供登录后回跳(对齐 ProtectedRoute 的 from 机制)。
      const from = window.location.pathname + window.location.search;
      const isOnLogin = window.location.pathname === '/login';
      if (!isOnLogin) {
        if (authNavigator) {
          authNavigator('/login', { replace: true, state: { from } });
        } else {
          // 回退:App 尚未注册导航器(极早期 401)。用 sessionStorage 暂存 from,
          // 登录页挂载后读取回跳。
          try {
            sessionStorage.setItem('auth-redirect-from', from);
          } catch {
            // 忽略隐私模式等写入失败
          }
          window.location.replace('/login');
        }
      }
    }

    const message = error.response?.data?.details || error.message || i18n.t('common:requestFailed', { defaultValue: 'Request failed' });
    console.error('API Error:', message);
    return Promise.reject(new Error(message));
  }
);

export default apiClient;
