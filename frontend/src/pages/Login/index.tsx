import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate, useLocation } from 'react-router-dom';
import { login } from '@/api/auth';
import { useAuthStore } from '@/stores/authStore';
import { useI18nNamespace } from '@/i18n/useI18nNamespace';
import { User, Lock, Loader2, AlertCircle } from 'lucide-react';
import './Login.css';

export const Login: React.FC = () => {
  const navigate = useNavigate();
  const location = useLocation();
  const { t } = useTranslation(['login', 'common']);
  const isI18nReady = useI18nNamespace(['login', 'common']);
  const setAuth = useAuthStore((state) => state.setAuth);

  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [isLoading, setIsLoading] = useState(false);

  // 登录成功后回跳到来源页:优先 location.state.from(ProtectedRoute / 401 拦截器设置),
  // 兜底 sessionStorage('auth-redirect-from',硬跳转场景),再兜底首页。
  const getRedirectTarget = (): string => {
    const fromState = (location.state as { from?: string } | null)?.from;
    if (fromState && fromState !== '/login') return fromState;
    try {
      const fromSession = sessionStorage.getItem('auth-redirect-from');
      if (fromSession) {
        sessionStorage.removeItem('auth-redirect-from');
        return fromSession;
      }
    } catch {
      // 忽略隐私模式读取失败
    }
    return '/';
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');

    if (!username.trim() || !password.trim()) {
      setError(t('login:required'));
      return;
    }

    setIsLoading(true);

    try {
      const response = await login({ username, password });
      setAuth(response.token, response.user);
      navigate(getRedirectTarget(), { replace: true });
    } catch (err) {
      setError(err instanceof Error ? err.message : t('login:failed'));
    } finally {
      setIsLoading(false);
    }
  };

  if (!isI18nReady) {
    return <div className="page-loading">{t('common:loading')}</div>;
  }

  return (
    <div className="login-page">
      <div className="login-container">
        <div className="login-header">
          <div className="login-logo">
            <img src={`${import.meta.env.BASE_URL}logo.png`} alt="IPTV Recorder" />
          </div>
          <h1 className="login-title">IPTV Recorder</h1>
          <p className="login-subtitle">{t('login:subtitle')}</p>
        </div>

        <form className="login-form" onSubmit={handleSubmit}>
          {error && (
            <div className="login-error">
              <AlertCircle size={18} />
              <span>{error}</span>
            </div>
          )}

          <div className="form-group">
            <div className="input-wrapper">
              <User size={18} className="input-icon" />
              <input
                type="text"
                className="login-input"
                placeholder={t('login:username')}
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                disabled={isLoading}
                autoComplete="username"
              />
            </div>
          </div>

          <div className="form-group">
            <div className="input-wrapper">
              <Lock size={18} className="input-icon" />
              <input
                type="password"
                className="login-input"
                placeholder={t('login:password')}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                disabled={isLoading}
                autoComplete="current-password"
              />
            </div>
          </div>

          <button
            type="submit"
            className="login-button"
            disabled={isLoading}
          >
            {isLoading ? (
              <>
                <Loader2 size={18} className="animate-spin" />
                <span>{t('login:submitting')}</span>
              </>
            ) : (
              <span>{t('login:submit')}</span>
            )}
          </button>
        </form>

        <div className="login-footer">
          <p>{t('login:firstRunHint')}</p>
        </div>
      </div>
    </div>
  );
};

export default Login;
