import { createContext, useContext, useCallback, useState } from 'react';
import { Snackbar, Alert, AlertColor } from '@mui/material';

export interface ToastMessage {
  id: string;
  type: 'success' | 'error' | 'warning' | 'info';
  priority: number;
  timestamp: number;
  title: string;
  description?: string;
  duration?: number;
  dismissible?: boolean;
}

export interface ToastInput {
  type: 'success' | 'error' | 'warning' | 'info';
  title: string;
  description?: string;
  duration?: number;
  dismissible?: boolean;
  priority?: number;
}

export interface ToastContextValue {
  toasts: ToastMessage[];
  addToast: (toast: ToastInput) => void;
  removeToast: (id: string) => void;
  success: (title: string, description?: string) => void;
  error: (title: string, description?: string) => void;
  warning: (title: string, description?: string) => void;
  info: (title: string, description?: string) => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

const DEFAULT_DURATION = 3000;
const MAX_TOASTS = 3;

// Priority levels: higher number = higher priority
export const PRIORITY_ERROR = 10;
export const PRIORITY_WARNING = 5;
export const PRIORITY_INFO = 3;
export const PRIORITY_SUCCESS = 1;

const DEFAULT_PRIORITY_BY_TYPE: Record<string, number> = {
  error: PRIORITY_ERROR,
  warning: PRIORITY_WARNING,
  info: PRIORITY_INFO,
  success: PRIORITY_SUCCESS,
};

export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [toasts, setToasts] = useState<ToastMessage[]>([]);
  let toastIdCounter = 0;

  const removeToast = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const addToast = useCallback(
    (toast: ToastInput) => {
      const id = `toast-${++toastIdCounter}`;
      const duration = toast.duration ?? DEFAULT_DURATION;
      const priority = toast.priority ?? DEFAULT_PRIORITY_BY_TYPE[toast.type];
      const timestamp = Date.now();

      const newToast: ToastMessage = {
        ...toast,
        id,
        priority,
        timestamp,
        duration,
      };

      setToasts((prev) => {
        const next = [...prev, newToast];
        // Sort by priority (desc) and timestamp (desc)
        next.sort((a, b) => {
          if (a.priority !== b.priority) {
            return b.priority - a.priority; // Higher priority first
          }
          return b.timestamp - a.timestamp; // Newer first if same priority
        });
        // Keep only the top MAX_TOASTS
        if (next.length > MAX_TOASTS) {
          return next.slice(0, MAX_TOASTS);
        }
        return next;
      });

      // Auto dismiss
      if (duration > 0) {
        setTimeout(() => {
          removeToast(id);
        }, duration);
      }
    },
    [removeToast]
  );

  const success = useCallback(
    (title: string, description?: string) => {
      addToast({ type: 'success', priority: PRIORITY_SUCCESS, title, description });
    },
    [addToast]
  );

  const error = useCallback(
    (title: string, description?: string) => {
      addToast({ type: 'error', priority: PRIORITY_ERROR, title, description });
    },
    [addToast]
  );

  const warning = useCallback(
    (title: string, description?: string) => {
      addToast({ type: 'warning', priority: PRIORITY_WARNING, title, description });
    },
    [addToast]
  );

  const info = useCallback(
    (title: string, description?: string) => {
      addToast({ type: 'info', priority: PRIORITY_INFO, title, description });
    },
    [addToast]
  );

  const value: ToastContextValue = {
    toasts,
    addToast,
    removeToast,
    success,
    error,
    warning,
    info,
  };

  return (
    <ToastContext.Provider value={value}>
      {children}
      {toasts.map((toast) => (
        <Snackbar
          key={toast.id}
          open={true}
          anchorOrigin={{ vertical: 'top', horizontal: 'right' }}
          sx={{
            mt: toasts.indexOf(toast) * 7, // Stack multiple toasts
          }}
        >
          <Alert
            severity={toast.type as AlertColor}
            variant="filled"
            onClose={toast.dismissible !== false ? () => removeToast(toast.id) : undefined}
            sx={{
              minWidth: '300px',
              borderRadius: 2,
              boxShadow: 6,
            }}
          >
            <strong>{toast.title}</strong>
            {toast.description && (
              <div style={{ marginTop: '4px' }}>{toast.description}</div>
            )}
          </Alert>
        </Snackbar>
      ))}
    </ToastContext.Provider>
  );
}

export function useToast() {
  const context = useContext(ToastContext);
  if (!context) {
    throw new Error('useToast must be used within a ToastProvider');
  }
  return context;
}

export function ToastContainer() {
  // Toast container is now handled by ToastProvider
  return null;
}

export default ToastProvider;
