import { useEffect, useMemo, useState, useCallback, useRef } from "react";
import { api } from "../api";
import { useToast, Button, Modal, useModal } from "../components";
import type { LatencyStat, Settings, ThemeConfig, Upstream } from "../types";
import {
  X, Eye, EyeOff, Save, RotateCw, Shield, Palette, Database,
  RefreshCw, Globe, CheckCircle, AlertCircle, Clock, Zap, Server,
  Key, Trash2, Plus, TestTube, Sun, Moon, RotateCcw, Download, Upload, Copy,
  Archive
} from "lucide-react";
import { getAllThemes } from "../theme/presets";
import { applyThemeFromConfig, normalizeThemeConfig, resolveTheme } from "../theme/runtime";

const emptyUpstream = (): Upstream => ({ id: "", endpoints: [] });

// 简化 endpoint URL 显示，只显示域名和端口
const simplifyEndpoint = (url: string): string => {
  try {
    const parsed = new URL(url);
    let result = parsed.hostname;
    if (parsed.port && parsed.port !== "443" && parsed.port !== "80") {
      result += `:${parsed.port}`;
    }
    return result;
  } catch {
    return url;
  }
};

const maskApiKey = (apiKey: string | undefined | null): string => {
  if (!apiKey || apiKey.length <= 8) {
    return apiKey || "";
  }
  const prefix = apiKey.slice(0, 8);
  const suffix = apiKey.slice(-4);
  return `${prefix}...${suffix}`;
};

const copyToClipboard = async (text: string): Promise<boolean> => {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
};

