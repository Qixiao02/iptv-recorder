import React from 'react';
import { useTranslation } from 'react-i18next';
import { AlertTriangle, Info, CheckCircle, XCircle, X } from 'lucide-react';
import type { ToastItem } from './useToast';
import './ConfirmDialog.css';

interface ConfirmDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm: () => void;
  title: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  type?: 'danger' | 'warning' | 'info' | 'success';
  isLoading?: boolean;
}

const iconMap = {
  danger: <XCircle size={24} />,
  warning: <AlertTriangle size={24} />,
  info: <Info size={24} />,
  success: <CheckCircle size={24} />,
};

export const ConfirmDialog: React.FC<ConfirmDialogProps> = ({
  isOpen,
  onClose,
  onConfirm,
  title,
  message,
  confirmText,
  cancelText,
  type = 'warning',
  isLoading = false,
}) => {
  const { t } = useTranslation(['common']);

  if (!isOpen) return null;

  const handleConfirm = () => {
    if (!isLoading) {
      onConfirm();
    }
  };

  return (
    <div className="confirm-overlay" onClick={onClose}>
      <div className={`confirm-dialog ${type}`} onClick={(e) => e.stopPropagation()}>
        <button className="confirm-close" onClick={onClose} disabled={isLoading}>
          <X size={18} />
        </button>

        <div className={`confirm-icon ${type}`}>
          {iconMap[type]}
        </div>

        <h3 className="confirm-title">{title}</h3>
        <p className="confirm-message">{message}</p>

        <div className="confirm-actions">
          <button className="btn btn-ghost" onClick={onClose} disabled={isLoading}>
            {cancelText ?? t('common:cancel')}
          </button>
          <button
            className={`btn btn-${type === 'danger' ? 'danger' : 'primary'}`}
            onClick={handleConfirm}
            disabled={isLoading}
          >
            {isLoading ? t('common:processing') : confirmText ?? t('common:confirm')}
          </button>
        </div>
      </div>
    </div>
  );
};

interface ToastProps {
  message: string;
  type?: 'success' | 'error' | 'info' | 'warning';
  onClose: () => void;
}

export const Toast: React.FC<ToastProps> = ({ message, type = 'info', onClose }) => {
  return (
    <div className={`toast toast-${type}`}>
      <span className="toast-message">{message}</span>
      <button className="toast-close" onClick={onClose}>
        <X size={14} />
      </button>
    </div>
  );
};

export const ToastContainer: React.FC<{
  toasts: ToastItem[];
  onRemove: (id: number) => void;
}> = ({ toasts, onRemove }) => {
  return (
    <div className="toast-container">
      {toasts.map((toast) => (
        <Toast
          key={toast.id}
          message={toast.message}
          type={toast.type}
          onClose={() => onRemove(toast.id)}
        />
      ))}
    </div>
  );
};

export default ConfirmDialog;

