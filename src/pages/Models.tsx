import { useEffect, useState, useMemo, useCallback, useRef } from "react";
import { api } from "../api";
import { useToast, Modal, useModal } from "../components";
import type { ModelConfig, ModelRoute, Settings, ToolInfo, ToolConfigBackup } from "../types";
import {
  Bot,
  Zap,
  Sparkles,
  Package,
  Folder,
  Lightbulb,
  Archive,
  RotateCcw,
  Trash2,
  Eye,
  Info,
  Save,
  RotateCw,
  FileText,
  SortAsc,
  SortDesc,
  Calendar,
  Clock,
} from "lucide-react";

const emptyRoute = (): ModelRoute => ({
  provider: "",
  upstream_id: "",
  upstream_model_id: "",
  priority: undefined,
});

const emptyModel = (): ModelConfig => ({
  id: "",
  display_name: "",
  provider: "",
  upstream_id: "",
  upstream_model_id: "",
  routes: [emptyRoute()],
  price_prompt_per_1k: 0,
  price_completion_per_1k: 0,
  priority: 50, // Default priority
  is_temporary: false,
});

const CLAUDE_CODE_RESERVED_IDS = new Set([
  "claude-sonnet-4-5-20250929",
  "claude-haiku-4-5-20251001",
]);

const isClaudeCodeReservedModel = (model?: ModelConfig | null) =>
  !!model?.id && CLAUDE_CODE_RESERVED_IDS.has(model.id);

const normalizeRoutes = (model?: ModelConfig | null): ModelRoute[] => {
  if (!model) return [];
  if (model.routes && model.routes.length > 0) {
    return model.routes;
  }
  if (model.provider?.trim() && model.upstream_id?.trim()) {
    return [
      {
        provider: model.provider,
        upstream_id: model.upstream_id,
        upstream_model_id: model.upstream_model_id,
        priority: undefined,
      },
    ];
  }
  return [];
};

const summarizeRoutes = (
  routes: ModelRoute[],
  getValue: (route: ModelRoute) => string | undefined,
  fallback: string,
) => {
  const values = routes
    .map((route) => getValue(route))
    .filter((value): value is string => !!value && value.trim().length > 0);
  if (values.length === 0) return fallback;
  const unique = Array.from(new Set(values));
  if (unique.length === 1) return unique[0];
  return `${unique[0]} +${unique.length - 1}`;
};

const modelSupportsProvider = (model: ModelConfig, provider: string) => {
  const target = provider.toLowerCase();
  return normalizeRoutes(model).some(
    (route) => route.provider.toLowerCase() === target,
  );
};

const getBackupKind = (description: string) => {
  if (description.includes("手动备份")) {
    return { label: "手动", className: "backup-tag--manual" };
  }
  if (description.includes("自动备份")) {
    return { label: "自动", className: "backup-tag--auto" };
  }
  return { label: "备份", className: "backup-tag--default" };
};

const getBackupExtraInfo = (backup: ToolConfigBackup) => {
  const names = (backup.extra_files ?? [])
    .map((file) => file.filename)
    .filter((name) => name.trim().length > 0);

  if (names.length === 0) return null;

  const preview = names.slice(0, 2).join(", ");
  const suffix = names.length > 2 ? ` 等${names.length}个` : "";
  return {
    count: names.length,
    label: `${preview}${suffix}`,
  };
};

const getBackupTimestampValue = (timestamp: ToolConfigBackup["timestamp"]) => {
  if (typeof timestamp === "number") {
    return timestamp;
  }
  const parsed = Date.parse(timestamp);
  return Number.isNaN(parsed) ? 0 : parsed;
};

interface AutoConfigStatus {
  claude: { configured: boolean; model?: string; baseUrl?: string };
  codex: { configured: boolean; model?: string; baseUrl?: string };
  gemini: { configured: boolean; model?: string; baseUrl?: string };
}

