import type { ThemeConfig } from "../types";
import { applyThemeToCSS, darkThemes, lightThemes } from "./presets";
import type { MaterialTheme } from "./presets";

export type ThemeMode = "light" | "dark" | "auto";

export interface NormalizedThemeConfig extends ThemeConfig {
  mode: ThemeMode;
  light_preset: string;
  dark_preset: string;
}

export interface ResolvedTheme {
  theme: MaterialTheme;
  isDark: boolean;
  mode: ThemeMode;
  source: "preset" | "custom";
  presetKey: string;
}

export interface ThemeChangeDetail {
  config?: ThemeConfig;
  resolved: ResolvedTheme;
}

const DEFAULT_LIGHT_KEY = "default";
const DEFAULT_DARK_KEY = "default";
const THEME_EVENT = "ccr-theme-change";

function isThemeMode(value: string | undefined): value is ThemeMode {
  return value === "light" || value === "dark" || value === "auto";
}

function safeParseTheme(raw?: string): MaterialTheme | null {
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as MaterialTheme;
    if (!parsed || typeof parsed !== "object") return null;
    if (!parsed.colors || typeof parsed.colors !== "object") return null;
    if (!parsed.name || typeof parsed.name !== "string") return null;
    return parsed;
  } catch {
    return null;
  }
}

export function getSystemPrefersDark(): boolean {
  if (typeof window === "undefined" || !window.matchMedia) {
    return false;
  }
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export function normalizeThemeConfig(config?: ThemeConfig): NormalizedThemeConfig {
  return {
    mode: isThemeMode(config?.mode) ? config.mode : "light",
    light_preset: config?.light_preset || DEFAULT_LIGHT_KEY,
    dark_preset: config?.dark_preset || DEFAULT_DARK_KEY,
    light_custom: config?.light_custom,
    dark_custom: config?.dark_custom,
  };
}

export function resolveTheme(
  config?: ThemeConfig,
  prefersDark: boolean = getSystemPrefersDark(),
): ResolvedTheme {
  const normalized = normalizeThemeConfig(config);
  const isDark = normalized.mode === "dark" || (normalized.mode === "auto" && prefersDark);
  const customRaw = isDark ? normalized.dark_custom : normalized.light_custom;
  const customTheme = safeParseTheme(customRaw);

  if (customTheme) {
    return {
      theme: customTheme,
      isDark,
      mode: normalized.mode,
      source: "custom",
      presetKey: isDark ? normalized.dark_preset : normalized.light_preset,
    };
  }

  const presets = isDark ? darkThemes : lightThemes;
  const fallbackKey = isDark ? DEFAULT_DARK_KEY : DEFAULT_LIGHT_KEY;
  const requestedKey = isDark ? normalized.dark_preset : normalized.light_preset;
  const theme = presets[requestedKey] || presets[fallbackKey];

  return {
    theme,
    isDark,
    mode: normalized.mode,
    source: "preset",
    presetKey: presets[requestedKey] ? requestedKey : fallbackKey,
  };
}

let activeConfig: ThemeConfig | undefined;

export function applyThemeFromConfig(
  config?: ThemeConfig,
  prefersDark: boolean = getSystemPrefersDark(),
): ResolvedTheme {
  activeConfig = config;
  const resolved = resolveTheme(config, prefersDark);
  applyThemeToCSS(resolved.theme, resolved.isDark);
  if (typeof window !== "undefined") {
    window.dispatchEvent(
      new CustomEvent<ThemeChangeDetail>(THEME_EVENT, {
        detail: { config: activeConfig, resolved },
      }),
    );
  }
  return resolved;
}

export function onThemeChange(handler: (detail: ThemeChangeDetail) => void): () => void {
  if (typeof window === "undefined") {
    return () => {};
  }
  const listener = (event: Event) => {
    const detail = (event as CustomEvent<ThemeChangeDetail>).detail;
    if (detail) {
      handler(detail);
    }
  };
  window.addEventListener(THEME_EVENT, listener);
  return () => window.removeEventListener(THEME_EVENT, listener);
}

export function startThemeWatcher(): () => void {
  if (typeof window === "undefined" || !window.matchMedia) {
    return () => {};
  }

  const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
  const handleChange = () => {
    applyThemeFromConfig(activeConfig, mediaQuery.matches);
  };

  if (mediaQuery.addEventListener) {
    mediaQuery.addEventListener("change", handleChange);
  } else {
    mediaQuery.addListener(handleChange);
  }

  return () => {
    if (mediaQuery.removeEventListener) {
      mediaQuery.removeEventListener("change", handleChange);
    } else {
      mediaQuery.removeListener(handleChange);
    }
  };
}
