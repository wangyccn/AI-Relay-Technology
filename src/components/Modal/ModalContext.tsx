import { createContext, useContext, useCallback, useState } from 'react';
import { Dialog, DialogTitle, DialogContent, DialogActions, Button, IconButton, Box, Typography, Chip, TextField } from '@mui/material';
import { Close as CloseIcon, ReportProblem, Info, CheckCircle, Error as ErrorIcon } from '@mui/icons-material';

/**
 * 弹窗优先级常量（0-10）
 * 数值越高，优先级越高，显示越靠上
 * 同优先级的弹窗按时间排序（先来先处理）
 */
export const ModalPriority = {
  /** 最低优先级 - 普通提示信息 */
  LOWEST: 0,
  /** 低优先级 - 一般通知 */
  LOW: 2,
  /** 普通优先级 - 默认值 */
  NORMAL: 4,
  /** 成功提示 */
  SUCCESS: 5,
  /** 中等优先级 - 需要关注的信息 */
  MEDIUM: 6,
  /** 警告提示 */
  WARNING: 7,
  /** 高优先级 - 重要信息 */
  HIGH: 8,
  /** 错误提示 */
  ERROR: 9,
  /** 最高优先级 - 需要用户确认的操作 */
  CONFIRM: 10,
} as const;

export type ModalPriorityLevel = typeof ModalPriority[keyof typeof ModalPriority];

/** 根据弹窗类型获取默认优先级 */
const getDefaultPriorityByType = (type: ModalMessage['type']): number => {
  switch (type) {
    case 'confirm': return ModalPriority.CONFIRM;
    case 'prompt': return ModalPriority.CONFIRM;
    case 'error': return ModalPriority.ERROR;
    case 'warning': return ModalPriority.WARNING;
    case 'success': return ModalPriority.SUCCESS;
    case 'normal':
    default: return ModalPriority.NORMAL;
  }
};

/** 获取优先级描述文本 */
const getPriorityLabel = (priority: number): string => {
  if (priority >= 10) return '最高';
  if (priority >= 8) return '高';
  if (priority >= 6) return '中';
  if (priority >= 4) return '普通';
  if (priority >= 2) return '低';
  return '最低';
};

/** 获取优先级对应颜色 */
const getPriorityColor = (priority: number): 'error' | 'warning' | 'info' | 'success' | 'default' => {
  if (priority >= 9) return 'error';
  if (priority >= 7) return 'warning';
  if (priority >= 5) return 'success';
  if (priority >= 3) return 'info';
  return 'default';
};

export interface ModalMessage {
  id: string;
  type: 'confirm' | 'normal' | 'warning' | 'error' | 'success' | 'prompt';
  priority: number; // 0-10，数值越高优先级越高
  timestamp: number;
  title: string;
  content?: string | React.ReactNode;
  confirmText?: string;
  cancelText?: string;
  onConfirm?: () => void | Promise<void>;
  onCancel?: () => void;
  maxWidth?: 'xs' | 'sm' | 'md' | 'lg' | 'xl' | false;
  // Prompt 专用字段
  defaultValue?: string;
  placeholder?: string;
  onSubmit?: (value: string) => void | Promise<void>;
}

export interface ModalContextValue {
  modals: ModalMessage[];
  showConfirm: (title: string, content?: string, options?: ConfirmOptions) => Promise<boolean>;
  showPrompt: (title: string, content?: string, options?: PromptOptions) => Promise<string | null>;
  showModal: (title: string, content?: string | React.ReactNode, options?: ModalOptions) => void;
  showWarning: (title: string, content?: string | React.ReactNode, options?: Omit<ModalOptions, 'type'>) => void;
  showError: (title: string, content?: string | React.ReactNode, options?: Omit<ModalOptions, 'type'>) => void;
  showSuccess: (title: string, content?: string | React.ReactNode, options?: Omit<ModalOptions, 'type'>) => void;
  showInfo: (title: string, content?: string | React.ReactNode, options?: Omit<ModalOptions, 'type'>) => void;
  removeModal: (id: string) => void;
  clearAllModals: () => void;
}

