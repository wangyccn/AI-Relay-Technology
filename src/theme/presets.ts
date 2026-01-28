/**
 * Material Design 主题预设
 * 每个主题都包含完整的颜色配置
 */

export interface MaterialTheme {
  name: string;
  description: string;
  colors: {
    primary: string;
    onPrimary: string;
    primaryContainer: string;
    onPrimaryContainer: string;
    secondary: string;
    onSecondary: string;
    secondaryContainer: string;
    onSecondaryContainer: string;
    tertiary: string;
    onTertiary: string;
    tertiaryContainer: string;
    onTertiaryContainer: string;
    error: string;
    onError: string;
    errorContainer: string;
    onErrorContainer: string;
    surface: string;
    onSurface: string;
    surfaceVariant: string;
    onSurfaceVariant: string;
    outline: string;
    outlineVariant: string;
    surfaceContainer: string;
    surfaceContainerHigh: string;
    surfaceContainerLow: string;
  };
}

// 亮色主题预设
export const lightThemes: Record<string, MaterialTheme> = {
  default: {
    name: '默认亮色',
    description: '经典的青色与橙色配色',
    colors: {
      primary: '#0f6f63',
      onPrimary: '#ffffff',
      primaryContainer: '#a8f2e2',
      onPrimaryContainer: '#00201c',
      secondary: '#e68a2e',
      onSecondary: '#ffffff',
      secondaryContainer: '#ffe1c2',
      onSecondaryContainer: '#3a2200',
      tertiary: '#3b6d8c',
      onTertiary: '#ffffff',
      tertiaryContainer: '#cfe6ff',
      onTertiaryContainer: '#001f2b',
      error: '#ba1a1a',
      onError: '#ffffff',
      errorContainer: '#ffdad6',
      onErrorContainer: '#410002',
      surface: '#f9f8f4',
      onSurface: '#1b1c18',
      surfaceVariant: '#ebe6df',
      onSurfaceVariant: '#4a4d46',
      outline: '#c7c9c1',
      outlineVariant: '#d9ddd5',
      surfaceContainer: '#ffffff',
      surfaceContainerHigh: '#f2efe9',
      surfaceContainerLow: '#ffffff',
    },
  },
  ocean: {
    name: '海洋蓝',
    description: '清新的蓝色系主题',
    colors: {
      primary: '#0061a4',
      onPrimary: '#ffffff',
      primaryContainer: '#d1e4ff',
      onPrimaryContainer: '#001d36',
      secondary: '#4c616b',
      onSecondary: '#ffffff',
      secondaryContainer: '#cfe6f1',
      onSecondaryContainer: '#081e28',
      tertiary: '#5e5b7e',
      onTertiary: '#ffffff',
      tertiaryContainer: '#e3dfff',
      onTertiaryContainer: '#1a1836',
      error: '#ba1a1a',
      onError: '#ffffff',
      errorContainer: '#ffdad6',
      onErrorContainer: '#410002',
      surface: '#fdfcff',
      onSurface: '#1a1c1e',
      surfaceVariant: '#dfe3eb',
      onSurfaceVariant: '#43474e',
      outline: '#74777f',
      outlineVariant: '#c4c8cf',
      surfaceContainer: '#ffffff',
      surfaceContainerHigh: '#f0f4f9',
      surfaceContainerLow: '#ffffff',
    },
  },
  forest: {
    name: '森林绿',
    description: '自然的绿色系主题',
    colors: {
      primary: '#3e6b38',
      onPrimary: '#ffffff',
      primaryContainer: '#c1f1b6',
      onPrimaryContainer: '#072200',
      secondary: '#5a6149',
      onSecondary: '#ffffff',
      secondaryContainer: '#dee6c8',
      onSecondaryContainer: '#171e0b',
      tertiary: '#386668',
      onTertiary: '#ffffff',
      tertiaryContainer: '#bcebeb',
      onTertiaryContainer: '#002021',
      error: '#ba1a1a',
      onError: '#ffffff',
      errorContainer: '#ffdad6',
      onErrorContainer: '#410002',
      surface: '#fcf9f4',
      onSurface: '#1b1c18',
      surfaceVariant: '#e5e3dc',
      onSurfaceVariant: '#48473f',
      outline: '#797872',
      outlineVariant: '#c9c8c1',
      surfaceContainer: '#ffffff',
      surfaceContainerHigh: '#f0f1ea',
      surfaceContainerLow: '#ffffff',
    },
  },
  berry: {
    name: '浆果紫',
    description: '甜美的紫色系主题',
    colors: {
      primary: '#6b4c8e',
      onPrimary: '#ffffff',
      primaryContainer: '#f2daff',
      onPrimaryContainer: '#260645',
      secondary: '#635a70',
      onSecondary: '#ffffff',
      secondaryContainer: '#e9def6',
      onSecondaryContainer: '#1f182b',
      tertiary: '#7d5260',
      onTertiary: '#ffffff',
      tertiaryContainer: '#ffd9e3',
      onTertiaryContainer: '#301119',
      error: '#ba1a1a',
      onError: '#ffffff',
      errorContainer: '#ffdad6',
      onErrorContainer: '#410002',
      surface: '#fbf8fd',
      onSurface: '#1b1b1f',
      surfaceVariant: '#e6e0ec',
      onSurfaceVariant: '#49454e',
      outline: '#7a757f',
      outlineVariant: '#cac4cf',
      surfaceContainer: '#ffffff',
      surfaceContainerHigh: '#f4f3f9',
      surfaceContainerLow: '#ffffff',
    },
  },
  sunset: {
    name: '日落橙',
    description: '温暖的橙色系主题',
    colors: {
      primary: '#b9520a',
      onPrimary: '#ffffff',
      primaryContainer: '#ffccba',
      onPrimaryContainer: '#3d1600',
      secondary: '#6e5c4d',
      onSecondary: '#ffffff',
      secondaryContainer: '#f9e1cf',
      onSecondaryContainer: '#261911',
      tertiary: '#506435',
      onTertiary: '#ffffff',
      tertiaryContainer: '#d3e9b0',
      onTertiaryContainer: '#131f04',
      error: '#ba1a1a',
      onError: '#ffffff',
      errorContainer: '#ffdad6',
      onErrorContainer: '#410002',
      surface: '#fcf9f6',
      onSurface: '#1b1b17',
      surfaceVariant: '#ebe5df',
      onSurfaceVariant: '#4b473e',
      outline: '#7c776d',
      outlineVariant: '#cdc7c0',
      surfaceContainer: '#ffffff',
      surfaceContainerHigh: '#f3f0eb',
      surfaceContainerLow: '#ffffff',
    },
  },
};

