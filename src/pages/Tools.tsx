import { useEffect, useMemo, useState } from "react";
import { api } from "../api";
import { useToast, Modal } from "../components";
import type { EnvironmentReport, ToolInfo, ToolInstallPlan, InstallResult } from "../types";
import { ExternalLink, Terminal, Download, Package, Loader2 } from "lucide-react";

function SectionTable({
  title,
  tools,
  onInstall,
  onOpenConfig,
  onOpenCli,
}: {
  title: string;
  tools: ToolInfo[];
  onInstall: (tool: ToolInfo) => void;
  onOpenConfig: (tool: ToolInfo, folder?: boolean) => void;
  onOpenCli: (tool: ToolInfo) => void;
}) {
  if (!tools.length) return null;
  return (
    <div className="section">
      <div className="section-header">
        <h2>{title}</h2>
      </div>
      <div className="table-container">
        <table className="tools-table">
          <thead>
            <tr>
              <th className="tool-name-cell">名称</th>
              <th className="version-header version-cell">版本</th>
              <th className="config-path-header config-path-cell" title="默认配置文件路径，实际路径可能有区别">配置</th>
              <th className="status-cell">状态</th>
              <th className="actions-cell">操作</th>
            </tr>
          </thead>
          <tbody>
            {tools.map((tool) => (
              <tr key={tool.id}>
                <td className="tool-name-cell">
                  <div className="tool-name">{tool.name}</div>
                  <a href={tool.homepage} target="_blank" rel="noreferrer" className="muted tool-homepage">
                    {tool.homepage}
                  </a>
                </td>
                <td className="version-cell">{tool.version || "未知"}</td>
                <td 
                  className="config-path-cell" 
                  title={tool.config_path ? `默认路径: ${tool.config_path}\n实际配置文件位置可能因安装方式不同而有所区别` : "无配置文件"}
                >
                  <span className="config-path-text">{tool.config_path || "无"}</span>
                </td>
                <td className="status-cell">
                  <span className={`status-pill ${tool.installed ? "active" : "idle"}`}>
                    {tool.installed ? "已安装" : "缺失"}
                  </span>
                </td>
                <td className="actions-cell">
                  <div className="actions row-actions">
                    {tool.installed ? (
                      <>
                        <button type="button" onClick={() => onOpenConfig(tool)}>
                          编辑配置
                        </button>
                        <button
                          type="button"
                          className="secondary"
                          onClick={() => onOpenConfig(tool, true)}
                        >
                          打开目录
                        </button>
                        <button
                          type="button"
                          className="secondary"
                          onClick={() => onOpenCli(tool)}
                          disabled={!tool.launcher}
                        >
                          打开 CLI
                        </button>
                      </>
                    ) : (
                      <button type="button" onClick={() => onInstall(tool)}>
                        <Download size={14} style={{ marginRight: 4 }} />
                        安装
                      </button>
                    )}
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

export default function Tools() {
  const [env, setEnv] = useState<EnvironmentReport | null>(null);
  const [tools, setTools] = useState<ToolInfo[]>([]);
  const [installModal, setInstallModal] = useState<{
    tool: ToolInfo;
    plan: ToolInstallPlan | null;
  } | null>(null);
  const [installing, setInstalling] = useState<string | null>(null);
  const [installResult, setInstallResult] = useState<InstallResult | null>(null);
  
  const toast = useToast();

  const load = () => {
    api.environment
      .report()
      .then((data) => {
        setEnv(data);
        setTools([...(data.ide_tools || []), ...(data.languages || []), ...(data.ai_tools || [])]);
      })
      .catch(() => toast.error("无法加载系统信息"));
  };

  useEffect(() => {
    load();
  }, []);

  // 获取已安装的包管理器
  const installedManagers = useMemo(() => {
    if (!env) return [];
    return env.package_managers.filter(pm => pm.installed).map(pm => pm.name);
  }, [env]);

  const handleInstall = async (tool: ToolInfo) => {
    try {
      const plan = await api.tools.install(tool.id);
      setInstallModal({ tool, plan });
      setInstallResult(null);
    } catch {
      toast.error("无法获取安装信息");
    }
  };

  const handleOpenHomepage = async (tool: ToolInfo) => {
    try {
      await api.tools.openHomepage(tool.id);
    } catch {
      // 如果后端打开失败，尝试前端打开
      window.open(tool.homepage, "_blank");
    }
  };

  const handleExecuteInstall = async (tool: ToolInfo, manager: string) => {
    setInstalling(manager);
    setInstallResult(null);
    try {
      const result = await api.tools.executeInstall(tool.id, manager);
      setInstallResult(result);
      if (result.success) {
        toast.success(result.message);
        // 刷新工具列表
        load();
      } else {
        toast.error(result.message);
      }
    } catch (err: any) {
      toast.error(err.message || "安装失败");
      setInstallResult({
        success: false,
        message: err.message || "安装失败",
        output: undefined,
      });
    } finally {
      setInstalling(null);
    }
  };

  const handleOpenConfig = (tool: ToolInfo, folder = false) => {
    const action = folder ? api.tools.openConfigPath : api.tools.openConfig;
    action(tool.id).catch(() => toast.error("无法打开配置文件/目录"));
  };

  const handleOpenCli = (tool: ToolInfo) => {
    if (!tool.launcher) {
      return;
    }
    api.tools.openCli(tool.id).catch(() => toast.error("无法启动 CLI"));
  };

  const closeInstallModal = () => {
    setInstallModal(null);
    setInstallResult(null);
  };

  const grouped = useMemo(
    () => ({
      ide: tools.filter((t) => t.category === "ide" || t.category === "scm"),
      language: tools.filter((t) => t.category === "language"),
      ai: tools.filter((t) => t.category === "ai-cli"),
    }),
    [tools],
  );

  // 获取工具可用的安装命令（只显示已安装的包管理器）
  const getAvailableCommands = (tool: ToolInfo) => {
    if (!tool.install_commands) return [];
    return tool.install_commands.filter(cmd => installedManagers.includes(cmd.manager));
  };

  return (
    <div className="page">
      {/* 安装模态框 */}
      {installModal && (
        <Modal
          open={true}
          title={`安装 ${installModal.tool.name}`}
          onClose={closeInstallModal}
          size="md"
        >
          <div className="install-modal-content">
            {/* 打开官网按钮 */}
            <div className="install-option install-option-homepage">
              <div className="install-option-header">
                <ExternalLink size={20} />
                <div className="install-option-info">
                  <h4>访问官网下载</h4>
                  <p className="muted">打开官方网站手动下载安装</p>
                </div>
              </div>
              <button
                type="button"
                className="secondary"
                onClick={() => handleOpenHomepage(installModal.tool)}
              >
                <ExternalLink size={14} />
                打开官网
              </button>
            </div>

            {/* 包管理器安装选项 */}
            {getAvailableCommands(installModal.tool).length > 0 && (
              <>
                <div className="install-divider">
                  <span>或使用包管理器安装</span>
                </div>
                <div className="install-commands">
                  {getAvailableCommands(installModal.tool).map((cmd) => (
                    <div key={cmd.manager} className="install-option">
                      <div className="install-option-header">
                        <Terminal size={20} />
                        <div className="install-option-info">
                          <h4>{cmd.manager}</h4>
                          <code className="install-command">{cmd.manager} {cmd.command}</code>
                        </div>
                      </div>
                      <button
                        type="button"
                        onClick={() => handleExecuteInstall(installModal.tool, cmd.manager)}
                        disabled={installing !== null}
                      >
                        {installing === cmd.manager ? (
                          <>
                            <Loader2 size={14} className="spin" />
                            安装中...
                          </>
                        ) : (
                          <>
                            <Package size={14} />
                            执行安装
                          </>
                        )}
                      </button>
                    </div>
                  ))}
                </div>
              </>
            )}

            {/* 没有可用的包管理器时显示所有命令 */}
            {getAvailableCommands(installModal.tool).length === 0 && 
             installModal.plan?.commands && 
             installModal.plan.commands.length > 0 && (
              <>
                <div className="install-divider">
                  <span>安装命令参考</span>
                </div>
                <div className="install-commands-reference">
                  <p className="muted">以下包管理器未安装，请先安装对应的包管理器：</p>
                  {installModal.plan.commands.map((cmd) => (
                    <div key={cmd.manager} className="install-command-ref">
                      <span className="manager-name">{cmd.manager}:</span>
                      <code>{cmd.manager} {cmd.command}</code>
                    </div>
                  ))}
                </div>
              </>
            )}

            {/* 安装结果 */}
            {installResult && (
              <div className={`install-result ${installResult.success ? "success" : "error"}`}>
                <div className="install-result-header">
                  {installResult.success ? "✓ " : "✗ "}
                  {installResult.message}
                </div>
                {installResult.output && (
                  <pre className="install-output">{installResult.output}</pre>
                )}
              </div>
            )}
          </div>
        </Modal>
      )}

      <div className="section">
        <div className="section-header">
          <h2>系统配置</h2>
          {env && (
            <span className="muted">
              {env.os} · {env.arch}
            </span>
          )}
        </div>
        {!env && <p className="muted">正在加载系统信息...</p>}
        {env && (
          <div className="latency-grid">
            {env.package_managers.map((pm) => (
              <span
                key={pm.name}
                className={`status-pill ${pm.installed ? "active" : "idle"}`}
                title={pm.command_path || pm.install_hint || pm.name}
              >
                <span className="pm-name">{pm.name}</span>
                <span className="pm-separator">·</span>
                <span className="pm-status">{pm.installed ? "已就绪" : "未检测到"}</span>
                {pm.installed && pm.command_path && (
                  <>
                    <span className="pm-separator pm-separator-path">·</span>
                    <span className="pm-path">{pm.command_path}</span>
                  </>
                )}
              </span>
            ))}
          </div>
        )}
      </div>

      <SectionTable
        title="IDE / 工具链"
        tools={grouped.ide}
        onInstall={handleInstall}
        onOpenConfig={handleOpenConfig}
        onOpenCli={handleOpenCli}
      />
      <SectionTable
        title="编程语言"
        tools={grouped.language}
        onInstall={handleInstall}
        onOpenConfig={handleOpenConfig}
        onOpenCli={handleOpenCli}
      />
      <SectionTable
        title="AI 辅助工具"
        tools={grouped.ai}
        onInstall={handleInstall}
        onOpenConfig={handleOpenConfig}
        onOpenCli={handleOpenCli}
      />
    </div>
  );
}
