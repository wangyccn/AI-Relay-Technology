import { Chip, SxProps } from '@mui/material';
import { Circle } from '@mui/icons-material';

export type StatusType = 'online' | 'offline' | 'pending' | 'error' | 'warning' | 'idle';

export interface StatusBadgeProps {
  status: StatusType;
  label?: string;
  showDot?: boolean;
  pulse?: boolean;
  sx?: SxProps;
}

const statusConfig: Record<
  StatusType,
  { label: string; color: 'success' | 'default' | 'warning' | 'error' | 'info' }
> = {
  online: { label: '在线', color: 'success' },
  offline: { label: '离线', color: 'default' },
  pending: { label: '等待中', color: 'warning' },
  error: { label: '错误', color: 'error' },
  warning: { label: '警告', color: 'warning' },
  idle: { label: '空闲', color: 'info' },
};

export function StatusBadge({
  status,
  label,
  showDot = true,
  pulse = false,
  sx,
}: StatusBadgeProps) {
  const config = statusConfig[status];
  const displayLabel = label || config.label;

  // Material Design 3 风格样式
  const customSx: SxProps = {
    borderRadius: 2,
    fontWeight: 500,
    '& .MuiChip-label': {
      display: 'flex',
      alignItems: 'center',
      gap: 0.5,
    },
    ...sx,
  };

  const dotSx: SxProps = {
    fontSize: 8,
    ...(pulse && {
      animation: 'pulse 2s ease-in-out infinite',
      '@keyframes pulse': {
        '0%, 100%': {
          opacity: 1,
        },
        '50%': {
          opacity: 0.5,
        },
      },
    }),
  };

  return (
    <Chip
      label={
        <>
          {showDot && <Circle sx={dotSx} />}
          {displayLabel}
        </>
      }
      color={config.color}
      size="small"
      sx={customSx}
    />
  );
}

export default StatusBadge;
