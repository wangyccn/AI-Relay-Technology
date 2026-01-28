import { forwardRef } from 'react';
import { Button as MuiButton, ButtonProps as MuiButtonProps } from '@mui/material';
import { LoadingButton } from '@mui/lab';

export interface ButtonProps extends Omit<MuiButtonProps, 'variant'> {
  variant?: 'primary' | 'secondary' | 'danger' | 'success' | 'ghost' | 'contained' | 'outlined' | 'text';
  loading?: boolean;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  (
    {
      variant = 'primary',
      loading = false,
      children,
      ...props
    },
    ref
  ) => {
    const getMuiVariant = (): 'contained' | 'outlined' | 'text' => {
      if (variant === 'ghost' || variant === 'text') return 'text';
      if (variant === 'secondary') return 'outlined';
      return 'contained';
    };

    const getMuiColor = (): 'primary' | 'secondary' | 'error' | 'success' | 'info' => {
      if (variant === 'danger') return 'error';
      if (variant === 'success') return 'success';
      if (variant === 'secondary') return 'secondary';
      return 'primary';
    };

    const muiVariant = getMuiVariant();
    const muiColor = getMuiColor();

    if (loading) {
      return (
        <LoadingButton
          ref={ref}
          variant={muiVariant}
          color={muiColor}
          loading={loading}
          {...props}
        >
          {children}
        </LoadingButton>
      );
    }

    return (
      <MuiButton
        ref={ref}
        variant={muiVariant}
        color={muiColor}
        {...props}
      >
        {children}
      </MuiButton>
    );
  }
);

Button.displayName = 'Button';

export default Button;