export interface ConfirmOptions {
  confirmText?: string;
  cancelText?: string;
  onConfirm?: () => void | Promise<void>;
  onCancel?: () => void;
  maxWidth?: 'xs' | 'sm' | 'md' | 'lg' | 'xl' | false;
  priority?: number; // 默认为 CONFIRM (10)
}

export interface PromptOptions {
  confirmText?: string;
  cancelText?: string;
  defaultValue?: string;
  placeholder?: string;
  maxWidth?: 'xs' | 'sm' | 'md' | 'lg' | 'xl' | false;
  priority?: number; // 默认为 CONFIRM (10)
}

export interface ModalOptions {
  priority?: number; // 不指定则根据 type 自动设置
  type?: 'normal' | 'warning' | 'error' | 'success';
  confirmText?: string;
  onConfirm?: () => void | Promise<void>;
  onCancel?: () => void;
  maxWidth?: 'xs' | 'sm' | 'md' | 'lg' | 'xl' | false;
}

const ModalContext = createContext<ModalContextValue | null>(null);

// Modal icons mapping
const modalIcons: Record<string, React.ComponentType> = {
  warning: ReportProblem,
  error: ErrorIcon,
  success: CheckCircle,
  info: Info,
  confirm: ReportProblem,
  normal: Info,
  prompt: Info,
};

