import { alpha, createTheme, Theme } from '@mui/material/styles';
import type { MaterialTheme } from './presets';
import { lightThemes } from './presets';

const shadows = [
  'none',
  '0 2px 4px rgba(17, 24, 39, 0.06), 0 1px 2px rgba(17, 24, 39, 0.04)',
  '0 3px 6px rgba(17, 24, 39, 0.07), 0 1px 2px rgba(17, 24, 39, 0.05)',
  '0 5px 10px rgba(17, 24, 39, 0.08), 0 2px 4px rgba(17, 24, 39, 0.05)',
  '0 6px 13px rgba(17, 24, 39, 0.09), 0 2px 5px rgba(17, 24, 39, 0.06)',
  '0 8px 16px rgba(17, 24, 39, 0.10), 0 3px 6px rgba(17, 24, 39, 0.06)',
  '0 10px 20px rgba(17, 24, 39, 0.11), 0 3px 7px rgba(17, 24, 39, 0.07)',
  '0 11px 22px rgba(17, 24, 39, 0.12), 0 4px 8px rgba(17, 24, 39, 0.07)',
  '0 13px 25px rgba(17, 24, 39, 0.13), 0 4px 9px rgba(17, 24, 39, 0.08)',
  '0 14px 28px rgba(17, 24, 39, 0.14), 0 5px 10px rgba(17, 24, 39, 0.08)',
  '0 16px 31px rgba(17, 24, 39, 0.15), 0 5px 11px rgba(17, 24, 39, 0.09)',
  '0 18px 35px rgba(17, 24, 39, 0.16), 0 6px 13px rgba(17, 24, 39, 0.09)',
  '0 19px 38px rgba(17, 24, 39, 0.17), 0 6px 14px rgba(17, 24, 39, 0.10)',
  '0 21px 42px rgba(17, 24, 39, 0.18), 0 7px 15px rgba(17, 24, 39, 0.10)',
  '0 22px 44px rgba(17, 24, 39, 0.19), 0 7px 16px rgba(17, 24, 39, 0.11)',
  '0 24px 48px rgba(17, 24, 39, 0.20), 0 8px 17px rgba(17, 24, 39, 0.11)',
  '0 26px 51px rgba(17, 24, 39, 0.21), 0 9px 18px rgba(17, 24, 39, 0.12)',
  '0 27px 54px rgba(17, 24, 39, 0.22), 0 9px 19px rgba(17, 24, 39, 0.12)',
  '0 29px 57px rgba(17, 24, 39, 0.23), 0 10px 20px rgba(17, 24, 39, 0.13)',
  '0 30px 60px rgba(17, 24, 39, 0.24), 0 10px 21px rgba(17, 24, 39, 0.13)',
  '0 32px 63px rgba(17, 24, 39, 0.24), 0 11px 22px rgba(17, 24, 39, 0.14)',
  '0 34px 66px rgba(17, 24, 39, 0.24), 0 11px 23px rgba(17, 24, 39, 0.14)',
  '0 35px 70px rgba(17, 24, 39, 0.24), 0 12px 24px rgba(17, 24, 39, 0.15)',
  '0 37px 73px rgba(17, 24, 39, 0.24), 0 12px 25px rgba(17, 24, 39, 0.15)',
  '0 38px 76px rgba(17, 24, 39, 0.24), 0 13px 26px rgba(17, 24, 39, 0.15)',
] as const;

