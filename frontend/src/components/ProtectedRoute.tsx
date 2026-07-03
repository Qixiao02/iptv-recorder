import { Navigate, useLocation } from 'react-router-dom';
import { useAuthStore } from '@/stores/authStore';

interface ProtectedRouteProps {
  children: React.ReactNode;
}

export const ProtectedRoute: React.FC<ProtectedRouteProps> = ({ children }) => {
  const isAuthenticated = useAuthStore((state) => state.isAuthenticated);
  const location = useLocation();

  if (!isAuthenticated) {
    // 重定向到登录页,保存当前路径供登录后回跳。
    // from 用字符串(pathname+search)统一格式,与 401 拦截器、Login 回跳逻辑对齐。
    const from = location.pathname + location.search;
    return <Navigate to="/login" state={{ from }} replace />;
  }

  return <>{children}</>;
};

export default ProtectedRoute;
