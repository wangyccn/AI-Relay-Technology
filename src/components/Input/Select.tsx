import { forwardRef } from 'react';
import {
  Select as MuiSelect,
  MenuItem,
  FormControl,
  InputLabel,
  FormHelperText,
} from '@mui/material';

export interface SelectOption {
  value: string | number;
  label: string;
  disabled?: boolean;
}

export interface SelectProps {
  label?: string;
  options: SelectOption[];
  value: string | number;
  onChange: (value: string | number) => void;
  error?: boolean;
  helperText?: string;
  fullWidth?: boolean;
  placeholder?: string;
}

export const Select = forwardRef<HTMLDivElement, SelectProps>(
  (
    {
      label,
      options,
      error = false,
      helperText,
      fullWidth = true,
      placeholder,
      value,
      onChange,
    },
    ref
  ) => {
    return (
      <FormControl
        ref={ref}
        fullWidth={fullWidth}
        size="small"
        error={error}
      >
        {label && <InputLabel>{label}</InputLabel>}
        <MuiSelect
          value={value}
          label={label}
          onChange={(e) => onChange(e.target.value as string | number)}
        >
          {placeholder && (
            <MenuItem value="" disabled>
              <em>{placeholder}</em>
            </MenuItem>
          )}
          {options.map((option) => (
            <MenuItem
              key={option.value}
              value={option.value}
              disabled={option.disabled}
            >
              {option.label}
            </MenuItem>
          ))}
        </MuiSelect>
        {helperText && <FormHelperText>{helperText}</FormHelperText>}
      </FormControl>
    );
  }
);

Select.displayName = 'Select';

export default Select;