// 暗色主题预设
export const darkThemes: Record<string, MaterialTheme> = {
  default: {
    name: '默认暗色',
    description: '经典的青色与橙色暗色配色',
    colors: {
      primary: '#8cd5c6',
      onPrimary: '#003730',
      primaryContainer: '#0f6f63',
      onPrimaryContainer: '#a8f2e2',
      secondary: '#ffc59d',
      onSecondary: '#572e00',
      secondaryContainer: '#e68a2e',
      onSecondaryContainer: '#ffe1c2',
      tertiary: '#b4cae8',
      onTertiary: '#0f344d',
      tertiaryContainer: '#274e65',
      onTertiaryContainer: '#cfe6ff',
      error: '#ffb4ab',
      onError: '#690005',
      errorContainer: '#93000a',
      onErrorContainer: '#ffdad6',
      surface: '#1b1c18',
      onSurface: '#e2e3dd',
      surfaceVariant: '#4a4d46',
      onSurfaceVariant: '#c7c9c1',
      outline: '#74776f',
      outlineVariant: '#4a4d46',
      surfaceContainer: '#2a2b26',
      surfaceContainerHigh: '#31322d',
      surfaceContainerLow: '#22231e',
    },
  },
  midnight: {
    name: '午夜蓝',
    description: '深邃的蓝色暗色主题',
    colors: {
      primary: '#9fcafb',
      onPrimary: '#00325a',
      primaryContainer: '#004a82',
      onPrimaryContainer: '#d1e4ff',
      secondary: '#b9c8d2',
      onSecondary: '#24323d',
      secondaryContainer: '#3a4955',
      onSecondaryContainer: '#cfe6f1',
      tertiary: '#d4cfe8',
      onTertiary: '#302b4a',
      tertiaryContainer: '#474262',
      onTertiaryContainer: '#e3dfff',
      error: '#ffb4ab',
      onError: '#690005',
      errorContainer: '#93000a',
      onErrorContainer: '#ffdad6',
      surface: '#1a1c1e',
      onSurface: '#e2e2e6',
      surfaceVariant: '#43474e',
      onSurfaceVariant: '#c4c8cf',
      outline: '#8e9299',
      outlineVariant: '#43474e',
      surfaceContainer: '#282a2d',
      surfaceContainerHigh: '#32353a',
      surfaceContainerLow: '#1f2124',
    },
  },
  aurora: {
    name: '极光绿',
    description: '神秘的绿色暗色主题',
    colors: {
      primary: '#a6d5aa',
      onPrimary: '#0b3810',
      primaryContainer: '#234f25',
      onPrimaryContainer: '#c1f1b6',
      secondary: '#c8c9b1',
      onSecondary: '#2f3224',
      secondaryContainer: '#454839',
      onSecondaryContainer: '#dee6c8',
      tertiary: '#b0cfce',
      onTertiary: '#083737',
      tertiaryContainer: '#244e4e',
      onTertiaryContainer: '#bcebeb',
      error: '#ffb4ab',
      onError: '#690005',
      errorContainer: '#93000a',
      onErrorContainer: '#ffdad6',
      surface: '#1b1c18',
      onSurface: '#e3e3db',
      surfaceVariant: '#43473f',
      onSurfaceVariant: '#c4c8c0',
      outline: '#8e9289',
      outlineVariant: '#43473f',
      surfaceContainer: '#2a2b26',
      surfaceContainerHigh: '#353731',
      surfaceContainerLow: '#22221d',
    },
  },
  cosmic: {
    name: '星际紫',
    description: '神秘的紫色暗色主题',
    colors: {
      primary: '#d0bcff',
      onPrimary: '#381e72',
      primaryContainer: '#4f378b',
      onPrimaryContainer: '#e9dfff',
      secondary: '#cbc2db',
      onSecondary: '#332d41',
      secondaryContainer: '#4a4458',
      onSecondaryContainer: '#e9def6',
      tertiary: '#efb8c8',
      onTertiary: '#4a2530',
      tertiaryContainer: '#633b48',
      onTertiaryContainer: '#ffd9e3',
      error: '#ffb4ab',
      onError: '#690005',
      errorContainer: '#93000a',
      onErrorContainer: '#ffdad6',
      surface: '#141218',
      onSurface: '#e6e1e9',
      surfaceVariant: '#4a454e',
      onSurfaceVariant: '#cbc4cf',
      outline: '#958f9a',
      outlineVariant: '#4a454e',
      surfaceContainer: '#1f1a25',
      surfaceContainerHigh: '#2a2630',
      surfaceContainerLow: '#1b1b1f',
    },
  },
  magma: {
    name: '熔岩红',
    description: '热烈的红色暗色主题',
    colors: {
      primary: '#ffb59f',
      onPrimary: '#5f1600',
      primaryContainer: '#842200',
      onPrimaryContainer: '#ffccba',
      secondary: '#e0bfa9',
      onSecondary: '#3d2817',
      secondaryContainer: '#5c3f2d',
      onSecondaryContainer: '#f9e1cf',
      tertiary: '#b9c9a8',
      onTertiary: '#263421',
      tertiaryContainer: '#3c4b36',
      onTertiaryContainer: '#d3e9b0',
      error: '#ffb4ab',
      onError: '#690005',
      errorContainer: '#93000a',
      onErrorContainer: '#ffdad6',
      surface: '#1b1b17',
      onSurface: '#e4e2da',
      surfaceVariant: '#4b463e',
      onSurfaceVariant: '#cdc6bf',
      outline: '#969190',
      outlineVariant: '#4b463e',
      surfaceContainer: '#2a2a24',
      surfaceContainerHigh: '#383832',
      surfaceContainerLow: '#201f1b',
    },
  },
};

