import { useEffect, useMemo, useState } from "react";
import { api } from "../api";
import { useToast, useModal } from "../components";
import type { Project, ProjectInput, ToolInfo } from "../types";

// Tauri dialog API
let openDialog: ((options: { directory: boolean; multiple: boolean }) => Promise<string | string[] | null>) | null = null;

// 动态导入 Tauri dialog
import("@tauri-apps/plugin-dialog")
  .then((module) => {
    openDialog = module.open;
  })
  .catch(() => {
    console.log("Tauri dialog not available, using fallback");
  });

const initialForm: ProjectInput & { tagsText: string } = {
  name: "",
  path: "",
  description: "",
  tags: [],
  tagsText: "",
};

// 项目类型图标和颜色映射
const PROJECT_TYPE_CONFIG: Record<string, { icon: string; color: string; label: string }> = {
  "package.json": { icon: "📦", color: "#f7df1e", label: "Node.js" },
  "Cargo.toml": { icon: "🦀", color: "#dea584", label: "Rust" },
  "go.mod": { icon: "🐹", color: "#00add8", label: "Go" },
  "pyproject.toml": { icon: "🐍", color: "#3776ab", label: "Python" },
  "requirements.txt": { icon: "🐍", color: "#3776ab", label: "Python" },
  "pom.xml": { icon: "☕", color: "#b07219", label: "Java" },
  "build.gradle": { icon: "🐘", color: "#02303a", label: "Gradle" },
  "composer.json": { icon: "🐘", color: "#777bb4", label: "PHP" },
  "Gemfile": { icon: "💎", color: "#cc342d", label: "Ruby" },
  ".csproj": { icon: "🔷", color: "#512bd4", label: "C#" },
  "CMakeLists.txt": { icon: "⚙️", color: "#064f8c", label: "CMake" },
  "Makefile": { icon: "🔧", color: "#6d6e71", label: "Make" },
  "pubspec.yaml": { icon: "🎯", color: "#0175c2", label: "Dart/Flutter" },
  "deno.json": { icon: "🦕", color: "#000000", label: "Deno" },
  "tsconfig.json": { icon: "📘", color: "#3178c6", label: "TypeScript" },
};

// 支持的编辑器配置
const EDITOR_CONFIG: Record<string, { command: string; name: string; icon: string }> = {
  vscode: { command: "code", name: "VS Code", icon: "💻" },
  cursor: { command: "cursor", name: "Cursor", icon: "🖱️" },
  windsurf: { command: "windsurf", name: "Windsurf", icon: "🏄" },
  zed: { command: "zed", name: "Zed", icon: "⚡" },
  sublime: { command: "subl", name: "Sublime", icon: "🔶" },
  atom: { command: "atom", name: "Atom", icon: "⚛️" },
  webstorm: { command: "webstorm", name: "WebStorm", icon: "🌐" },
  idea: { command: "idea", name: "IDEA", icon: "💡" },
};

