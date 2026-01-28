import { useToast } from './ToastContext';
import { X, CheckCircle, XCircle, AlertTriangle, Info } from 'lucide-react';

const icons = {
  success: CheckCircle,
  error: XCircle,
  warning: AlertTriangle,
  info: Info,
};

export function ToastContainer() {
  const { toasts, removeToast } = useToast();

  if (toasts.length === 0) return null;

  return (
    <div 
      className="ccr-toast-container" 
      aria-live="polite" 
      aria-label="通知"
    >
      {toasts.map((toast) => {
        const Icon = icons[toast.type];
        
        return (
          <div
            key={toast.id}
            className={`ccr-toast ccr-toast--${toast.type}`}
            role="alert"
          >
            <div className="ccr-toast__icon">
              <Icon size={20} />
            </div>
            <div className="ccr-toast__content">
              <p className="ccr-toast__title">{toast.title}</p>
              {toast.description && (
                <p className="ccr-toast__description">{toast.description}</p>
              )}
            </div>
            {toast.dismissible && (
              <button
                type="button"
                className="ccr-toast__close"
                onClick={() => removeToast(toast.id)}
                aria-label="关闭通知"
              >
                <X size={16} />
              </button>
            )}
          </div>
        );
      })}
    </div>
  );
}
