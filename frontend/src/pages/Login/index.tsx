import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { login } from '@/api/auth';
import { useAuthStore } from '@/stores/authStore';
import { useI18nNamespace } from '@/i18n/useI18nNamespace';
import { Clapperboard, User, Lock, Loader2, AlertCircle } from 'lucide-react';
import './Login.css';

export const Login: React.FC = () => {
  const navigate = useNavigate();
  const { t } = useTranslation(['login', 'common']);
  const isI18nReady = useI18nNamespace(['login', 'common']);
  const setAuth = useAuthStore((state) => state.setAuth);

  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [isLoading, setIsLoading] = useState(false);

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
      navigate('/');
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
            <Clapperboard size={48} strokeWidth={1.5} />
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
