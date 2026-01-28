import { Chip, ChipProps } from '@mui/material';

export interface BadgeProps extends Omit<ChipProps, 'color' | 'variant'> {
  label: React.ReactNode;
  variant?: 'default' | 'primary' | 'success' | 'warning' | 'error' | 'info';
  size?: 'small' | 'medium';
  rounded?: boolean;
}

export function Badge({
  label,
  variant = 'default',
  size = 'small',
  rounded = true,
  sx,
  ...props
}: BadgeProps) {
  // 映射自定义 variant 到 MUI color
  const getMuiColor = (): 'default' | 'primary' | 'success' | 'warning' | 'error' | 'info' => {
    if (variant === 'default') return 'default';
    return variant;
  };

  // Material Design 3 风格样式
  const customSx = {
    borderRadius: rounded ? 3 : 1,
    fontWeight: 500,
    fontSize: size === 'small' ? '0.75rem' : '0.8125rem',
    height: size === 'small' ? 24 : 32,
    ...sx,
  };

  return (
    <Chip
      label={label}
      color={getMuiColor()}
      size={size}
      sx={customSx}
      {...props}
    />
  );
}

export default Badge;