export default function Models() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [saving, setSaving] = useState(false);
  const [editingIndex, setEditingIndex] = useState<number | null>(null);
  const [editingModel, setEditingModel] = useState<ModelConfig | null>(null);
  const [tools, setTools] = useState<ToolInfo[]>([]);
  const [autoConfigStatus, setAutoConfigStatus] = useState<AutoConfigStatus>({
    claude: { configured: false },
    codex: { configured: false },
    gemini: { configured: false },
  });
  const [configuring, setConfiguring] = useState<string | null>(null);
  const [selectedModelForConfig, setSelectedModelForConfig] = useState<string>("");
  const [selectedFastModel, setSelectedFastModel] = useState<string>("");
  const [backups, setBackups] = useState<Record<string, ToolConfigBackup[]>>({
    claude: [],
    codex: [],
    gemini: [],
  });
  // showBackups用于renderBackupList函数，但现在主要使用弹窗，保留用于向后兼容
  const [showBackups] = useState<string | null>(null); // Which tool's backups to show
  const [backupModalOpen, setBackupModalOpen] = useState(false);
  const [backupModalTool, setBackupModalTool] = useState<"claude" | "codex" | "gemini" | null>(null);
  const [backupSortOrder, setBackupSortOrder] = useState<"newest" | "oldest">("newest");
  const [previewBackup, setPreviewBackup] = useState<ToolConfigBackup | null>(null);

  // 自动保存相关状态
  const [autoSaveEnabled] = useState(true);
  const [hasUnsavedChanges, setHasUnsavedChanges] = useState(false);
  const [autoSaveIndicator, setAutoSaveIndicator] = useState<string>("");
  const autoSaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const originalSettingsRef = useRef<Settings | null>(null);

  const toast = useToast();
  const { showConfirm } = useModal();

  useEffect(() => {
    api.config
      .get()
      .then((cfg) => {
        setSettings(cfg);
        originalSettingsRef.current = JSON.parse(JSON.stringify(cfg)); // 深拷贝保存原始配置
        setHasUnsavedChanges(false);
      })
      .catch(() => toast.error("无法加载配置"));

    api.tools.list().then(setTools).catch(() => {});

    // 加载自动配置状态
    loadAutoConfigStatus();
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
      toast.success("模型路由已自动保存");

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

  const loadAutoConfigStatus = async () => {
    try {
      const status = await api.autoConfig.status();
      setAutoConfigStatus(status);
    } catch {
      // 忽略错误，使用默认状态
    }
  };

  const loadBackups = async (tool: "claude" | "codex" | "gemini") => {
    try {
      const response = await api.autoConfig.listBackups(tool);
      setBackups(prev => ({ ...prev, [tool]: response.backups }));
    } catch {
      // Silently ignore errors
    }
  };

  // 获取已配置的模型列表供选择（排除Auto和临时模型）
  const availableModels = useMemo(() => {
    if (!settings) return [];
    return settings.models.filter(
      (model) =>
        model.id &&
        normalizeRoutes(model).length > 0 &&
        !model.is_temporary &&
        model.id !== "auto",
    );
  }, [settings]);

  const selectedModel = useMemo(() => {
    if (!settings || !selectedModelForConfig) return null;
    return settings.models.find((model) => model.id === selectedModelForConfig) || null;
  }, [settings, selectedModelForConfig]);

  const supportsAnthropic = !!selectedModel && modelSupportsProvider(selectedModel, "anthropic");
  const supportsOpenAI = !!selectedModel && modelSupportsProvider(selectedModel, "openai");
  const supportsGemini = !!selectedModel && modelSupportsProvider(selectedModel, "gemini");
  const selectedFastModelConfig = useMemo(() => {
    if (!settings || !selectedFastModel) return null;
    return settings.models.find((model) => model.id === selectedFastModel) || null;
  }, [settings, selectedFastModel]);
  const fastModelReadyForClaude =
    !selectedFastModel ||
    (!!selectedFastModelConfig && modelSupportsProvider(selectedFastModelConfig, "anthropic"));

  const temporaryModels = useMemo(() => {
    if (!settings) return [];
    return settings.models.filter((model) => model.is_temporary);
  }, [settings]);

  const hasClaudeCodeTemporaryModels = useMemo(
    () => temporaryModels.some((model) => isClaudeCodeReservedModel(model)),
    [temporaryModels]
  );

  // 检查工具是否已安装
  const isToolInstalled = (toolId: string) => {
    return tools.find(t => t.id === toolId)?.installed ?? false;
  };

  // updateModel function removed as it was unused
  // const updateModel = (index: number, patch: Partial<ModelConfig>) => {
  //   setSettings((prev) => {
  //     if (!prev) return prev;
  //     const nextModels = prev.models.map((model, idx) =>
  //       idx === index ? { ...model, ...patch } : model,
  //     );
  //     return { ...prev, models: nextModels };
  //   });
  // };

  const handleAdd = () => {
    const newModel = emptyModel();
    setEditingModel(newModel);
    setEditingIndex(-1); // -1 表示新增
  };

  const handleEdit = (index: number) => {
    if (!settings) return;
    const model = settings.models[index];
    setEditingModel({ ...model, routes: normalizeRoutes(model) });
    setEditingIndex(index);
  };

  const handleSaveModel = () => {
    if (!settings || !editingModel) return;

    // 验证必填字段
    if (!editingModel.id.trim()) {
      toast.error("请填写模型 ID");
      return;
    }

    const trimmedRoutes = normalizeRoutes(editingModel)
      .map((route) => ({
        provider: route.provider.trim(),
        upstream_id: route.upstream_id.trim(),
        upstream_model_id: route.upstream_model_id?.trim() || undefined,
        priority:
          route.priority === undefined || Number.isNaN(route.priority)
            ? undefined
            : Math.max(0, Math.min(100, Number(route.priority))),
      }))
      .filter(
        (route) =>
          route.provider ||
          route.upstream_id ||
          route.upstream_model_id ||
          route.priority !== undefined,
      );

    if (trimmedRoutes.length === 0) {
      toast.error("请至少配置一个路由");
      return;
    }

    for (const route of trimmedRoutes) {
      if (!route.provider) {
        toast.error("请为每个路由选择提供方");
        return;
      }
      if (!route.upstream_id) {
        toast.error("请为每个路由选择上游");
        return;
      }
    }

    // 验证上游ID是否存在
    const validUpstreams = settings.upstreams.filter((up) => up.id && up.id.trim());
    const validUpstreamIds = new Set(validUpstreams.map((up) => up.id));
    const invalidRoute = trimmedRoutes.find(
      (route) => !validUpstreamIds.has(route.upstream_id),
    );
    if (invalidRoute) {
      toast.error(`路由上游不存在: ${invalidRoute.upstream_id}`);
      return;
    }

    // 如果显示名为空，自动使用ID作为显示名
    const primaryRoute = trimmedRoutes[0];
    const modelToSave = {
      ...editingModel,
      display_name: editingModel.display_name || editingModel.id,
      provider: primaryRoute?.provider || "",
      upstream_id: primaryRoute?.upstream_id || "",
      upstream_model_id: primaryRoute?.upstream_model_id,
      routes: trimmedRoutes,
    };

    if (editingIndex === -1) {
      // 新增
      setSettings({
        ...settings,
        models: [...settings.models, modelToSave],
      });
    } else if (editingIndex !== null) {
      // 编辑
      const nextModels = settings.models.map((model, idx) =>
        idx === editingIndex ? modelToSave : model
      );
      setSettings({ ...settings, models: nextModels });
    }

    setEditingModel(null);
    setEditingIndex(null);
  };

  const handleCancelEdit = () => {
    setEditingModel(null);
    setEditingIndex(null);
  };

  const updateRoute = (index: number, patch: Partial<ModelRoute>) => {
    if (!editingModel) return;
    const routes = editingModel.routes ?? [];
    const nextRoutes = routes.map((route, idx) =>
      idx === index ? { ...route, ...patch } : route,
    );
    setEditingModel({ ...editingModel, routes: nextRoutes });
  };

  const handleAddRoute = () => {
    if (!editingModel) return;
    const routes = editingModel.routes ?? [];
    setEditingModel({ ...editingModel, routes: [...routes, emptyRoute()] });
  };

  const handleRemoveRoute = (index: number) => {
    if (!editingModel) return;
    const routes = editingModel.routes ?? [];
    const nextRoutes = routes.filter((_, idx) => idx !== index);
    setEditingModel({ ...editingModel, routes: nextRoutes });
  };

  const handleRemove = async (index: number) => {
    const model = settings?.models[index];
    const modelName = model?.display_name || model?.id || "此模型";
    const warningMessage = isClaudeCodeReservedModel(model)
      ? "继续操作可能导致Claude Code配置解除，建议先恢复备份，再删除。"
      : "";
    const baseMessage = `确定要删除 "${modelName}" 吗？此操作需要保存后生效。`;
    const message = warningMessage ? `${baseMessage}\n${warningMessage}` : baseMessage;

    const confirmed = await showConfirm("删除模型", message, {
      confirmText: "确定",
      cancelText: "取消",
    });

    if (confirmed) {
      setSettings((prev) => {
        if (!prev) return prev;
        const nextModels = prev.models.filter((_, idx) => idx !== index);
        return { ...prev, models: nextModels };
      });
    }
  };

  const handleRemoveTemporaryModels = async () => {
    if (!settings || temporaryModels.length === 0) {
      return;
    }

    const warningMessage = hasClaudeCodeTemporaryModels
      ? "继续操作可能导致Claude Code配置解除，建议先恢复备份，再删除。"
      : "";
    const baseMessage = `确定要删除 ${temporaryModels.length} 个临时路由（系统生成）吗？此操作需要保存后生效。`;
    const message = warningMessage ? `${baseMessage}\n${warningMessage}` : baseMessage;

    const confirmed = await showConfirm("删除临时路由", message, {
      confirmText: "确定",
      cancelText: "取消",
    });

    if (confirmed) {
      setSettings((prev) => {
        if (!prev) return prev;
        const nextModels = prev.models.filter((model) => !model.is_temporary);
        return { ...prev, models: nextModels };
      });
      toast.success("已移除临时路由，将自动保存");
    }
  };

  const handleSave = async () => {
    // 先取消自动保存定时器
    if (autoSaveTimerRef.current) {
      clearTimeout(autoSaveTimerRef.current);
      autoSaveTimerRef.current = null;
    }

    if (!settings) return;
    setSaving(true);
    setAutoSaveIndicator("正在保存...");
    try {
      await api.config.save(settings);
      originalSettingsRef.current = JSON.parse(JSON.stringify(settings));
      setHasUnsavedChanges(false);
      setAutoSaveIndicator("保存成功");
      toast.success("模型路由已更新");

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

  const handleAutoConfig = async (tool: "claude" | "codex" | "gemini") => {
    if (!selectedModelForConfig) {
      toast.warning("请先选择要配置的模型");
      return;
    }
    const requiredProvider =
      tool === "claude" ? "anthropic" : tool === "codex" ? "openai" : "gemini";
    if (!selectedModel || !modelSupportsProvider(selectedModel, requiredProvider)) {
      toast.error(
        `当前模型未配置${requiredProvider}提供方，无法自动配置${getToolDisplayName(tool)}`,
      );
      return;
    }

    if (tool === "claude" && !fastModelReadyForClaude) {
      toast.error("蹇€熸ā鍨嬮渶閰嶇疆 Anthropic 鎻愪緵鏂癸紝璇烽噸鏂伴€夋嫨");
      return;
    }

    setConfiguring(tool);
    try {
      const canAutoBackup =
        (settings?.backup?.enabled ?? true) &&
        (settings?.backup?.auto_backup_on_config ?? true);

      if (canAutoBackup) {
        // Create backup before configuring
        try {
          const toolName = tool === "claude" ? "Claude Code" : tool === "codex" ? "Codex CLI" : "Gemini CLI";
          await api.autoConfig.createBackup(tool, `配置前自动备份 - ${toolName}`);
          await loadBackups(tool); // Reload backups
        } catch (backupErr) {
          // Continue even if backup fails
          console.warn("Failed to create backup:", backupErr);
        }
      }

      await api.autoConfig.configure({
        tool,
        modelId: selectedModelForConfig,
        fastModelId: tool === "claude" ? selectedFastModel || undefined : undefined, // Only send if set
        global: true, // Always use global config
      });
      toast.success(`${tool === "claude" ? "Claude Code" : tool === "codex" ? "Codex CLI" : "Gemini CLI"} 配置成功`);

      // Reload settings to show newly created special models
      const cfg = await api.config.get();
      setSettings(cfg);
      originalSettingsRef.current = JSON.parse(JSON.stringify(cfg));
      setHasUnsavedChanges(false);

      await loadAutoConfigStatus();
    } catch (err: any) {
      toast.error(err.message || "配置失败");
    } finally {
      setConfiguring(null);
    }
  };

  const handleManualBackup = async (tool: "claude" | "codex" | "gemini") => {
    if (!(settings?.backup?.enabled ?? true)) {
      toast.warning("备份已禁用，请在设置中启用");
      return;
    }
    try {
      const toolName = tool === "claude" ? "Claude Code" : tool === "codex" ? "Codex CLI" : "Gemini CLI";
      await api.autoConfig.createBackup(tool, `手动备份 - ${toolName}`);
      await loadBackups(tool);
      toast.success("备份创建成功");
    } catch (err: any) {
      toast.error(err.message || "备份失败");
    }
  };

  const handleRestoreBackup = async (backupId: string, tool: "claude" | "codex" | "gemini") => {
    const confirmed = await showConfirm("恢复备份", "确定要恢复此备份吗？当前配置将被覆盖。", {
      confirmText: "恢复",
      cancelText: "取消",
    });

    if (confirmed) {
      try {
        await api.autoConfig.restoreBackup(backupId);
        await loadAutoConfigStatus();
        await loadBackups(tool);
        toast.success("配置已恢复");
      } catch (err: any) {
        toast.error(err.message || "恢复失败");
      }
    }
  };

  const handleDeleteBackup = async (backupId: string, tool: "claude" | "codex" | "gemini") => {
    const confirmed = await showConfirm("删除备份", "确定要删除此备份吗？", {
      confirmText: "删除",
      cancelText: "取消",
    });

    if (confirmed) {
      try {
        await api.autoConfig.deleteBackup(backupId);
        await loadBackups(tool);
        toast.success("备份已删除");
      } catch (err: any) {
        toast.error(err.message || "删除失败");
      }
    }
  };

  // 打开备份弹窗
  const openBackupModal = async (tool: "claude" | "codex" | "gemini") => {
    setBackupModalTool(tool);
    setBackupModalOpen(true);
    if (backups[tool].length === 0) {
      await loadBackups(tool);
    }
  };

  // 关闭备份弹窗
  const closeBackupModal = () => {
    setBackupModalOpen(false);
    setBackupModalTool(null);
    setPreviewBackup(null);
  };

  // 获取排序后的备份列表
  const getSortedBackups = (tool: "claude" | "codex" | "gemini") => {
    const list = [...backups[tool]];
    if (backupSortOrder === "newest") {
      return list.sort((a, b) => getBackupTimestampValue(b.timestamp) - getBackupTimestampValue(a.timestamp));
    } else {
      return list.sort((a, b) => getBackupTimestampValue(a.timestamp) - getBackupTimestampValue(b.timestamp));
    }
  };

  // 获取工具显示名称
  const getToolDisplayName = (tool: "claude" | "codex" | "gemini") => {
    switch (tool) {
      case "claude": return "Claude Code";
      case "codex": return "Codex CLI";
      case "gemini": return "Gemini CLI";
    }
  };

  const renderBackupList = (tool: "claude" | "codex" | "gemini") => {
    if (showBackups !== tool) return null;

    const sortedBackups = [...backups[tool]].sort(
      (a, b) => getBackupTimestampValue(b.timestamp) - getBackupTimestampValue(a.timestamp)
    );

    return (
      <div className="backup-list">
        <div className="backup-list-header">
          <span className="backup-count">共 {sortedBackups.length} 个备份</span>
          <span className="muted">最新优先</span>
        </div>
        {sortedBackups.length === 0 ? (
          <p className="no-backups">暂无备份</p>
        ) : (
          sortedBackups.map((backup) => {
            const description = backup.description.trim() || "未命名备份";
            const kind = getBackupKind(backup.description || "");
            const extraInfo = getBackupExtraInfo(backup);

            return (
              <div key={backup.id} className="backup-item">
                <div className="backup-main">
                  <div className="backup-title-row">
                    <span className="backup-desc line-clamp-2">{description}</span>
                    <div className="backup-tags">
                      <span className={`backup-tag ${kind.className}`}>{kind.label}</span>
                      {extraInfo && (
                        <span className="backup-tag backup-tag--extra">+{extraInfo.count} 文件</span>
                      )}
                    </div>
                  </div>
                  <div className="backup-meta">
                    <span className="backup-time">
                      {new Date(backup.timestamp).toLocaleString("zh-CN")}
                    </span>
                    {extraInfo && (
                      <span className="backup-extra">附加文件: {extraInfo.label}</span>
                    )}
                  </div>
                </div>
                <div className="backup-actions">
                  <button
                    type="button"
                    className="backup-action-btn restore"
                    onClick={() => handleRestoreBackup(backup.id, tool)}
                  >
                    <RotateCcw size={12} /> 恢复
                  </button>
                  <button
                    type="button"
                    className="backup-action-btn delete"
                    onClick={() => handleDeleteBackup(backup.id, tool)}
                  >
                    <Trash2 size={12} /> 删除
                  </button>
                </div>
              </div>
            );
          })
        )}
      </div>
    );
  };

  const getBaseUrl = () => {
    // 获取当前服务的基础URL
    const port = 8787;
    return `http://127.0.0.1:${port}`;
  };

  return (
    <div className="page">
      {/* 自动保存状态指示器 */}
      {autoSaveEnabled && autoSaveIndicator && (
        <div className={`auto-save-indicator ${hasUnsavedChanges ? "unsaved" : "saved"}`}>
          <RotateCw size={14} className={saving ? "spin" : ""} />
          <span>{autoSaveIndicator}</span>
        </div>
      )}

      {/* 备份管理弹窗 */}
      <Modal
        open={backupModalOpen}
        onClose={closeBackupModal}
        title={backupModalTool ? `${getToolDisplayName(backupModalTool)} 备份管理` : "备份管理"}
        size="lg"
      >
        {backupModalTool && (
          <div className="backup-modal-content">
            <div className="backup-modal-header">
              <div className="backup-modal-stats">
                <span className="backup-count">共 {backups[backupModalTool].length} 个备份</span>
              </div>
              <div className="backup-modal-actions">
                <button
                  type="button"
                  className="backup-btn secondary sm"
                  onClick={() => setBackupSortOrder(backupSortOrder === "newest" ? "oldest" : "newest")}
                >
                  {backupSortOrder === "newest" ? <SortDesc size={14} /> : <SortAsc size={14} />}
                  {backupSortOrder === "newest" ? "最新优先" : "最旧优先"}
                </button>
                <button
                  type="button"
                  className="backup-btn sm"
                  onClick={() => handleManualBackup(backupModalTool)}
                >
                  <Archive size={14} /> 创建备份
                </button>
              </div>
            </div>

            <div className="backup-modal-list">
              {getSortedBackups(backupModalTool).length === 0 ? (
                <div className="backup-empty">
                  <Archive size={48} className="backup-empty-icon" />
                  <p>暂无备份</p>
                  <p className="muted">点击"创建备份"按钮创建第一个备份</p>
                </div>
              ) : (
                getSortedBackups(backupModalTool).map((backup) => {
                  const description = backup.description.trim() || "未命名备份";
                  const kind = getBackupKind(backup.description || "");
                  const extraInfo = getBackupExtraInfo(backup);
                  const isPreviewActive = previewBackup?.id === backup.id;

                  return (
                    <div key={backup.id} className={`backup-modal-item ${isPreviewActive ? "active" : ""}`}>
                      <div className="backup-modal-item-main">
                        <div className="backup-modal-item-header">
                          <span className="backup-desc">{description}</span>
                          <div className="backup-tags">
                            <span className={`backup-tag ${kind.className}`}>{kind.label}</span>
                            {extraInfo && (
                              <span className="backup-tag backup-tag--extra">+{extraInfo.count} 文件</span>
                            )}
                          </div>
                        </div>
                        <div className="backup-modal-item-meta">
                          <span className="backup-time">
                            <Calendar size={12} />
                            {new Date(backup.timestamp).toLocaleDateString("zh-CN")}
                          </span>
                          <span className="backup-time">
                            <Clock size={12} />
                            {new Date(backup.timestamp).toLocaleTimeString("zh-CN")}
                          </span>
                          {extraInfo && (
                            <span className="backup-extra">附加: {extraInfo.label}</span>
                          )}
                        </div>
                      </div>
                      <div className="backup-modal-item-actions">
                        <button
                          type="button"
                          className="backup-action-btn secondary sm"
                          onClick={() => setPreviewBackup(isPreviewActive ? null : backup)}
                          title="预览内容"
                        >
                          <FileText size={12} /> {isPreviewActive ? "收起" : "预览"}
                        </button>
                        <button
                          type="button"
                          className="backup-action-btn restore"
                          onClick={() => handleRestoreBackup(backup.id, backupModalTool)}
                        >
                          <RotateCcw size={12} /> 恢复
                        </button>
                        <button
                          type="button"
                          className="backup-action-btn delete"
                          onClick={() => handleDeleteBackup(backup.id, backupModalTool)}
                        >
                          <Trash2 size={12} /> 删除
                        </button>
                      </div>
                      {isPreviewActive && (
                        <div className="backup-preview">
                          <div className="backup-preview-header">
                            <span>配置内容预览</span>
                          </div>
                          <pre className="backup-preview-content">
                            {(() => {
                              try {
                                return JSON.stringify(JSON.parse(backup.content), null, 2);
                              } catch {
                                return backup.content;
                              }
                            })()}
                          </pre>
                          {backup.extra_files && backup.extra_files.length > 0 && (
                            <>
                              <div className="backup-preview-header">
                                <span>附加文件</span>
                              </div>
                              {backup.extra_files.map((file, idx) => (
                                <div key={idx} className="backup-preview-extra">
                                  <div className="backup-preview-filename">{file.filename}</div>
                                  <pre className="backup-preview-content">{file.content}</pre>
                                </div>
                              ))}
                            </>
                          )}
                        </div>
                      )}
                    </div>
                  );
                })
              )}
            </div>
          </div>
        )}
      </Modal>

      {/* 编辑/新增模型页面 */}
      {editingModel !== null && (
        <div className="section model-editor-section">
          <div className="model-editor-body">
            <div className="section-header">
              <h2>{editingIndex === -1 ? "新增模型" : "编辑模型"}</h2>
            </div>
            <div className="form-grid">
              <label>
                模型 ID（请求时使用）
                <input
                  value={editingModel.id}
                  onChange={(e) => setEditingModel({ ...editingModel, id: e.target.value })}
                  placeholder="例如: gpt-4o, claude-3-opus"
                />
              </label>
              <label>
                显示名（留空则使用ID）
                <input
                  value={editingModel.display_name}
                  onChange={(e) => setEditingModel({ ...editingModel, display_name: e.target.value })}
                  placeholder={editingModel.id || "自动使用模型ID"}
                />
              </label>
              <label>
                Prompt 价格 ($/1k tokens)
                <input
                  type="number"
                  min="0"
                  step="0.0001"
                  value={editingModel.price_prompt_per_1k}
                  onChange={(e) =>
                    setEditingModel({ ...editingModel, price_prompt_per_1k: Number(e.target.value) })
                  }
                />
              </label>
              <label>
                Completion 价格 ($/1k tokens)
                <input
                  type="number"
                  min="0"
                  step="0.0001"
                  value={editingModel.price_completion_per_1k}
                  onChange={(e) =>
                    setEditingModel({ ...editingModel, price_completion_per_1k: Number(e.target.value) })
                  }
                />
              </label>
              <label>
                优先级 (0-99, 100为系统保留)
                <input
                  type="number"
                  min="0"
                  max="99"
                  value={editingModel.priority}
                  onChange={(e) =>
                    setEditingModel({ ...editingModel, priority: Math.min(99, Math.max(0, Number(e.target.value))) })
                  }
                />
                <span className="muted" style={{ fontSize: 12 }}>数值越大优先级越高,相同ID时优先使用优先级高的模型</span>
              </label>
            </div>
            <div className="route-list">
              <div className="route-list-header">
                <span>路由配置</span>
                <button type="button" className="secondary sm" onClick={handleAddRoute}>
                  添加路由
                </button>
              </div>
              {(editingModel.routes ?? []).length === 0 && (
                <div className="muted route-empty">请添加至少一个路由</div>
              )}
              {(editingModel.routes ?? []).map((route, idx) => (
                <div key={`route-${idx}`} className="route-row">
                  <label>
                    提供方
                    <select
                      value={route.provider}
                      onChange={(e) => updateRoute(idx, { provider: e.target.value })}
                    >
                      <option value="">请选择</option>
                      <option value="openai">OpenAI</option>
                      <option value="anthropic">Anthropic</option>
                      <option value="gemini">Google Gemini</option>
                    </select>
                  </label>
                  <label>
                    上游
                    <select
                      value={route.upstream_id}
                      onChange={(e) => updateRoute(idx, { upstream_id: e.target.value })}
                    >
                      <option value="">请选择上游</option>
                      {settings?.upstreams.filter((up) => up.id && up.id.trim()).map((up) => (
                        <option key={up.id} value={up.id}>
                          {up.id}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label>
                    上游模型ID
                    <input
                      value={route.upstream_model_id || ""}
                      onChange={(e) =>
                        updateRoute(idx, { upstream_model_id: e.target.value })
                      }
                      placeholder="留空则使用模型ID"
                    />
                  </label>
                  <label>
                    上游优先级
                    <input
                      type="number"
                      min="0"
                      max="100"
                      value={route.priority ?? ""}
                      onChange={(e) =>
                        updateRoute(idx, {
                          priority: e.target.value === "" ? undefined : Number(e.target.value),
                        })
                      }
                      placeholder="未设置为随机"
                    />
                  </label>
                  <button
                    type="button"
                    className="secondary sm danger route-remove"
                    onClick={() => handleRemoveRoute(idx)}
                  >
                    移除
                  </button>
                </div>
              ))}
              {settings?.upstreams.filter((up) => up.id && up.id.trim()).length === 0 && (
                <div className="muted route-empty">请先在设置页面配置上游</div>
              )}
            </div>
            <div className="modal-actions">
              <button type="button" className="secondary" onClick={handleCancelEdit}>
                取消
              </button>
              <button type="button" onClick={handleSaveModel} disabled={!editingModel.id}>
                确定
              </button>
            </div>
          </div>
        </div>
      )}

      {/* 模型路由配置卡片 */}
      {editingModel === null && (
        <>
          <div className="section section-models">
        <div className="section-header">
          <h2>模型路由</h2>
          <div className="actions">
            <button type="button" onClick={handleAdd}>
              新增模型
            </button>
            <button
              type="button"
              className="secondary danger"
              onClick={handleRemoveTemporaryModels}
              disabled={!settings || temporaryModels.length === 0}
              title={
                temporaryModels.length === 0
                  ? "暂无临时路由"
                  : `将移除 ${temporaryModels.length} 个临时路由`
              }
            >
              <Trash2 size={16} />
              一键删除临时路由（系统生成）
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
        {!settings && <p className="muted">正在加载配置...</p>}
        {settings && (
          <div className="table-container">
            <table>
              <thead>
                <tr>
                  <th>模型 ID</th>
                  <th>显示名</th>
                  <th>提供方</th>
                  <th>上游</th>
                  <th>上游模型</th>
                  <th>优先级</th>
                  <th>Prompt $/1k</th>
                  <th>Completion $/1k</th>
                  <th>操作</th>
                </tr>
              </thead>
              <tbody>
                {settings.models.length === 0 ? (
                  <tr>
                    <td colSpan={9} className="table-empty">
                      <div className="table-empty-content">
                        <span className="table-empty-icon"><Package size={40} /></span>
                        <p>暂无模型配置</p>
                        <p className="muted">点击"新增模型"添加第一个模型路由</p>
                      </div>
                    </td>
                  </tr>
                ) : (
                  <>
                    {/* Auto路由 - 始终显示在有模型配置的表格顶部 */}
                    <tr
                      className="auto-route-row"
                    >
                      <td>
                        <span className="model-id-cell auto-route-id">
                          auto
                          <span className="auto-route-tag">
                            AUTO
                          </span>
                        </span>
                      </td>
                      <td>
                        <span className="model-name">自动路由 (智能选择)</span>
                      </td>
                      <td>
                        <span className="provider-badge auto-route-badge">
                          auto
                        </span>
                      </td>
                      <td>
                        <span className="upstream-badge auto-route-badge">
                          自动选择最优上游
                        </span>
                      </td>
                      <td>
                        <span className="upstream-model-badge auto-route-badge">
                          根据优先级
                        </span>
                      </td>
                      <td>
                        <span className="priority-badge auto-route-priority">
                          动态
                        </span>
                      </td>
                      <td className="price-cell auto-route-price">自动</td>
                      <td className="price-cell auto-route-price">自动</td>
                      <td className="actions-cell">
                        <div className="row-actions">
                          <button
                            type="button"
                            className="secondary sm"
                            disabled
                            title="Auto路由由系统自动管理"
                          >
                            系统管理
                          </button>
                        </div>
                      </td>
                    </tr>
                    {settings.models.map((model, index) => {
                    const isTemporary = model.is_temporary;
                    const isClaudeCodeReserved = isClaudeCodeReservedModel(model);
                    const routes = normalizeRoutes(model);
                    const providerSummary = summarizeRoutes(
                      routes,
                      (route) => route.provider,
                      model.provider || "-",
                    );
                    const upstreamSummary = summarizeRoutes(
                      routes,
                      (route) => route.upstream_id,
                      model.upstream_id || "未选择",
                    );
                    const upstreamModelSummary = summarizeRoutes(
                      routes,
                      (route) => route.upstream_model_id,
                      model.upstream_model_id || "同ID",
                    );
                    return (
                      <tr
                        key={`${model.id}-${index}`}
                        className={
                          isTemporary
                            ? `temp-model-row${isClaudeCodeReserved ? " temp-model-row--reserved" : ""}`
                            : ""
                        }
                      >
                        <td>
                          <span className="model-id-cell">
                            {model.id || <em className="muted">未设置</em>}
                            {isTemporary && (
                              <span
                                className={`model-flag ${
                                  isClaudeCodeReserved ? "model-flag--reserved" : "model-flag--temp"
                                }`}
                              >
                                {isClaudeCodeReserved ? "CLAUDE CODE" : "TEMP"}
                              </span>
                            )}
                          </span>
                        </td>
                        <td>
                          <span className="model-name">
                            {model.display_name || model.id || <em className="muted">-</em>}
                            {isClaudeCodeReserved && (
                              <span className="model-flag model-flag--note">
                                仅限Claude Code
                              </span>
                            )}
                          </span>
                        </td>
                        <td>
                          <span className="provider-badge">{providerSummary}</span>
                        </td>
                      <td>
                        <span className="upstream-badge">{upstreamSummary}</span>
                      </td>
                      <td>
                        <span className="upstream-model-badge">{upstreamModelSummary}</span>
                      </td>
                      <td>
                        <span className={`priority-badge ${model.priority >= 100 ? 'temp' : model.priority >= 80 ? 'high' : model.priority >= 50 ? 'medium' : 'low'}`}>
                          {model.priority}
                          {model.is_temporary && <span title="临时自动生成">*</span>}
                        </span>
                      </td>
                      <td className="price-cell">${model.price_prompt_per_1k.toFixed(4)}</td>
                      <td className="price-cell">${model.price_completion_per_1k.toFixed(4)}</td>
                      <td className="actions-cell">
                        <div className="row-actions">
                          <button
                            type="button"
                            className="secondary sm"
                            onClick={() => handleEdit(index)}
                            disabled={isClaudeCodeReserved}
                            title={isClaudeCodeReserved ? "Claude Code保留模型不可编辑" : "编辑模型"}
                          >
                            编辑
                          </button>
                          <button
                            type="button"
                            className="secondary sm danger"
                            onClick={() => handleRemove(index)}
                            title={isClaudeCodeReserved ? "删除后可能解除Claude Code配置" : "移除模型"}
                          >
                            移除
                          </button>
                        </div>
                      </td>
                    </tr>
                    );
                  })}
                  </>
                )}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* 自动配置卡片 */}
      <div className="section section-auto-config">
        <div className="section-header">
          <div className="section-title-group">
            <h2>自动配置 AI CLI</h2>
            <span className="config-badge">一键配置</span>
          </div>
        </div>
        <p className="section-description">
          将已配置的模型路由自动同步到本地 AI CLI 工具，无需手动编辑配置文件。
        </p>

        {availableModels.length === 0 ? (
          <div className="empty-config-hint">
            <span className="hint-icon"><Lightbulb size={24} /></span>
            <p>请先在上方添加并保存模型路由配置</p>
          </div>
        ) : (
          <>
            <div className="config-options">
              <label className="config-select-label">
                选择主模型
                <select
                  value={selectedModelForConfig}
                  onChange={(e) => setSelectedModelForConfig(e.target.value)}
                  className="config-model-select"
                >
                  <option value="">请选择模型</option>
                  {availableModels.map((m) => (
                    <option key={m.id} value={m.id}>
                      {m.display_name || m.id} (
                      {summarizeRoutes(
                        normalizeRoutes(m),
                        (route) => route.provider,
                        m.provider || "未配置",
                      )}
                      )
                    </option>
                  ))}
                </select>
              </label>
              <label className="config-select-label">
                选择快速模型（可选）
                <select
                  value={selectedFastModel}
                  onChange={(e) => setSelectedFastModel(e.target.value)}
                  className="config-model-select"
                  title="为Claude Code配置快速模型，用于加速某些操作。如果不选择，将使用主模型。"
                >
                  <option value="">使用主模型</option>
                  {availableModels.map((m) => (
                    <option key={m.id} value={m.id}>
                      {m.display_name || m.id} (
                      {summarizeRoutes(
                        normalizeRoutes(m),
                        (route) => route.provider,
                        m.provider || "未配置",
                      )}
                      )
                    </option>
                  ))}
                </select>
              </label>
            </div>

            <div className="auto-config-grid">
              {/* Claude Code */}
              <div className={`config-tool-card ${!isToolInstalled("claude-code") ? "not-installed" : ""}`}>
                <div className="tool-card-header">
                  <div className="tool-icon claude-icon"><Bot size={24} /></div>
                  <div className="tool-info">
                    <h3>Claude Code</h3>
                    <span className={`install-status ${isToolInstalled("claude-code") ? "installed" : ""}`}>
                      {isToolInstalled("claude-code") ? "已安装" : "未安装"}
                    </span>
                  </div>
                </div>
                {autoConfigStatus.claude.configured && (
                  <div className="current-config">
                    <span className="config-label">当前配置:</span>
                    <span className="config-value">{autoConfigStatus.claude.model || "未知"}</span>
                  </div>
                )}
                <div className="config-path">
                  <span className="path-icon"><Folder size={14} /></span>
                  <code>~/.claude/settings.json</code>
                </div>
                <button
                  type="button"
                  className={`config-btn ${autoConfigStatus.claude.configured ? "reconfigure" : ""}`}
                  onClick={() => handleAutoConfig("claude")}
                  disabled={
                    !isToolInstalled("claude-code") ||
                    configuring === "claude" ||
                    !selectedModelForConfig ||
                    !supportsAnthropic ||
                    !fastModelReadyForClaude
                  }
                >
                  {configuring === "claude" ? "配置中..." : autoConfigStatus.claude.configured ? "重新配置" : "配置"}
                </button>
                <div className="backup-controls">
                  <button
                    type="button"
                    className="backup-btn secondary sm"
                    onClick={() => handleManualBackup("claude")}
                    disabled={!isToolInstalled("claude-code")}
                  >
                    <Archive size={14} /> 手动备份
                  </button>
                  <button
                    type="button"
                    className="backup-btn secondary sm"
                    onClick={() => openBackupModal("claude")}
                    disabled={!isToolInstalled("claude-code")}
                  >
                    <Eye size={14} /> 管理备份
                  </button>
                </div>
                {renderBackupList("claude")}
              </div>

              {/* Codex CLI */}
              <div className={`config-tool-card ${!isToolInstalled("codex") ? "not-installed" : ""}`}>
                <div className="tool-card-header">
                  <div className="tool-icon codex-icon"><Zap size={24} /></div>
                  <div className="tool-info">
                    <h3>Codex CLI</h3>
                    <span className={`install-status ${isToolInstalled("codex") ? "installed" : ""}`}>
                      {isToolInstalled("codex") ? "已安装" : "未安装"}
                    </span>
                  </div>
                </div>
                {autoConfigStatus.codex.configured && (
                  <div className="current-config">
                    <span className="config-label">当前配置:</span>
                    <span className="config-value">{autoConfigStatus.codex.model || "未知"}</span>
                  </div>
                )}
                <div className="config-path">
                  <span className="path-icon"><Folder size={14} /></span>
                  <code>~/.codex/config.toml</code>
                </div>
                <button
                  type="button"
                  className={`config-btn ${autoConfigStatus.codex.configured ? "reconfigure" : ""}`}
                  onClick={() => handleAutoConfig("codex")}
                  disabled={
                    !isToolInstalled("codex") ||
                    configuring === "codex" ||
                    !selectedModelForConfig ||
                    !supportsOpenAI
                  }
                >
                  {configuring === "codex" ? "配置中..." : autoConfigStatus.codex.configured ? "重新配置" : "配置"}
                </button>
                <div className="backup-controls">
                  <button
                    type="button"
                    className="backup-btn secondary sm"
                    onClick={() => handleManualBackup("codex")}
                    disabled={!isToolInstalled("codex")}
                  >
                    <Archive size={14} /> 手动备份
                  </button>
                  <button
                    type="button"
                    className="backup-btn secondary sm"
                    onClick={() => openBackupModal("codex")}
                    disabled={!isToolInstalled("codex")}
                  >
                    <Eye size={14} /> 管理备份
                  </button>
                </div>
                {renderBackupList("codex")}
              </div>

              {/* Gemini CLI */}
              <div className={`config-tool-card ${!isToolInstalled("gemini-cli") ? "not-installed" : ""}`}>
                <div className="tool-card-header">
                  <div className="tool-icon gemini-icon"><Sparkles size={24} /></div>
                  <div className="tool-info">
                    <h3>Gemini CLI</h3>
                    <span className={`install-status ${isToolInstalled("gemini-cli") ? "installed" : ""}`}>
                      {isToolInstalled("gemini-cli") ? "已安装" : "未安装"}
                    </span>
                  </div>
                </div>
                {autoConfigStatus.gemini.configured && (
                  <div className="current-config">
                    <span className="config-label">当前配置:</span>
                    <span className="config-value">{autoConfigStatus.gemini.model || "未知"}</span>
                  </div>
                )}
                <div className="config-path">
                  <span className="path-icon"><Folder size={14} /></span>
                  <code>~/.gemini/settings.json</code>
                </div>
                <button
                  type="button"
                  className={`config-btn ${autoConfigStatus.gemini.configured ? "reconfigure" : ""}`}
                  onClick={() => handleAutoConfig("gemini")}
                  disabled={
                    !isToolInstalled("gemini-cli") ||
                    configuring === "gemini" ||
                    !selectedModelForConfig ||
                    !supportsGemini
                  }
                >
                  {configuring === "gemini" ? "配置中..." : autoConfigStatus.gemini.configured ? "重新配置" : "配置"}
                </button>
                <div className="backup-controls">
                  <button
                    type="button"
                    className="backup-btn secondary sm"
                    onClick={() => handleManualBackup("gemini")}
                    disabled={!isToolInstalled("gemini-cli")}
                  >
                    <Archive size={14} /> 手动备份
                  </button>
                  <button
                    type="button"
                    className="backup-btn secondary sm"
                    onClick={() => openBackupModal("gemini")}
                    disabled={!isToolInstalled("gemini-cli")}
                  >
                    <Eye size={14} /> 管理备份
                  </button>
                </div>
                {renderBackupList("gemini")}
              </div>
            </div>

            <div className="config-info-box">
              <span className="info-icon"><Info size={20} /></span>
              <div className="info-content">
                <p><strong>配置说明：</strong></p>
                <ul>
                  <li>Claude Code: 修改 <code>ANTHROPIC_BASE_URL</code>、<code>ANTHROPIC_MODEL</code> 等环境变量</li>
                  <li>Codex CLI: 修改 <code>config.toml</code> 中的模型提供商配置</li>
                  <li>Gemini CLI: 修改 <code>.env</code> 中的 API 配置</li>
                </ul>
                <p className="muted">配置将使用本地代理地址: <code>{getBaseUrl()}</code></p>
              </div>
            </div>
          </>
        )}
      </div>
        </>
      )}
    </div>
  );
}
