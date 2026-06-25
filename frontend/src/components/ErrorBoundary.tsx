import React from 'react';
import i18n from '@/i18n';

interface Props {
  children: React.ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

class ErrorBoundary extends React.Component<Props, State> {
  state: State = { hasError: false, error: null };

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    console.error('ErrorBoundary caught:', error, info);
  }

  render() {
    if (this.state.hasError) {
      const title = i18n.t('components:errorBoundary.title', { defaultValue: 'Page Error' });
      const unknown = i18n.t('components:errorBoundary.unknown', { defaultValue: 'Unknown error' });
      const retry = i18n.t('components:errorBoundary.retry', { defaultValue: 'Retry' });

      return (
        <div style={{
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          justifyContent: 'center',
          minHeight: '100vh',
          padding: '24px',
          textAlign: 'center',
        }}>
          <h2 style={{ marginBottom: '12px' }}>{title}</h2>
          <p style={{ color: '#666', marginBottom: '16px' }}>
            {this.state.error?.message || unknown}
          </p>
          <button
            onClick={() => this.setState({ hasError: false, error: null })}
            style={{
              padding: '8px 20px',
              cursor: 'pointer',
              border: '1px solid #d9d9d9',
              borderRadius: '6px',
              background: '#fff',
            }}
          >
            {retry}
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}

export default ErrorBoundary;
