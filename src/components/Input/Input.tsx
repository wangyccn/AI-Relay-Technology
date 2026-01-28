import { forwardRef } from 'react';
import { TextField, TextFieldProps } from '@mui/material';

export interface InputProps extends Omit<TextFieldProps, 'variant'> {
  label?: string;
  error?: boolean;
  helperText?: string;
  inputPrefix?: React.ReactNode;
  inputSuffix?: React.ReactNode;
  fullWidth?: boolean;
}

export const Input = forwardRef<HTMLInputElement, InputProps>(
  (
    {
      label,
      error = false,
      helperText,
      fullWidth = true,
      size = 'small',
      InputProps,
      ...props
    },
    ref
  ) => {
    return (
      <TextField
        ref={ref}
        label={label}
        error={error}
        helperText={helperText}
        fullWidth={fullWidth}
        size={size}
        variant="outlined"
        InputProps={InputProps}
        {...props}
      />
    );
  }
);

Input.displayName = 'Input';

export default Input;