export default function SettingsPage() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [themeModalOpen, setThemeModalOpen] = useState(false);
  const [restartPromptOpen, setRestartPromptOpen] = useState(false);
  const allThemes = useMemo(() => getAllThemes(), []);
  const themeConfig = useMemo(() => normalizeThemeConfig(settings?.theme), [settings?.theme]);
  const resolvedTheme = useMemo(() => resolveTheme(themeConfig), [themeConfig]);
  const resolvePresetName = useCallback(
    (preset: string, primary: typeof allThemes.light, secondary: typeof allThemes.dark, fallback: string) =>
      primary.find((t) => t.key === preset)?.name ||
      secondary.find((t) => t.key === preset)?.name ||
      fallback,
    [],
  );
  const lightPresetName = useMemo(
    () =>
      resolvePresetName(
        themeConfig.light_preset,
        allThemes.light,
        allThemes.dark,
        "默认亮色",
      ),
    [allThemes.dark, allThemes.light, resolvePresetName, themeConfig.light_preset],
  );
  const darkPresetName = useMemo(
    () =>
      resolvePresetName(
        themeConfig.dark_preset,
        allThemes.dark,
        allThemes.light,
        "默认暗色",
      ),
    [allThemes.dark, allThemes.light, resolvePresetName, themeConfig.dark_preset],
  );
  const hasLightCustom = Boolean(themeConfig.light_custom);
  const hasDarkCustom = Boolean(themeConfig.dark_custom);
  const [saving, setSaving] = useState(false);
  const [latencies, setLatencies] = useState<Record<number, LatencyStat[]>>({});
  const [providers, setProviders] = useState<string[]>([]);
  const [refreshingToken, setRefreshingToken] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [importing, setImporting] = useState(false);
  const [clearingData, setClearingData] = useState(false);
  const [endpointInputs, setEndpointInputs] = useState<Record<number, string>>({});
  const [showApiKeys, setShowApiKeys] = useState<Record<number, boolean>>({});

  // 自动保存相关状态
  const [autoSaveEnabled, setAutoSaveEnabled] = useState(true);
  const [hasUnsavedChanges, setHasUnsavedChanges] = useState(false);
  const [autoSaveIndicator, setAutoSaveIndicator] = useState<string>("");
  const autoSaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const originalSettingsRef = useRef<Settings | null>(null);

  const toast = useToast();
  const { showConfirm } = useModal();

  useEffect(() => {
    applyThemeFromConfig(themeConfig);
  }, [themeConfig]);

  useEffect(() => {
    api.config
      .get()
      .then((cfg) => {
        const normalizedTheme = normalizeThemeConfig(cfg.theme);
        const hydrated = { ...cfg, theme: normalizedTheme };
        setSettings(hydrated);
        originalSettingsRef.current = JSON.parse(JSON.stringify(hydrated)); // 深拷贝保存原始配置
        setHasUnsavedChanges(false);

        // 缓存转发 Token
        if (cfg.forward_token) {
          try {
            localStorage.setItem("ccr_forward_token", cfg.forward_token);
          } catch {
            // ignore
          }
        }
      })
      .catch(() => toast.error("无法加载配置"));
    api
      .providers()
      .then((res) => setProviders(res.providers || []))
      .catch(() => {});
  }, []);

  // 监听设置变化，实现自动保存
  useEffect(() => {
    if (!settings || !originalSettingsRef.current || !autoSaveEnabled) {
      return;
    }

    // 检查是否有变化
    const settingsStr = JSON.stringify(settings);
    const originalStr = JSON.stringify(originalSettingsRef.current);

    if (settingsStr !== originalStr) {
      setHasUnsavedChanges(true);
      setAutoSaveIndicator("有未保存的更改");

      // 清除之前的定时器
      if (autoSaveTimerRef.current) {
        clearTimeout(autoSaveTimerRef.current);
      }

      // 设置新的自动保存定时器（3秒后保存）
      autoSaveTimerRef.current = setTimeout(() => {
        handleAutoSave();
      }, 3000);
    } else {
      setHasUnsavedChanges(false);
      setAutoSaveIndicator("");
    }

    return () => {
      if (autoSaveTimerRef.current) {
        clearTimeout(autoSaveTimerRef.current);
      }
    };
  }, [settings, autoSaveEnabled]);

  // 自动保存函数
  const handleAutoSave = useCallback(async () => {
    if (!settings || saving) return;

    setSaving(true);
    setAutoSaveIndicator("正在保存...");

    try {
      await api.config.save(settings);
      originalSettingsRef.current = JSON.parse(JSON.stringify(settings));
      setHasUnsavedChanges(false);
      setAutoSaveIndicator("已自动保存");
      toast.success("配置已自动保存");

      // 2秒后清除指示器
      setTimeout(() => {
        setAutoSaveIndicator("");
      }, 2000);
    } catch (err: any) {
      toast.error("自动保存失败: " + (err.message || "未知错误"));
      setAutoSaveIndicator("保存失败");
    } finally {
      setSaving(false);
    }
  }, [settings, saving, toast]);

  // 手动保存函数（修改后调用自动保存）
  const handleSave = async () => {
    // 先取消自动保存定时器
    if (autoSaveTimerRef.current) {
      clearTimeout(autoSaveTimerRef.current);
      autoSaveTimerRef.current = null;
    }

    if (!settings) return;

    // 验证所有上游都有标识
    const invalidUpstreams = settings.upstreams.filter((up) => !up.id || !up.id.trim());
    if (invalidUpstreams.length > 0) {
      toast.error("请为所有上游填写标识");
      return;
    }

    // 验证上游标识唯一性
    const ids = settings.upstreams.map(up => up.id.trim());
    const duplicates = ids.filter((id, idx) => ids.indexOf(id) !== idx);
    if (duplicates.length > 0) {
      toast.error(`上游标识重复: ${duplicates[0]}`);
      return;
    }

    setSaving(true);
    setAutoSaveIndicator("正在保存...");
    try {
      await api.config.save(settings);
      originalSettingsRef.current = JSON.parse(JSON.stringify(settings));
      setHasUnsavedChanges(false);
      setAutoSaveIndicator("保存成功");
      toast.success("设置已更新");

      // 2秒后清除指示器
      setTimeout(() => {
        setAutoSaveIndicator("");
      }, 2000);
    } catch (err: any) {
      toast.error(err.message || "保存失败");
      setAutoSaveIndicator("保存失败");
    } finally {
      setSaving(false);
    }
  };

  const updateUpstream = (index: number, patch: Partial<Upstream>) => {
    setSettings((prev) => {
      if (!prev) return prev;
      const next = prev.upstreams.map((up, idx) => (idx === index ? { ...up, ...patch } : up));
      return { ...prev, upstreams: next };
    });
  };

  const handleAddEndpoint = (index: number) => {
    const input = endpointInputs[index]?.trim();
    if (!input) return;

    // 验证是否是有效的 URL
    try {
      new URL(input);
    } catch {
      toast.error("请输入有效的 URL 地址");
      return;
    }

    const current = settings?.upstreams[index]?.endpoints || [];
    if (current.includes(input)) {
      toast.warning("该地址已存在");
      return;
    }

    updateUpstream(index, { endpoints: [...current, input] });
    setEndpointInputs((prev) => ({ ...prev, [index]: "" }));
  };

  const handleCopyApiKey = useCallback(async (apiKey: string | undefined) => {
    if (!apiKey) return;
    const success = await copyToClipboard(apiKey);
    if (success) {
      toast.success("API Key 已复制到剪贴板");
    } else {
      toast.error("复制失败，请手动复制");
    }
  }, [toast]);

  const handleRemoveEndpoint = (upstreamIndex: number, endpointIndex: number) => {
    const current = settings?.upstreams[upstreamIndex]?.endpoints || [];
    const next = current.filter((_, idx) => idx !== endpointIndex);
    updateUpstream(upstreamIndex, { endpoints: next });
  };

  const handleEndpointKeyDown = (e: React.KeyboardEvent<HTMLInputElement>, index: number) => {
    if (e.key === "Enter") {
      e.preventDefault();
      handleAddEndpoint(index);
    }
  };

  const handleAddUpstream = () => {
    setSettings((prev) => {
      if (!prev) return prev;
      return { ...prev, upstreams: [...prev.upstreams, emptyUpstream()] };
    });
  };

  const handleRemoveUpstream = async (index: number) => {
    const upstreamId = settings?.upstreams[index]?.id || "此上游";
    const confirmed = await showConfirm(
      "确认删除",
      `确定要删除上游 "${upstreamId}" 吗？此操作需要保存后生效。`,
      {
        confirmText: "删除",
        cancelText: "取消",
      }
    );

    if (confirmed) {
      setSettings((prev) => {
        if (!prev) return prev;
        const next = prev.upstreams.filter((_, idx) => idx !== index);
        return { ...prev, upstreams: next };
      });
      toast.info("上游已移除，请保存配置");
    }
  };

  const handleRetryChange = (field: keyof Settings, value: number) => {
    setSettings((prev) => (prev ? { ...prev, [field]: value } : prev));
  };

  const handlePing = async (index: number) => {
    const upstream = settings?.upstreams[index];
    if (!upstream) return;

    if (upstream.endpoints.length === 0) {
      toast.warning("请先添加终端地址");
      return;
    }

    try {
      const res = await api.upstreams.testUrls(upstream.endpoints);
      setLatencies((prev) => ({ ...prev, [index]: res.latency || [] }));
    } catch {
      toast.error("Ping 失败");
    }
  };

  const handleEditEndpoint = (upstreamIndex: number, endpointIndex: number) => {
    const endpoint = settings?.upstreams[upstreamIndex]?.endpoints[endpointIndex];
    if (!endpoint) return;

    // 将内容放入输入框
    setEndpointInputs((prev) => ({ ...prev, [upstreamIndex]: endpoint }));
    // 删除该标签
    handleRemoveEndpoint(upstreamIndex, endpointIndex);
  };

  const handleRefreshToken = async () => {
    setRefreshingToken(true);
    try {
      const res = await api.forward.refreshToken();
      setSettings((prev) => (prev ? { ...prev, forward_token: res.token } : prev));
      try {
        localStorage.setItem("ccr_forward_token", res.token);
      } catch {
        // ignore
      }
      toast.success("转发 Token 已刷新");
    } catch (err: any) {
      toast.error(err.message || "刷新失败");
    } finally {
      setRefreshingToken(false);
    }
  };

  const handleCopyToken = () => {
    if (!settings?.forward_token) return;
    navigator.clipboard.writeText(settings.forward_token).then(
      () => toast.success("已复制转发 Token"),
      () => toast.error("复制失败"),
    );
  };

  const updateThemeConfig = async (patch: Partial<ThemeConfig>, message: string) => {
    if (!settings) return;
    if (autoSaveTimerRef.current) {
      clearTimeout(autoSaveTimerRef.current);
      autoSaveTimerRef.current = null;
    }
    const nextTheme = { ...themeConfig, ...patch };
    const updatedSettings: Settings = { ...settings, theme: nextTheme };

    setSettings(updatedSettings);
    setRestartPromptOpen(true);

    try {
      await api.config.save(updatedSettings);
      originalSettingsRef.current = JSON.parse(JSON.stringify(updatedSettings));
      setHasUnsavedChanges(false);
      setAutoSaveIndicator("");
      toast.success(message);
    } catch (err: any) {
      toast.error("保存失败: " + (err.message || "未知错误"));
    }
  };

  const handleThemeModeChange = (mode: "light" | "dark" | "auto") => {
    updateThemeConfig({ mode }, "主题模式已更新");
  };

  const handleLightThemeSelect = (key: string) => {
    updateThemeConfig(
      { mode: "light", light_preset: key, light_custom: undefined },
      "亮色主题已应用",
    );
  };

  const handleDarkThemeSelect = (key: string) => {
    updateThemeConfig(
      { mode: "dark", dark_preset: key, dark_custom: undefined },
      "暗色主题已应用",
    );
  };

  const handleImportTheme = async (
    target: "light" | "dark",
    event: React.ChangeEvent<HTMLInputElement>,
  ) => {
    const file = event.target.files?.[0];
    if (!file || !settings) return;

    try {
      const text = await file.text();
      const themeData = JSON.parse(text);

      // 验证主题格式
      if (!themeData || typeof themeData !== "object" || !themeData.colors) {
        throw new Error("无效的主题格式");
      }

      const patch: Partial<ThemeConfig> =
        target === "dark"
          ? { mode: "dark" as const, dark_custom: text }
          : { mode: "light" as const, light_custom: text };
      await updateThemeConfig(patch, "主题已导入");
    } catch (err: any) {
      toast.error("导入失败: " + (err.message || "未知错误"));
    }

    // 清空input
    event.target.value = "";
  };

  const handleExportTheme = (target: "light" | "dark") => {
    if (!settings) {
      toast.error("没有可导出的主题配置");
      return;
    }

    const list = target === "dark" ? allThemes.dark : allThemes.light;
    const presetKey = target === "dark" ? themeConfig.dark_preset : themeConfig.light_preset;
    const preset = list.find((t) => t.key === presetKey) || list[0];
    const custom = target === "dark" ? themeConfig.dark_custom : themeConfig.light_custom;
    const { key: _key, ...themePayload } = preset;
    const themeJson = custom || JSON.stringify(themePayload, null, 2);

    const blob = new Blob([themeJson], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = `theme-${target}-${Date.now()}.json`;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    URL.revokeObjectURL(url);
    toast.success("主题已导出");
  };

  const clearCustomTheme = (target: "light" | "dark") => {
    const patch = target === "dark" ? { dark_custom: undefined } : { light_custom: undefined };
    updateThemeConfig(patch, "已恢复预设主题");
  };

  const handleBackup = async () => {
    setExporting(true);
    try {
      const data = await api.export.backup();
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = `ccr-backup-${Date.now()}.json`;
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);
      URL.revokeObjectURL(url);
      toast.success("备份已导出");
    } catch {
      toast.error("导出失败");
    } finally {
      setExporting(false);
    }
  };

  const handleRestore = async () => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".json";
    input.onchange = async (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (!file) return;

      setImporting(true);
      try {
        const text = await file.text();
        const data = JSON.parse(text);

        // Validate backup data
        if (!data || typeof data !== "object") {
          throw new Error("无效的备份文件格式");
        }

        await api.export.restore(data);
        toast.info("配置已恢复，正在重新加载...");

        // Reload configuration
        setTimeout(async () => {
          try {
            const cfg = await api.config.get();
            setSettings(cfg);
            if (cfg.forward_token) {
              try {
                localStorage.setItem("ccr_forward_token", cfg.forward_token);
              } catch {
                // ignore
              }
            }
            toast.success("配置恢复成功");
          } catch {
            toast.warning("配置恢复成功，但重新加载失败，请刷新页面");
          }
        }, 500);
      } catch (err: any) {
        toast.error(err.message || "恢复失败");
      } finally {
        setImporting(false);
      }
    };
    input.click();
  };

  const handleClearAllData = useCallback(async () => {
    const confirmed = await showConfirm(
      "确认清除全部数据",
      "将清除统计、日志、项目、工具、模型以及本地配置，此操作不可撤销。",
      {
        confirmText: "清除",
        cancelText: "取消",
      },
    );

    if (!confirmed) return;

    if (autoSaveTimerRef.current) {
      clearTimeout(autoSaveTimerRef.current);
      autoSaveTimerRef.current = null;
    }

    setClearingData(true);
    try {
      await api.data.clear();
      toast.success("已清除全部数据");

      const cfg = await api.config.get();
      const normalizedTheme = normalizeThemeConfig(cfg.theme);
      const hydrated = { ...cfg, theme: normalizedTheme };
      setSettings(hydrated);
      originalSettingsRef.current = JSON.parse(JSON.stringify(hydrated));
      setHasUnsavedChanges(false);
      setAutoSaveIndicator("");
      setLatencies({});
      setEndpointInputs({});
      setShowApiKeys({});

      try {
        if (cfg.forward_token) {
          localStorage.setItem("ccr_forward_token", cfg.forward_token);
        } else {
          localStorage.removeItem("ccr_forward_token");
        }
      } catch {
        // ignore
      }
    } catch (err: any) {
      toast.error(err.message || "清除失败");
    } finally {
      setClearingData(false);
    }
  }, [showConfirm, toast]);

  const providerOptions = useMemo(
    () => (providers.length ? providers : ["openai", "anthropic", "gemini"]),
    [providers],
  );

  return (
    <div className="page">
      {/* 自动保存状态指示器 */}
      {autoSaveEnabled && autoSaveIndicator && (
        <div className={`auto-save-indicator ${hasUnsavedChanges ? "unsaved" : "saved"}`}>
          <RotateCw size={14} className={saving ? "spin" : ""} />
          <span>{autoSaveIndicator}</span>
        </div>
      )}

      {/* 安全与主题设置 */}
      <div className="section settings-section-enhanced">
        <div className="section-header">
          <div className="section-title-group">
            <Shield size={20} className="section-icon" />
            <h2>安全与主题</h2>
          </div>
          <div className="actions">
            <button type="button" className="secondary" onClick={handleCopyToken}>
              <Key size={16} />
              复制 Token
            </button>
            <button type="button" onClick={handleRefreshToken} disabled={refreshingToken}>
              <RefreshCw size={16} className={refreshingToken ? "spin" : ""} />
              {refreshingToken ? "刷新中..." : "刷新 Token"}
            </button>
          </div>
        </div>
        {!settings && <p className="muted">加载配置中...</p>}
        {settings && (
          <div className="form-grid">
            <label>
              <span className="muted"><Key size={14} style={{verticalAlign: 'middle', marginRight: 4}} />转发 Token</span>
              <div className="token-chip">{settings.forward_token || "生成中..."}</div>
            </label>
            <label>
              <span className="muted"><Palette size={14} style={{verticalAlign: 'middle', marginRight: 4}} />主题设置</span>
              <button type="button" className="secondary" onClick={() => setThemeModalOpen(true)}>
                <Palette size={16} />
                配置主题
              </button>
            </label>
            <label>
              <span className="muted"><Database size={14} style={{verticalAlign: 'middle', marginRight: 4}} />数据备份</span>
              <div style={{ display: "flex", gap: "8px" }}>
                <button type="button" className="secondary" onClick={handleBackup} disabled={exporting}>
                  {exporting ? "导出中..." : "导出备份"}
                </button>
                <button type="button" className="secondary" onClick={handleRestore} disabled={importing}>
                  {importing ? "恢复中..." : "恢复备份"}
                </button>
              </div>
            </label>
            <label className="full">
              <span className="muted"><Trash2 size={14} style={{verticalAlign: 'middle', marginRight: 4}} />清除全部数据</span>
              <div style={{ display: "flex", gap: "8px", alignItems: "center", flexWrap: "wrap" }}>
                <button
                  type="button"
                  className="secondary danger"
                  onClick={handleClearAllData}
                  disabled={clearingData}
                >
                  {clearingData ? "清除中..." : "一键清除"}
                </button>
                <span className="muted" style={{ fontSize: 12 }}>
                  将清除统计、日志、项目、工具、模型以及本地配置
                </span>
              </div>
            </label>
            <label className="full toggle-inline-row">
              <span className="toggle-label">自动保存</span>
              <label className="toggle-switch-inline">
                <input
                  type="checkbox"
                  checked={autoSaveEnabled}
                  onChange={(e) => setAutoSaveEnabled(e.target.checked)}
                />
                <span className="toggle-slider" />
              </label>
            </label>
          </div>
        )}
      </div>

      {/* 代理配置 */}
      <div className="section settings-section-enhanced">
        <div className="section-header">
          <div className="section-title-group">
            <Globe size={20} className="section-icon" />
            <h2>代理配置</h2>
          </div>
          <div className="actions">
            <button
              type="button"
              onClick={handleSave}
              disabled={saving}
              className={hasUnsavedChanges ? "save-btn-unsaved" : ""}
            >
              <Save size={16} />
              {saving ? "保存中..." : hasUnsavedChanges ? "立即保存" : "保存"}
            </button>
          </div>
        </div>
        {!settings && <p className="muted">加载配置中...</p>}
        {settings && (
          <div className="form-grid">
            <label className="full toggle-inline-row">
              <span className="toggle-label">启用代理</span>
              <label className="toggle-switch-inline">
                <input
                  type="checkbox"
                  checked={settings.proxy?.enabled ?? false}
                  onChange={(e) => setSettings(prev => prev ? {
                    ...prev,
                    proxy: { ...prev.proxy, enabled: e.target.checked, type: prev.proxy?.type || "system" }
                  } : prev)}
                />
                <span className="toggle-slider" />
              </label>
            </label>
            <label>
              <span className="muted">代理类型</span>
              <select
                value={settings.proxy?.type || "system"}
                onChange={(e) => setSettings(prev => prev ? {
                  ...prev,
                  proxy: { ...(prev.proxy || { enabled: false }), type: e.target.value as "system" | "custom" | "none" }
                } : prev)}
                disabled={!settings.proxy?.enabled}
              >
                <option value="system">系统代理</option>
                <option value="custom">自定义代理</option>
                <option value="none">不使用代理</option>
              </select>
            </label>
            {settings.proxy?.type === "custom" && (
              <label className="full">
                <span className="muted">代理地址</span>
                <input
                  type="text"
                  value={settings.proxy?.url || ""}
                  onChange={(e) => setSettings(prev => prev ? {
                    ...prev,
                    proxy: { ...(prev.proxy || { enabled: false }), url: e.target.value }
                  } : prev)}
                  placeholder="http://127.0.0.1:8080"
                  disabled={!settings.proxy?.enabled}
                />
              </label>
            )}
            {settings.proxy?.type === "custom" && (
              <label>
                <span className="muted">用户名</span>
                <input
                  type="text"
                  value={settings.proxy?.username || ""}
                  onChange={(e) => setSettings(prev => prev ? {
                    ...prev,
                    proxy: { ...(prev.proxy || { enabled: false }), username: e.target.value }
                  } : prev)}
                  disabled={!settings.proxy?.enabled}
                  placeholder="可选"
                />
              </label>
            )}
            {settings.proxy?.type === "custom" && (
              <label>
                <span className="muted">密码</span>
                <input
                  type="password"
                  value={settings.proxy?.password || ""}
                  onChange={(e) => setSettings(prev => prev ? {
                    ...prev,
                    proxy: { ...(prev.proxy || { enabled: false }), password: e.target.value }
                  } : prev)}
                  disabled={!settings.proxy?.enabled}
                  placeholder="可选"
                />
              </label>
            )}
            <label className="full">
              <span className="muted">绕过列表</span>
              <input
                type="text"
                value={settings.proxy?.bypass?.join(", ") || ""}
                onChange={(e) => setSettings(prev => prev ? {
                  ...prev,
                  proxy: {
                    ...(prev.proxy || { enabled: false }),
                    bypass: e.target.value ? e.target.value.split(",").map(s => s.trim()) : undefined
                  }
                } : prev)}
                placeholder="localhost,127.0.0.1,*.local"
                disabled={!settings.proxy?.enabled}
              />
              <span className="muted" style={{ fontSize: 12 }}>多个地址用逗号分隔，支持通配符</span>
            </label>
          </div>
        )}
      </div>

      <div className="section settings-section-enhanced">
        <div className="section-header">
          <div className="section-title-group">
            <Zap size={20} className="section-icon" />
            <h2>路由与重试</h2>
          </div>
          <div className="actions">
            <button
              type="button"
              onClick={handleSave}
              disabled={saving}
              className={hasUnsavedChanges ? "save-btn-unsaved" : ""}
            >
              <Save size={16} />
              {saving ? "保存中..." : hasUnsavedChanges ? "立即保存" : "保存"}
            </button>
          </div>
        </div>
        {!settings && <p className="muted">加载配置中...</p>}
        {settings && (
          <div className="form-grid">
            <label className="full toggle-inline-row">
              <span className="toggle-label">启用失败重试</span>
              <label className="toggle-switch-inline">
                <input
                  type="checkbox"
                  checked={settings.enable_retry_fallback ?? false}
                  onChange={(e) => setSettings(prev => prev ? {
                    ...prev,
                    enable_retry_fallback: e.target.checked
                  } : prev)}
                />
                <span className="toggle-slider" />
              </label>
            </label>
            <label className="full toggle-inline-row">
              <span className="toggle-label">启用动态模型</span>
              <label className="toggle-switch-inline">
                <input
                  type="checkbox"
                  checked={settings.enable_dynamic_model ?? false}
                  onChange={(e) => setSettings(prev => prev ? {
                    ...prev,
                    enable_dynamic_model: e.target.checked
                  } : prev)}
                />
                <span className="toggle-slider" />
              </label>
            </label>
            <label>
              <span className="muted">最大重试次数</span>
              <input
                type="number"
                min="1"
                value={settings.retry_max_attempts ?? 4}
                onChange={(e) => handleRetryChange("retry_max_attempts", Number(e.target.value))}
              />
            </label>
            <label>
              <span className="muted">初始重试间隔 (ms)</span>
              <input
                type="number"
                min="50"
                value={settings.retry_initial_ms ?? 300}
                onChange={(e) => handleRetryChange("retry_initial_ms", Number(e.target.value))}
              />
            </label>
            <label>
              <span className="muted">最大重试间隔 (ms)</span>
              <input
                type="number"
                min="50"
                value={settings.retry_max_ms ?? 3000}
                onChange={(e) => handleRetryChange("retry_max_ms", Number(e.target.value))}
              />
            </label>
          </div>
        )}
      </div>

      <div className="section settings-section-enhanced">
        <div className="section-header">
          <div className="section-title-group">
            <Archive size={20} className="section-icon" />
            <h2>自动备份</h2>
          </div>
          <div className="actions">
            <button
              type="button"
              onClick={handleSave}
              disabled={saving}
              className={hasUnsavedChanges ? "save-btn-unsaved" : ""}
            >
              <Save size={16} />
              {saving ? "保存中..." : hasUnsavedChanges ? "立即保存" : "保存"}
            </button>
          </div>
        </div>
        {!settings && <p className="muted">加载配置中...</p>}
        {settings && (
          <div className="form-grid">
            <label className="full toggle-inline-row">
              <span className="toggle-label">启用自动备份</span>
              <label className="toggle-switch-inline">
                <input
                  type="checkbox"
                  checked={settings.backup?.enabled ?? true}
                  onChange={(e) => setSettings(prev => prev ? {
                    ...prev,
                    backup: {
                      ...(prev.backup || { enabled: true, auto_backup_on_config: true, max_backups: 20, tool_paths: [] }),
                      enabled: e.target.checked,
                      tool_paths: prev.backup?.tool_paths ?? []
                    }
                  } : prev)}
                />
                <span className="toggle-slider" />
              </label>
            </label>
            <label className="full toggle-inline-row">
              <span className="toggle-label">配置变更时自动备份</span>
              <label className="toggle-switch-inline">
                <input
                  type="checkbox"
                  checked={settings.backup?.auto_backup_on_config ?? true}
                  onChange={(e) => setSettings(prev => prev ? {
                    ...prev,
                    backup: {
                      ...(prev.backup || { enabled: true, auto_backup_on_config: true, max_backups: 20, tool_paths: [] }),
                      auto_backup_on_config: e.target.checked,
                      tool_paths: prev.backup?.tool_paths ?? []
                    }
                  } : prev)}
                />
                <span className="toggle-slider" />
              </label>
            </label>
            <label>
              <span className="muted">最大备份数</span>
              <input
                type="number"
                min="5"
                max="100"
                value={settings.backup?.max_backups ?? 20}
                onChange={(e) => setSettings(prev => prev ? {
                  ...prev,
                  backup: {
                    ...(prev.backup || { enabled: true, auto_backup_on_config: true, max_backups: 20, tool_paths: [] }),
                    max_backups: Math.max(5, Math.min(100, Number(e.target.value))),
                    tool_paths: prev.backup?.tool_paths ?? []
                  }
                } : prev)}
                disabled={!(settings.backup?.enabled ?? true)}
              />
              <span className="muted" style={{ fontSize: 12 }}>仅保留每个工具最近 N 份</span>
            </label>
            <div className="full">
              <div className="muted" style={{ fontWeight: 600 }}>AI 工具配置路径</div>
              <div className="muted" style={{ fontSize: 12 }}>当前仅支持内置路径，暂不支持自定义</div>
            </div>
          </div>
        )}
      </div>

      <div className="section settings-section-enhanced">
        <div className="section-header">
          <div className="section-title-group">
            <Server size={20} className="section-icon" />
            <h2>上游配置</h2>
          </div>
          <div className="actions">
            <button type="button" onClick={handleAddUpstream}>
              <Plus size={16} />
              新增上游
            </button>
            <button
              type="button"
              className={hasUnsavedChanges ? "" : "secondary"}
              onClick={handleSave}
              disabled={saving}
            >
              <Save size={16} />
              {saving ? "保存中..." : hasUnsavedChanges ? "立即保存" : "保存"}
            </button>
          </div>
        </div>
        {settings &&
          settings.upstreams.map((upstream, index) => (
            <div key={index} className="upstream-card" style={{ marginBottom: 16 }}>
              <div className="upstream-card-header">
                <div className="upstream-title-group">
                  <Server size={18} className="upstream-icon" />
                  <h3>{upstream.id || "新上游"}</h3>
                </div>
                <button
                  type="button"
                  className="secondary danger"
                  onClick={() => handleRemoveUpstream(index)}
                >
                  <Trash2 size={14} />
                  删除
                </button>
              </div>
              <div className="form-grid">
                <label>
                  <span className="muted">标识</span>
                  <input
                    value={upstream.id}
                    onChange={(e) => updateUpstream(index, { id: e.target.value })}
                  />
                </label>
                <label>
                  <span className="muted">API 风格</span>
                  <select
                    value={upstream.api_style || ""}
                    onChange={(e) => updateUpstream(index, { api_style: e.target.value })}
                  >
                    <option value="">请选择</option>
                    {providerOptions.map((p) => (
                      <option key={p} value={p}>
                        {p}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  <span className="muted">API Key（可选）</span>
                  <div className="api-key-input-container">
                    <input
                      type={showApiKeys[index] ? "text" : "password"}
                      value={showApiKeys[index] ? (upstream.api_key || "") : maskApiKey(upstream.api_key)}
                      onChange={(e) => updateUpstream(index, { api_key: e.target.value })}
                      placeholder="留空则使用请求头或环境变量"
                      className="api-key-input"
                    />
                    {upstream.api_key && (
                      <button
                        type="button"
                        className="api-key-toggle secondary"
                        onClick={() => handleCopyApiKey(upstream.api_key)}
                        title="复制 API Key"
                      >
                        <Copy size={16} />
                      </button>
                    )}
                    <button
                      type="button"
                      className="api-key-toggle secondary"
                      onClick={() => setShowApiKeys((prev) => ({ ...prev, [index]: !prev[index] }))}
                      title={showApiKeys[index] ? "隐藏" : "显示"}
                    >
                      {showApiKeys[index] ? <EyeOff size={16} /> : <Eye size={16} />}
                    </button>
                  </div>
                </label>
                <label className="full endpoint-label">
                  <span className="muted">终端地址（输入后按回车添加，点击标签可编辑）</span>
                  <div className="endpoint-input-container">
                    <input
                      type="text"
                      value={endpointInputs[index] || ""}
                      onChange={(e) => setEndpointInputs((prev) => ({ ...prev, [index]: e.target.value }))}
                      onKeyDown={(e) => handleEndpointKeyDown(e, index)}
                      placeholder="https://api.example.com/v1"
                      className="endpoint-input"
                    />
                    <button
                      type="button"
                      className="secondary endpoint-add-btn"
                      onClick={() => handleAddEndpoint(index)}
                    >
                      添加
                    </button>
                  </div>
                  {upstream.endpoints.length > 0 && (
                    <div className="endpoint-tags">
                      {upstream.endpoints.map((ep, epIndex) => (
                        <span
                          key={epIndex}
                          className="endpoint-tag"
                          title={`${ep}\n点击编辑`}
                          onClick={() => handleEditEndpoint(index, epIndex)}
                        >
                          <span className="endpoint-tag-text">{simplifyEndpoint(ep)}</span>
                          <button
                            type="button"
                            className="endpoint-tag-remove"
                            onClick={(e) => {
                              e.stopPropagation();
                              handleRemoveEndpoint(index, epIndex);
                            }}
                          >
                            <X size={14} />
                          </button>
                        </span>
                      ))}
                    </div>
                  )}
                </label>
                <label className="latency-label">
                  <span className="muted"><TestTube size={14} style={{verticalAlign: 'middle', marginRight: 4}} />延迟测试</span>
                  <div className="latency-controls">
                    <button
                      type="button"
                      className="secondary"
                      onClick={() => handlePing(index)}
                      disabled={upstream.endpoints.length === 0}
                    >
                      <Clock size={14} />
                      测试延迟
                    </button>
                  </div>
                </label>
              </div>
              {latencies[index]?.length ? (
                <div className="latency-results">
                  <table className="latency-table">
                    <thead>
                      <tr>
                        <th>节点</th>
                        <th>延迟</th>
                        <th>状态</th>
                      </tr>
                    </thead>
                    <tbody>
                      {latencies[index].map((l, lIndex) => (
                        <tr key={`${index}-${lIndex}`}>
                          <td className="latency-endpoint" title={l.endpoint}>
                            {simplifyEndpoint(l.endpoint)}
                          </td>
                          <td className={`latency-ms ${l.ok ? "ok" : "fail"}`}>
                            {l.ok ? <CheckCircle size={14} style={{verticalAlign: 'middle', marginRight: 4}} /> : <AlertCircle size={14} style={{verticalAlign: 'middle', marginRight: 4}} />}
                            {l.ms ? `${l.ms}ms` : "-"}
                          </td>
                          <td>
                            <span className={`latency-status ${l.ok ? "ok" : "fail"}`}>
                              {l.ok ? "正常" : "超时"}
                            </span>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              ) : (
                <p className="muted latency-hint">点击"测试延迟"查看节点响应时间</p>
              )}
            </div>
          ))}
      </div>

      {/* 主题配置弹窗 */}
      <Modal
        open={themeModalOpen}
        onClose={() => setThemeModalOpen(false)}
        title="主题配置"
        size="lg"
      >
        <div className="theme-config-content">
          <div className="theme-config-header">
            <div className="theme-current">
              <span className="theme-current-label">当前主题</span>
              <div className="theme-current-meta">
                <span className={`theme-pill ${resolvedTheme.isDark ? "dark" : "light"}`}>
                  {resolvedTheme.isDark ? "暗色" : "亮色"}
                </span>
                <span className="theme-current-name">{resolvedTheme.theme.name}</span>
                <span className="theme-current-source">
                  {resolvedTheme.source === "custom" ? "自定义" : "预设"}
                </span>
              </div>
              <span className="theme-current-sub">
                {themeConfig.mode === "auto"
                  ? "自动模式跟随系统"
                  : `模式：${themeConfig.mode === "dark" ? "暗色" : "亮色"}`}
              </span>
            </div>

            <div className="theme-mode-group" role="group" aria-label="主题模式">
              <button
                type="button"
                className={`theme-mode-btn ${themeConfig.mode === "light" ? "active" : ""}`}
                onClick={() => handleThemeModeChange("light")}
              >
                <Sun size={18} />
                亮色
              </button>
              <button
                type="button"
                className={`theme-mode-btn ${themeConfig.mode === "dark" ? "active" : ""}`}
                onClick={() => handleThemeModeChange("dark")}
              >
                <Moon size={18} />
                暗色
              </button>
              <button
                type="button"
                className={`theme-mode-btn ${themeConfig.mode === "auto" ? "active" : ""}`}
                onClick={() => handleThemeModeChange("auto")}
              >
                <RotateCcw size={18} />
                自动
              </button>
            </div>
          </div>

          <div className="theme-panels">
            <section className="theme-panel">
              <div className="theme-panel-header">
                <div className="theme-panel-title">
                  <Sun size={18} />
                  <div>
                    <div className="theme-panel-label">亮色主题</div>
                    <div className="theme-panel-meta">
                      <span>当前预设：{lightPresetName}</span>
                      {hasLightCustom && <span className="theme-custom-pill">自定义已导入</span>}
                    </div>
                  </div>
                </div>
                <div className="theme-panel-actions">
                  {hasLightCustom && (
                    <button
                      type="button"
                      className="secondary sm"
                      onClick={() => clearCustomTheme("light")}
                    >
                      恢复预设
                    </button>
                  )}
                  <button
                    type="button"
                    className="secondary sm"
                    onClick={() => handleExportTheme("light")}
                  >
                    <Download size={14} />
                    导出
                  </button>
                  <label className="theme-import-btn sm">
                    <input
                      type="file"
                      accept=".json"
                      onChange={(event) => handleImportTheme("light", event)}
                    />
                    <Upload size={14} />
                    导入
                  </label>
                </div>
              </div>
              <div className="theme-list">
                {allThemes.light.map((theme) => (
                  <button
                    key={theme.key}
                    type="button"
                    className={`theme-option ${themeConfig.light_preset === theme.key ? "active" : ""}`}
                    onClick={() => handleLightThemeSelect(theme.key)}
                    title={theme.description}
                  >
                    <div
                      className="theme-preview"
                      style={{
                        background: `linear-gradient(135deg, ${theme.colors.primary}, ${theme.colors.secondary})`,
                      }}
                    >
                      <div className="theme-preview-dots">
                        <span
                          className="theme-preview-dot"
                          style={{ background: theme.colors.surface }}
                        />
                        <span
                          className="theme-preview-dot"
                          style={{ background: theme.colors.primary }}
                        />
                        <span
                          className="theme-preview-dot"
                          style={{ background: theme.colors.secondary }}
                        />
                      </div>
                    </div>
                    <div className="theme-option-info">
                      <span className="theme-option-name">{theme.name}</span>
                      <span className="theme-option-desc">{theme.description}</span>
                    </div>
                  </button>
                ))}
              </div>
            </section>

            <section className="theme-panel">
              <div className="theme-panel-header">
                <div className="theme-panel-title">
                  <Moon size={18} />
                  <div>
                    <div className="theme-panel-label">暗色主题</div>
                    <div className="theme-panel-meta">
                      <span>当前预设：{darkPresetName}</span>
                      {hasDarkCustom && <span className="theme-custom-pill">自定义已导入</span>}
                    </div>
                  </div>
                </div>
                <div className="theme-panel-actions">
                  {hasDarkCustom && (
                    <button
                      type="button"
                      className="secondary sm"
                      onClick={() => clearCustomTheme("dark")}
                    >
                      恢复预设
                    </button>
                  )}
                  <button
                    type="button"
                    className="secondary sm"
                    onClick={() => handleExportTheme("dark")}
                  >
                    <Download size={14} />
                    导出
                  </button>
                  <label className="theme-import-btn sm">
                    <input
                      type="file"
                      accept=".json"
                      onChange={(event) => handleImportTheme("dark", event)}
                    />
                    <Upload size={14} />
                    导入
                  </label>
                </div>
              </div>
              <div className="theme-list">
                {allThemes.dark.map((theme) => (
                  <button
                    key={theme.key}
                    type="button"
                    className={`theme-option ${themeConfig.dark_preset === theme.key ? "active" : ""}`}
                    onClick={() => handleDarkThemeSelect(theme.key)}
                    title={theme.description}
                  >
                    <div
                      className="theme-preview"
                      style={{
                        background: `linear-gradient(135deg, ${theme.colors.primary}, ${theme.colors.secondary})`,
                      }}
                    >
                      <div className="theme-preview-dots">
                        <span
                          className="theme-preview-dot"
                          style={{ background: theme.colors.surface }}
                        />
                        <span
                          className="theme-preview-dot"
                          style={{ background: theme.colors.primary }}
                        />
                        <span
                          className="theme-preview-dot"
                          style={{ background: theme.colors.secondary }}
                        />
                      </div>
                    </div>
                    <div className="theme-option-info">
                      <span className="theme-option-name">{theme.name}</span>
                      <span className="theme-option-desc">{theme.description}</span>
                    </div>
                  </button>
                ))}
              </div>
            </section>
          </div>
        </div>
      </Modal>

      {/* 重启提示弹窗 */}
      <Modal
        open={restartPromptOpen}
        onClose={() => setRestartPromptOpen(false)}
        title="重启应用"
        size="sm"
      >
        <div style={{ textAlign: 'center', padding: '20px 0' }}>
          <RotateCcw size={48} style={{ color: 'var(--md-sys-color-primary)', marginBottom: 16 }} />
          <p style={{ fontSize: '1.1rem', marginBottom: 8, fontWeight: 600 }}>
            主题已应用
          </p>
          <p style={{ color: 'var(--md-sys-color-on-surface-variant)', marginBottom: 24 }}>
            部分组件重启后可完全同步
          </p>
          <div className="actions" style={{ justifyContent: 'center', gap: 12 }}>
            <Button variant="secondary" onClick={() => setRestartPromptOpen(false)}>
              稍后重启
            </Button>
            <Button onClick={() => {
              // 尝试通过 Tauri API 重启
              try {
                // @ts-ignore
                if (window.__TAURI__?.process?.relaunch) {
                  // @ts-ignore
                  window.__TAURI__.process.relaunch();
                } else {
                  toast.info("请在开发环境中手动重启应用");
                }
              } catch {
                toast.info("请在开发环境中手动重启应用");
              }
            }}>
              立即重启
            </Button>
          </div>
        </div>
      </Modal>

    </div>
  );
}