export const buildMuiTheme = (preset: MaterialTheme, isDark: boolean): Theme => {
  const colors = preset.colors;

  const primary = {
    main: colors.primary,
    light: colors.primaryContainer,
    dark: colors.onPrimaryContainer,
    contrastText: colors.onPrimary,
  };

  const secondary = {
    main: colors.secondary,
    light: colors.secondaryContainer,
    dark: colors.onSecondaryContainer,
    contrastText: colors.onSecondary,
  };

  const info = {
    main: colors.tertiary,
    light: colors.tertiaryContainer,
    dark: colors.onTertiaryContainer,
    contrastText: colors.onTertiary,
  };

  const success = {
    main: isDark ? '#3ddc97' : '#1b9c5c',
    light: isDark ? '#65e3b2' : '#42c78a',
    dark: isDark ? '#249668' : '#0e6b3b',
    contrastText: '#ffffff',
  };

  const warning = {
    main: isDark ? '#f0b458' : '#e09f3e',
    light: isDark ? '#f6cd86' : '#f4c06a',
    dark: isDark ? '#b57b24' : '#b8781b',
    contrastText: isDark ? '#1b1c18' : '#ffffff',
  };

  const error = {
    main: colors.error,
    light: colors.errorContainer,
    dark: colors.onErrorContainer,
    contrastText: colors.onError,
  };

  return createTheme({
  palette: {
    mode: isDark ? 'dark' : 'light',
    primary,
    secondary,
    error,
    warning,
    success,
    info,
    background: {
      default: colors.surface,
      paper: colors.surfaceContainer,
    },
    text: {
      primary: colors.onSurface,
      secondary: colors.onSurfaceVariant,
      disabled: alpha(colors.onSurface, 0.45),
    },
    divider: colors.outlineVariant,
    action: {
      hover: alpha(primary.main, 0.06),
      selected: alpha(primary.main, 0.12),
      focus: alpha(primary.main, 0.12),
      disabledBackground: alpha(colors.onSurface, 0.08),
    },
  },
  typography: {
    fontFamily: '"Plus Jakarta Sans", "Noto Sans SC", "Segoe UI", sans-serif',
    fontSize: 14,
    h1: { fontSize: '2.125rem', fontWeight: 700, lineHeight: 1.2 },
    h2: { fontSize: '1.75rem', fontWeight: 700, lineHeight: 1.25 },
    h3: { fontSize: '1.5rem', fontWeight: 700, lineHeight: 1.3 },
    h4: { fontSize: '1.25rem', fontWeight: 700 },
    h5: { fontSize: '1.125rem', fontWeight: 700 },
    h6: { fontSize: '1rem', fontWeight: 700 },
    body1: { fontSize: '1rem', lineHeight: 1.5 },
    body2: { fontSize: '0.875rem', lineHeight: 1.5 },
    button: { fontSize: '0.875rem', fontWeight: 600, textTransform: 'none' },
    caption: { fontSize: '0.75rem', color: colors.onSurfaceVariant },
  },
  shape: {
    borderRadius: 16,
  },
  shadows: shadows as any,
  components: {
    MuiButton: {
      defaultProps: {
        disableElevation: true,
      },
      styleOverrides: {
        root: {
          borderRadius: 999,
          textTransform: 'none',
          fontWeight: 600,
          padding: '8px 18px',
          transition: 'transform 150ms ease, box-shadow 150ms ease, background 150ms ease',
        },
        contained: {
          boxShadow: '0 2px 8px rgba(15, 23, 42, 0.16)',
          '&:hover': {
            boxShadow: '0 6px 14px rgba(15, 23, 42, 0.2)',
            transform: 'translateY(-1px)',
          },
        },
        outlined: {
          borderWidth: 1,
          borderColor: alpha(primary.main, 0.4),
          '&:hover': {
            borderColor: primary.main,
            backgroundColor: alpha(primary.main, 0.08),
          },
        },
        sizeSmall: { padding: '6px 14px', fontSize: '0.8125rem' },
        sizeLarge: { padding: '10px 22px', fontSize: '0.9375rem' },
      },
    },
    MuiTextField: {
      defaultProps: { variant: 'outlined', size: 'small' },
    },
    MuiOutlinedInput: {
      styleOverrides: {
        root: {
          borderRadius: 12,
          backgroundColor: colors.surfaceContainerLow,
          transition: 'box-shadow 150ms ease, border-color 150ms ease',
          '& .MuiOutlinedInput-notchedOutline': {
            borderColor: colors.outline,
          },
          '&:hover .MuiOutlinedInput-notchedOutline': {
            borderColor: primary.main,
          },
          '&.Mui-focused': {
            boxShadow: `0 0 0 3px ${alpha(primary.main, 0.15)}`,
          },
          '&.Mui-focused .MuiOutlinedInput-notchedOutline': {
            borderColor: primary.main,
            borderWidth: 2,
          },
        },
        input: {
          padding: '12px 14px',
        },
        inputSizeSmall: {
          padding: '8px 12px',
        },
      },
    },
    MuiInputLabel: {
      styleOverrides: {
        root: {
          fontWeight: 600,
          color: colors.onSurfaceVariant,
        },
      },
    },
    MuiCard: {
      styleOverrides: {
        root: {
          borderRadius: 18,
          border: `1px solid ${colors.outlineVariant}`,
          boxShadow: '0 4px 12px rgba(17, 24, 39, 0.08)',
          transition: 'transform 200ms ease, box-shadow 200ms ease',
          overflow: 'hidden',
          '&:hover': {
            boxShadow: '0 10px 24px rgba(17, 24, 39, 0.12)',
            transform: 'translateY(-2px)',
          },
        },
      },
    },
    MuiAppBar: {
      styleOverrides: {
        root: {
          boxShadow: '0 1px 2px rgba(17, 24, 39, 0.06)',
          backgroundColor: alpha(colors.surfaceContainer, isDark ? 0.9 : 0.92),
          color: colors.onSurface,
          backdropFilter: 'blur(12px)',
        },
      },
    },
    MuiDrawer: {
      styleOverrides: {
        paper: {
          borderRight: `1px solid ${colors.outlineVariant}`,
          backgroundColor: colors.surfaceContainer,
        },
      },
    },
    MuiChip: {
      styleOverrides: {
        root: { borderRadius: 8, fontWeight: 600 },
        sizeSmall: { height: 24, fontSize: '0.75rem' },
        sizeMedium: { height: 32, fontSize: '0.8125rem' },
      },
    },
    MuiTableHead: {
      styleOverrides: {
        root: { backgroundColor: colors.surfaceContainerHigh },
      },
    },
    MuiTableCell: {
      styleOverrides: {
        head: {
          fontWeight: 700,
          fontSize: '0.75rem',
          textTransform: 'uppercase',
          letterSpacing: '0.05em',
          color: colors.onSurfaceVariant,
          borderColor: colors.outlineVariant,
        },
        body: { borderColor: colors.outlineVariant },
      },
    },
    MuiDialog: {
      styleOverrides: {
        paper: {
          borderRadius: 20,
          boxShadow: '0 24px 48px rgba(17, 24, 39, 0.2)',
        },
      },
    },
    MuiDialogTitle: {
      styleOverrides: {
        root: {
          padding: '24px 24px 16px',
          fontSize: '1.25rem',
          fontWeight: 700,
        },
      },
    },
    MuiDialogContent: {
      styleOverrides: {
        root: { padding: '16px 24px' },
      },
    },
    MuiDialogActions: {
      styleOverrides: {
        root: {
          padding: '16px 24px 24px',
          gap: 8,
        },
      },
    },
    MuiSnackbar: {
      styleOverrides: {
        root: {
          '& .MuiSnackbarContent-root': {
            borderRadius: 12,
            boxShadow: '0 6px 16px rgba(17, 24, 39, 0.18)',
          },
        },
      },
    },
    MuiAlert: {
      styleOverrides: {
        root: { borderRadius: 12 },
        filled: { fontWeight: 600 },
      },
    },
    MuiSelect: {
      defaultProps: { variant: 'outlined', size: 'small' },
      styleOverrides: {
        icon: {
          color: colors.onSurfaceVariant,
        },
      },
    },
    MuiIconButton: {
      styleOverrides: {
        root: {
          borderRadius: 12,
          transition: 'all 150ms ease-in-out',
          '&:hover': { backgroundColor: alpha(primary.main, 0.08) },
        },
      },
    },
    MuiTooltip: {
      styleOverrides: {
        tooltip: {
          backgroundColor: 'rgba(20, 24, 28, 0.92)',
          borderRadius: 8,
          fontSize: '0.75rem',
          padding: '6px 12px',
        },
      },
    },
    MuiPaper: {
      styleOverrides: {
        root: { backgroundImage: 'none' },
        elevation1: { boxShadow: '0 2px 8px rgba(17, 24, 39, 0.08)' },
        elevation2: { boxShadow: '0 6px 16px rgba(17, 24, 39, 0.12)' },
      },
    },
    MuiDivider: {
      styleOverrides: {
        root: { borderColor: colors.outlineVariant },
      },
    },
    MuiListItem: {
      styleOverrides: {
        root: {
          borderRadius: 12,
          margin: '2px 8px',
        },
      },
    },
    MuiListItemButton: {
      styleOverrides: {
        root: {
          borderRadius: 12,
          transition: 'all 150ms ease-in-out',
          '&:hover': {
            backgroundColor: alpha(primary.main, 0.06),
          },
        },
      },
    },
  },
  transitions: {
    duration: {
      shortest: 150,
      shorter: 200,
      short: 250,
      standard: 300,
      complex: 375,
      enteringScreen: 225,
      leavingScreen: 195,
    },
    easing: {
      easeInOut: 'cubic-bezier(0.4, 0, 0.2, 1)',
      easeOut: 'cubic-bezier(0.0, 0, 0.2, 1)',
      easeIn: 'cubic-bezier(0.4, 0, 1, 1)',
      sharp: 'cubic-bezier(0.4, 0, 0.6, 1)',
    },
  },
  zIndex: {
    mobileStepper: 1000,
    speedDial: 1050,
    appBar: 1100,
    drawer: 1200,
    modal: 1300,
    snackbar: 1400,
    tooltip: 1500,
  },
  breakpoints: {
    values: {
      xs: 0,
      sm: 600,
      md: 900,
      lg: 1200,
      xl: 1536,
    },
  },
  });
};

export const lightTheme: Theme = buildMuiTheme(lightThemes.default, false);

export default lightTheme;
