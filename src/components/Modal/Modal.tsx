import {
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  IconButton,
} from '@mui/material';
import { Close as CloseIcon } from '@mui/icons-material';

export interface ModalProps {
  open: boolean;
  onClose: () => void;
  title?: string;
  maxWidth?: 'xs' | 'sm' | 'md' | 'lg' | 'xl' | false;
  fullWidth?: boolean;
  children: React.ReactNode;
  footer?: React.ReactNode;
  size?: 'xs' | 'sm' | 'md' | 'lg' | 'xl';
}

export function Modal({
  open,
  onClose,
  title,
  maxWidth,
  fullWidth = true,
  children,
  footer,
  size = 'md',
}: ModalProps) {
  return (
    <Dialog
      open={open}
      onClose={onClose}
      maxWidth={maxWidth || size}
      fullWidth={fullWidth}
    >
      {title && (
        <DialogTitle>
          {title}
          <IconButton
            aria-label="关闭"
            onClick={onClose}
            sx={{ position: 'absolute', right: 8, top: 8 }}
          >
            <CloseIcon />
          </IconButton>
        </DialogTitle>
      )}

      <DialogContent>{children}</DialogContent>

      {footer && <DialogActions>{footer}</DialogActions>}
    </Dialog>
  );
}

export default Modal;