// 应用主题到CSS变量
export function applyThemeToCSS(theme: MaterialTheme, isDark: boolean): void {
  const root = document.documentElement;
  const colors = theme.colors;

  root.style.colorScheme = isDark ? "dark" : "light";
  root.style.setProperty('--md-sys-color-primary', colors.primary);
  root.style.setProperty('--md-sys-color-on-primary', colors.onPrimary);
  root.style.setProperty('--md-sys-color-primary-container', colors.primaryContainer);
  root.style.setProperty('--md-sys-color-on-primary-container', colors.onPrimaryContainer);
  root.style.setProperty('--md-sys-color-secondary', colors.secondary);
  root.style.setProperty('--md-sys-color-on-secondary', colors.onSecondary);
  root.style.setProperty('--md-sys-color-secondary-container', colors.secondaryContainer);
  root.style.setProperty('--md-sys-color-on-secondary-container', colors.onSecondaryContainer);
  root.style.setProperty('--md-sys-color-tertiary', colors.tertiary);
  root.style.setProperty('--md-sys-color-on-tertiary', colors.onTertiary);
  root.style.setProperty('--md-sys-color-tertiary-container', colors.tertiaryContainer);
  root.style.setProperty('--md-sys-color-on-tertiary-container', colors.onTertiaryContainer);
  root.style.setProperty('--md-sys-color-error', colors.error);
  root.style.setProperty('--md-sys-color-on-error', colors.onError);
  root.style.setProperty('--md-sys-color-error-container', colors.errorContainer);
  root.style.setProperty('--md-sys-color-on-error-container', colors.onErrorContainer);
  root.style.setProperty('--md-sys-color-surface', colors.surface);
  root.style.setProperty('--md-sys-color-on-surface', colors.onSurface);
  root.style.setProperty('--md-sys-color-surface-variant', colors.surfaceVariant);
  root.style.setProperty('--md-sys-color-on-surface-variant', colors.onSurfaceVariant);
  root.style.setProperty('--md-sys-color-outline', colors.outline);
  root.style.setProperty('--md-sys-color-outline-variant', colors.outlineVariant);
  root.style.setProperty('--md-sys-color-surface-container', colors.surfaceContainer);
  root.style.setProperty('--md-sys-color-surface-container-high', colors.surfaceContainerHigh);
  root.style.setProperty('--md-sys-color-surface-container-low', colors.surfaceContainerLow);
  root.style.setProperty('--brand-gradient', `linear-gradient(135deg, ${colors.primary}, ${colors.secondary})`);

  // 设置背景渐变
  if (isDark) {
    document.body.style.background = colors.surface;
    root.style.setProperty('--bg-secondary', colors.surfaceContainer);
  } else {
    document.body.style.background = `
      radial-gradient(1200px circle at 10% -10%, ${colors.primary}18, transparent 60%),
      radial-gradient(900px circle at 90% 0%, ${colors.secondary}18, transparent 55%),
      linear-gradient(180deg, ${colors.surface} 0%, ${colors.surfaceContainerHigh} 100%)
    `;
    root.style.setProperty('--bg-secondary', colors.surfaceContainerHigh);
  }
}

// 获取所有可用的主题列表
export function getAllThemes() {
  return {
    light: Object.entries(lightThemes).map(([key, theme]) => ({
      key,
      ...theme,
    })),
    dark: Object.entries(darkThemes).map(([key, theme]) => ({
      key,
      ...theme,
    })),
  };
}