export default function Projects() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [form, setForm] = useState(initialForm);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [loading, setLoading] = useState(false);
  const [query, setQuery] = useState("");
  const [tools, setTools] = useState<ToolInfo[]>([]);
  const [selectingFolder, setSelectingFolder] = useState(false);
  const [projectTypes, setProjectTypes] = useState<Record<number, string[]>>({});
  const [availableEditors, setAvailableEditors] = useState<string[]>(["vscode"]);
  const [expandedActions, setExpandedActions] = useState<number | null>(null);

  const toast = useToast();
  const { showConfirm } = useModal();

  // 选择文件夹
  const handleSelectFolder = async () => {
    setSelectingFolder(true);

    try {
      if (openDialog) {
        // 使用 Tauri 原生对话框
        const selected = await openDialog({
          directory: true,
          multiple: false,
        });

        if (selected && typeof selected === "string") {
          const pathSep = selected.includes("\\") ? "\\" : "/";
          const folderName = selected.split(pathSep).pop() || "";

          setForm((prev) => ({
            ...prev,
            path: selected,
            name: prev.name || folderName,
          }));
          toast.success("已选择文件夹");
        }
      } else {
        // 降级方案：提示用户手动输入
        toast.warning("请手动输入项目路径（Tauri 对话框不可用）");
      }
    } catch (err: any) {
      console.error("选择文件夹失败:", err);
      toast.error("选择文件夹失败: " + (err.message || "未知错误"));
    } finally {
      setSelectingFolder(false);
    }
  };

  const load = () => {
    api.projects
      .list()
      .then((data) => {
        setProjects(data);
        // 检测每个项目的类型
        detectProjectTypes(data);
      })
      .catch(() => toast.error("无法加载项目，请确认后台服务已运行"));
  };

  // 检测项目配置文件类型 (通过后端API)
  const detectProjectTypes = async (projectList: Project[]) => {
    const types: Record<number, string[]> = {};
    for (const proj of projectList) {
      try {
        const response = await fetch(`http://127.0.0.1:8787/api/projects/${proj.id}/detect-type`);
        if (response.ok) {
          const data = await response.json();
          types[proj.id] = data.types || [];
        }
      } catch {
        // 忽略错误，使用空类型
      }
    }
    setProjectTypes(types);
  };

  // 检测可用编辑器
  const detectAvailableEditors = async () => {
    try {
      const response = await fetch("http://127.0.0.1:8787/api/editors");
      if (response.ok) {
        const data = await response.json();
        setAvailableEditors(data.editors || ["vscode"]);
      }
    } catch {
      // 默认只有 vscode
      setAvailableEditors(["vscode"]);
    }
  };

  useEffect(() => {
    load();
    detectAvailableEditors();
  }, []);

  useEffect(() => {
    api.tools
      .list()
      .then(setTools)
      .catch(() => {});
  }, []);

  const handleSubmit = async (evt: React.FormEvent) => {
    evt.preventDefault();
    setLoading(true);
    const payload: ProjectInput = {
      name: form.name.trim(),
      path: form.path.trim(),
      description: form.description,
      tags: form.tagsText
        .split(",")
        .map((tag) => tag.trim())
        .filter(Boolean),
    };

    try {
      if (editingId) {
        await api.projects.update(editingId, payload);
      } else {
        await api.projects.create(payload);
      }
      setForm(initialForm);
      setEditingId(null);
      toast.success(editingId ? "项目已更新" : "项目已创建");
      load();
    } catch (err: any) {
      toast.error(err.message || "保存失败");
    } finally {
      setLoading(false);
    }
  };

  const handleEdit = (project: Project) => {
    setEditingId(project.id);
    setForm({
      name: project.name,
      path: project.path,
      description: project.description,
      tags: project.tags,
      tagsText: project.tags.join(", "),
    });
  };

  const handleDelete = async (project: Project) => {
    const confirmed = await showConfirm(
      "确认删除",
      `确定要删除项目 "${project.name}" 吗？此操作不可撤销。`,
      {
        confirmText: "删除",
        cancelText: "取消",
      }
    );

    if (confirmed) {
      api.projects
        .remove(project.id)
        .then(() => {
          toast.success("项目已删除");
          load();
        })
        .catch(() => toast.error("删除失败"));
    }
  };

  const handleOpen = (project: Project, target: string) => {
    api.projects.open(project.id, target).catch(() => {
      toast.error("无法打开，可能未安装对应工具");
    });
  };

  const sorted = useMemo(
    () => [...projects].sort((a, b) => b.created_at - a.created_at),
    [projects],
  );

  const filtered = useMemo(() => {
    const key = query.trim().toLowerCase();
    if (!key) return sorted;
    return sorted.filter((p) => {
      const haystack = `${p.name} ${p.path} ${p.description} ${p.tags.join(" ")}`.toLowerCase();
      return haystack.includes(key);
    });
  }, [sorted, query]);

  const installedMap = useMemo(() => {
    return tools.reduce<Record<string, ToolInfo>>((acc, t) => {
      if (t.installed) acc[t.id] = t;
      return acc;
    }, {});
  }, [tools]);

  return (
    <div className="page">
      <div className="section">
        <div className="section-header">
          <h2>{editingId ? "编辑项目" : "新增项目"}</h2>
        </div>
        <form onSubmit={handleSubmit} className="form-grid">
          <label>
            <span className="muted">名称</span>
            <input
              required
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
              placeholder="我的 CLI 实验场"
            />
          </label>
          <label>
            <span className="muted">路径</span>
            <div className="input-with-button">
              <input
                required
                value={form.path}
                onChange={(e) => setForm({ ...form, path: e.target.value })}
                placeholder="C:\dev\project 或点击选择文件夹"
              />
              <button
                type="button"
                className="secondary"
                onClick={handleSelectFolder}
                disabled={selectingFolder}
              >
                {selectingFolder ? "选择中..." : "选择文件夹"}
              </button>
            </div>
          </label>
          <label className="full">
            <span className="muted">描述</span>
            <textarea
              rows={2}
              value={form.description}
              onChange={(e) => setForm({ ...form, description: e.target.value })}
            />
          </label>
          <label className="full">
            <span className="muted">标签（用逗号分隔）</span>
            <input
              value={form.tagsText}
              onChange={(e) => setForm({ ...form, tagsText: e.target.value })}
              placeholder="infra, llm, ops"
            />
          </label>
          <div className="actions full">
            <button type="submit" disabled={loading}>
              {loading ? "保存中..." : "保存"}
            </button>
            {editingId && (
              <button
                type="button"
                className="secondary"
                onClick={() => {
                  setEditingId(null);
                  setForm(initialForm);
                }}
              >
                取消
              </button>
            )}
          </div>
        </form>
      </div>

      <div className="section section-projects">
        <div className="section-header">
          <h2>项目列表</h2>
          <div className="actions">
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="搜索名称/路径/标签"
            />
            <span className="muted">{filtered.length} 条</span>
          </div>
        </div>

        {filtered.length === 0 ? (
          <div className="empty-state">
            <span className="empty-icon">📁</span>
            <p>暂无项目</p>
            <p className="muted">点击上方"选择文件夹"添加第一个项目</p>
          </div>
        ) : (
          <div className="project-card-grid">
            {filtered.map((project) => {
              const types = projectTypes[project.id] || [];
              const isExpanded = expandedActions === project.id;

              return (
                <div key={project.id} className="project-card">
                  {/* 项目类型指示器 */}
                  <div className="project-type-badges">
                    {types.length > 0 ? (
                      types.slice(0, 3).map((type) => {
                        const config = PROJECT_TYPE_CONFIG[type];
                        return config ? (
                          <span
                            key={type}
                            className="type-badge"
                            style={{ borderColor: config.color }}
                            title={config.label}
                          >
                            {config.icon} {config.label}
                          </span>
                        ) : null;
                      })
                    ) : (
                      <span className="type-badge type-badge-unknown">📂 项目</span>
                    )}
                  </div>

                  {/* 项目信息 */}
                  <div className="project-card-header">
                    <h3 className="project-name">{project.name}</h3>
                    <div className="project-actions-toggle">
                      <button
                        type="button"
                        className="icon-btn secondary"
                        onClick={() => setExpandedActions(isExpanded ? null : project.id)}
                        title="更多操作"
                      >
                        ⋮
                      </button>
                    </div>
                  </div>

                  {project.description && (
                    <p className="project-description">{project.description}</p>
                  )}

                  <div className="project-path" title={project.path}>
                    <span className="path-icon">📍</span>
                    <span className="path-text">{project.path}</span>
                  </div>

                  {/* 标签 */}
                  {project.tags.length > 0 && (
                    <div className="project-tags">
                      {project.tags.map((tag) => (
                        <span key={tag} className="tag">
                          {tag}
                        </span>
                      ))}
                    </div>
                  )}

                  {/* 快捷操作按钮 */}
                  <div className="project-quick-actions">
                    <button
                      type="button"
                      className="quick-action-btn"
                      onClick={() => handleOpen(project, "folder")}
                      title="打开文件夹"
                    >
                      📁 文件夹
                    </button>
                    <button
                      type="button"
                      className="quick-action-btn"
                      onClick={() => handleOpen(project, "terminal")}
                      title="打开终端"
                    >
                      💻 终端
                    </button>
                    {availableEditors.includes("vscode") && (
                      <button
                        type="button"
                        className="quick-action-btn"
                        onClick={() => handleOpen(project, "vscode")}
                        title="在 VS Code 中打开"
                      >
                        {EDITOR_CONFIG.vscode.icon} Code
                      </button>
                    )}
                    {availableEditors.includes("cursor") && (
                      <button
                        type="button"
                        className="quick-action-btn"
                        onClick={() => handleOpen(project, "cursor")}
                        title="在 Cursor 中打开"
                      >
                        {EDITOR_CONFIG.cursor.icon} Cursor
                      </button>
                    )}
                  </div>

                  {/* AI 编程助手 - 直接显示 */}
                  {(installedMap["claude-code"] || installedMap["gemini-cli"] || installedMap["codex"]) && (
                    <div className="project-ai-actions">
                      <span className="ai-actions-label">AI 助手</span>
                      <div className="ai-actions-buttons">
                        {installedMap["claude-code"] && (
                          <button
                            type="button"
                            className="quick-action-btn ai-quick-btn"
                            onClick={() => handleOpen(project, "claude")}
                            title="使用 Claude Code 打开"
                          >
                            🤖 Claude
                          </button>
                        )}
                        {installedMap["gemini-cli"] && (
                          <button
                            type="button"
                            className="quick-action-btn ai-quick-btn"
                            onClick={() => handleOpen(project, "gemini")}
                            title="使用 Gemini CLI 打开"
                          >
                            ✨ Gemini
                          </button>
                        )}
                        {installedMap["codex"] && (
                          <button
                            type="button"
                            className="quick-action-btn ai-quick-btn"
                            onClick={() => handleOpen(project, "codex")}
                            title="使用 Codex 打开"
                          >
                            🧠 Codex
                          </button>
                        )}
                      </div>
                    </div>
                  )}

                  {/* 展开的更多操作 */}
                  {isExpanded && (
                    <div className="project-expanded-actions">
                      <div className="action-group">
                        <span className="action-group-label">编辑器</span>
                        <div className="action-group-buttons">
                          {availableEditors.map((editor) => {
                            const config = EDITOR_CONFIG[editor];
                            if (!config) return null;
                            return (
                              <button
                                key={editor}
                                type="button"
                                className="action-btn"
                                onClick={() => handleOpen(project, editor)}
                              >
                                {config.icon} {config.name}
                              </button>
                            );
                          })}
                        </div>
                      </div>

                      <div className="action-group">
                        <span className="action-group-label">管理</span>
                        <div className="action-group-buttons">
                          <button
                            type="button"
                            className="action-btn"
                            onClick={() => {
                              handleEdit(project);
                              setExpandedActions(null);
                            }}
                          >
                            ✏️ 编辑
                          </button>
                          <button
                            type="button"
                            className="action-btn danger-btn"
                            onClick={() => handleDelete(project)}
                          >
                            🗑️ 删除
                          </button>
                        </div>
                      </div>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