// Prompt Dialog Component - handles input state internally
function PromptDialogContent({
  modal,
  onSubmit,
  onCancel,
}: {
  modal: ModalMessage;
  onSubmit: (value: string) => void;
  onCancel: () => void;
}) {
  const [inputValue, setInputValue] = useState(modal.defaultValue || '');

  const handleSubmit = () => {
    if (modal.onSubmit) {
      modal.onSubmit(inputValue);
    }
    onSubmit(inputValue);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      handleSubmit();
    }
  };

  return (
    <>
      <DialogContent sx={{ pt: 1 }}>
        {modal.content && (
          <Typography variant="body1" color="text.secondary" sx={{ mb: 2 }}>
            {modal.content}
          </Typography>
        )}
        <TextField
          autoFocus
          fullWidth
          value={inputValue}
          onChange={(e) => setInputValue(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={modal.placeholder}
          variant="outlined"
          size="small"
        />
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button onClick={onCancel} color="inherit">
          {modal.cancelText || '取消'}
        </Button>
        <Button onClick={handleSubmit} variant="contained" color="primary">
          {modal.confirmText || '确认'}
        </Button>
      </DialogActions>
    </>
  );
}

export function ModalProvider({ children }: { children: React.ReactNode }) {
  const [modals, setModals] = useState<ModalMessage[]>([]);
  let modalIdCounter = 0;

  /**
   * 弹窗排序规则：
   * 1. 优先级高的排在前面（降序）
   * 2. 同优先级的按时间排序：先来先处理（升序）
   */
  const sortedModals = [...modals].sort((a, b) => {
    if (a.priority !== b.priority) {
      return b.priority - a.priority; // 高优先级在前
    }
    return a.timestamp - b.timestamp; // 同优先级：先来先处理
  });

  // Get the top modal to display
  const activeModal = sortedModals.length > 0 ? sortedModals[0] : null;

  const addModal = useCallback(
    (modal: Omit<ModalMessage, 'id' | 'timestamp'>) => {
      const id = `modal-${++modalIdCounter}`;
      const timestamp = Date.now();

      const newModal: ModalMessage = {
        ...modal,
        id,
        timestamp,
      };

      setModals((prev) => [...prev, newModal]);
      return id;
    },
    []
  );

  const removeModal = useCallback((id: string) => {
    setModals((prev) => prev.filter((m) => m.id !== id));
  }, []);

  const clearAllModals = useCallback(() => {
    setModals([]);
  }, []);

  // Show a confirm dialog with highest priority (default: CONFIRM = 10)
  const showConfirm = useCallback(
    (title: string, content?: string, options?: ConfirmOptions): Promise<boolean> => {
      return new Promise<boolean>((resolve) => {
        const id = addModal({
          type: 'confirm',
          priority: options?.priority ?? ModalPriority.CONFIRM,
          title,
          content,
          confirmText: options?.confirmText || '确认',
          cancelText: options?.cancelText || '取消',
          maxWidth: options?.maxWidth || 'sm',
          onConfirm: async () => {
            if (options?.onConfirm) {
              await options.onConfirm();
            }
            removeModal(id);
            resolve(true);
          },
          onCancel: () => {
            if (options?.onCancel) {
              options.onCancel();
            }
            removeModal(id);
            resolve(false);
          },
        });
      });
    },
    [addModal, removeModal]
  );

  // Show a modal with configurable priority (auto-detect by type if not specified)
  const showModal = useCallback(
    (title: string, content?: string | React.ReactNode, options?: ModalOptions) => {
      const type = options?.type || 'normal';
      const priority = options?.priority ?? getDefaultPriorityByType(type);

      addModal({
        type,
        priority,
        title,
        content,
        confirmText: options?.confirmText,
        maxWidth: options?.maxWidth || 'md',
        onConfirm: options?.onConfirm,
        onCancel: options?.onCancel,
      });
    },
    [addModal]
  );

  // Convenience method: show warning modal (priority: WARNING = 7)
  const showWarning = useCallback(
    (title: string, content?: string | React.ReactNode, options?: Omit<ModalOptions, 'type'>) => {
      showModal(title, content, { ...options, type: 'warning' });
    },
    [showModal]
  );

  // Convenience method: show error modal (priority: ERROR = 9)
  const showError = useCallback(
    (title: string, content?: string | React.ReactNode, options?: Omit<ModalOptions, 'type'>) => {
      showModal(title, content, { ...options, type: 'error' });
    },
    [showModal]
  );

  // Convenience method: show success modal (priority: SUCCESS = 5)
  const showSuccess = useCallback(
    (title: string, content?: string | React.ReactNode, options?: Omit<ModalOptions, 'type'>) => {
      showModal(title, content, { ...options, type: 'success' });
    },
    [showModal]
  );

  // Convenience method: show info modal (priority: NORMAL = 4)
  const showInfo = useCallback(
    (title: string, content?: string | React.ReactNode, options?: Omit<ModalOptions, 'type'>) => {
      showModal(title, content, { ...options, type: 'normal' });
    },
    [showModal]
  );

  // Show a prompt dialog (input dialog) with highest priority (default: CONFIRM = 10)
  const showPrompt = useCallback(
    (title: string, content?: string, options?: PromptOptions): Promise<string | null> => {
      return new Promise<string | null>((resolve) => {
        const id = addModal({
          type: 'prompt',
          priority: options?.priority ?? ModalPriority.CONFIRM,
          title,
          content,
          confirmText: options?.confirmText || '确认',
          cancelText: options?.cancelText || '取消',
          maxWidth: options?.maxWidth || 'sm',
          defaultValue: options?.defaultValue,
          placeholder: options?.placeholder,
          onSubmit: (value: string) => {
            removeModal(id);
            resolve(value);
          },
          onCancel: () => {
            removeModal(id);
            resolve(null);
          },
        });
      });
    },
    [addModal, removeModal]
  );

  // Handle modal close
  const handleModalClose = useCallback(
    (modal: ModalMessage) => {
      if (modal.onCancel) {
        modal.onCancel();
      }
      removeModal(modal.id);
    },
    [removeModal]
  );

  // Handle modal confirm
  const handleModalConfirm = useCallback(
    async (modal: ModalMessage) => {
      if (modal.onConfirm) {
        await modal.onConfirm();
      }
      removeModal(modal.id);
    },
    [removeModal]
  );

  const value: ModalContextValue = {
    modals,
    showConfirm,
    showPrompt,
    showModal,
    showWarning,
    showError,
    showSuccess,
    showInfo,
    removeModal,
    clearAllModals,
  };

  return (
    <ModalContext.Provider value={value}>
      {children}

      {/* Render only the active modal (highest priority) */}
      {activeModal && (
        <Dialog
          open={true}
          onClose={() => handleModalClose(activeModal)}
          maxWidth={activeModal.maxWidth || 'md'}
          fullWidth={true}
          sx={{
            '& .MuiDialog-paper': {
              borderRadius: 2,
              boxShadow: 24,
            },
          }}
        >
          <DialogTitle
            sx={{
              display: 'flex',
              alignItems: 'center',
              gap: 1,
              pb: 2,
            }}
          >
            {activeModal.type !== 'normal' && (
              <Box
                sx={{
                  color:
                    activeModal.type === 'error'
                      ? 'error.main'
                      : activeModal.type === 'warning' || activeModal.type === 'confirm'
                        ? 'warning.main'
                        : activeModal.type === 'success'
                          ? 'success.main'
                          : 'info.main',
                }}
              >
                {(() => {
                  const IconComponent = modalIcons[activeModal.type] || Info;
                  return <IconComponent />;
                })()}
              </Box>
            )}
            <Typography variant="h6" component="span" sx={{ flex: 1 }}>
              {activeModal.title}
            </Typography>
            <IconButton
              aria-label="关闭"
              onClick={() => handleModalClose(activeModal)}
              sx={{ ml: 'auto' }}
            >
              <CloseIcon />
            </IconButton>
          </DialogTitle>

          {/* Prompt 类型弹窗使用专门的组件 */}
          {activeModal.type === 'prompt' ? (
            <PromptDialogContent
              modal={activeModal}
              onSubmit={() => {}}
              onCancel={() => handleModalClose(activeModal)}
            />
          ) : (
            <>
              <DialogContent sx={{ pt: 1 }}>
                {typeof activeModal.content === 'string' ? (
                  <Typography variant="body1" color="text.secondary">
                    {activeModal.content}
                  </Typography>
                ) : (
                  activeModal.content
                )}
              </DialogContent>

              <DialogActions sx={{ px: 3, pb: 2 }}>
                {activeModal.type === 'confirm' ? (
                  <>
                    <Button
                      onClick={() => handleModalClose(activeModal)}
                      color="inherit"
                    >
                      {activeModal.cancelText || '取消'}
                    </Button>
                    <Button
                      onClick={() => handleModalConfirm(activeModal)}
                      variant="contained"
                      color="primary"
                      autoFocus
                    >
                      {activeModal.confirmText || '确认'}
                    </Button>
                  </>
                ) : (
                  activeModal.onConfirm && (
                    <Button
                      onClick={() => handleModalConfirm(activeModal)}
                      variant="contained"
                      color="primary"
                      autoFocus
                    >
                      {activeModal.confirmText || '确定'}
                    </Button>
                  )
                )}
              </DialogActions>
            </>
          )}
        </Dialog>
      )}

      {/* Show indicator if there are multiple pending modals */}
      {sortedModals.length > 1 && (
        <Box
          sx={{
            position: 'fixed',
            bottom: 16,
            left: '50%',
            transform: 'translateX(-50%)',
            bgcolor: 'background.paper',
            boxShadow: 6,
            px: 3,
            py: 1.5,
            borderRadius: 2,
            display: 'flex',
            alignItems: 'center',
            gap: 2,
            zIndex: (theme) => theme.zIndex.modal - 1,
          }}
        >
          <Typography variant="body2" color="text.secondary">
            还有 {sortedModals.length - 1} 个弹窗等待处理
          </Typography>
          {/* 显示等待队列中最高优先级的弹窗信息 */}
          {sortedModals[1] && (
            <Chip
              size="small"
              label={`下一个: ${getPriorityLabel(sortedModals[1].priority)}优先级`}
              color={getPriorityColor(sortedModals[1].priority)}
              variant="outlined"
            />
          )}
        </Box>
      )}
    </ModalContext.Provider>
  );
}

export function useModal() {
  const context = useContext(ModalContext);
  if (!context) {
    throw new Error('useModal must be used within a ModalProvider');
  }
  return context;
}

export default ModalProvider;
