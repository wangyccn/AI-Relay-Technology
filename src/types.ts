export interface StatsSummary {
  range: string;
  requests: number;
  tokens: number;
  price_usd: number;
}

export interface StatsSeries {
  days: string[];
  tokens?: number[];
  price?: number[];
}

export interface ChannelStats {
  channel: string;
  tokens: number;
  price_usd: number;
}

export interface ModelStats {
  model: string;
  requests: number;
  tokens: number;
  price_usd: number;
}

export interface Project {
  id: number;
  name: string;
  path: string;
  description: string;
  tags: string[];
  created_at: number;
}

export interface ProjectInput {
  name: string;
  path: string;
  description?: string;
  tags?: string[];
}

export interface ToolInfo {
  id: string;
  name: string;
  category?: string;
  installed: boolean;
  version?: string;
  command_path?: string;
  config_path?: string;
  launcher?: string;
  install_commands?: InstallCommand[];
  install_hint: string;
  homepage: string;
}

export interface ToolInstallPlan {
  id: string;
  instructions: string;
  url: string;
  commands?: InstallCommand[];
}

export interface InstallResult {
  success: boolean;
  message: string;
  output?: string;
}

export interface InstallCommand {
  manager: string;
  command: string;
}

export interface ThemeConfig {
  mode: 'light' | 'dark' | 'auto';
  light_preset?: string;
  dark_preset?: string;
  light_custom?: string; // Theme JSON
  dark_custom?: string; // Theme JSON
}

export interface RateLimitConfig {
  rpm?: number;
  max_concurrent?: number;
  max_concurrent_per_session?: number;
  budget_daily_usd?: number;
  budget_weekly_usd?: number;
  budget_monthly_usd?: number;
}

// AI 工具配置文件路径
export interface ToolConfigPath {
  tool: 'claude' | 'codex' | 'gemini'; // 工具名称
  config_path: string; // 配置文件路径
  enabled: boolean; // 是否启用该工具的备份
  file_kind?: string; // 配置文件类型 (settings/config/auth/env)
}

export interface BackupConfig {
  enabled: boolean;
  max_backups: number; // 最大备份数量，超过后自动滚动删除
  auto_backup_on_config: boolean; // 配置变更时自动备份
  tool_paths: ToolConfigPath[]; // 各工具配置文件路径
}

export interface Settings {
  upstreams: Upstream[];
  models: ModelConfig[];
  retry_max_attempts?: number;
  retry_initial_ms?: number;
  retry_max_ms?: number;
  forward_token?: string;
  preferred_api_style?: string;
  accent_color?: string; // 保留用于向后兼容
  proxy?: ProxyConfig;
  enable_retry_fallback?: boolean;
  enable_dynamic_model?: boolean;
  limits?: RateLimitConfig;
  theme?: ThemeConfig;
  backup?: BackupConfig;
}

export interface ProxyConfig {
  enabled: boolean;
  type?: 'system' | 'custom' | 'none';
  url?: string;
  username?: string;
  password?: string;
  bypass?: string[];
}

export interface Upstream {
  id: string;
  endpoints: string[];
  api_style?: string;
  api_key?: string;
}

export interface LatencyStat {
  endpoint: string;
  ok: boolean;
  ms?: number;
}

export interface UpstreamLatency {
  upstream: string;
  latency: LatencyStat[];
}

export interface ModelConfig {
  id: string;
  display_name: string;
  provider: string;
  upstream_id: string;
  upstream_model_id?: string;
  routes?: ModelRoute[];
  price_prompt_per_1k: number;
  price_completion_per_1k: number;
  priority: number;
  is_temporary?: boolean;
}

export interface ModelRoute {
  provider: string;
  upstream_id: string;
  upstream_model_id?: string;
  priority?: number;
}

export interface PackageManagerInfo {
  name: string;
  installed: boolean;
  command_path?: string;
  install_hint?: string;
}

export interface EnvironmentReport {
  os: string;
  arch: string;
  package_managers: PackageManagerInfo[];
  ide_tools: ToolInfo[];
  languages: ToolInfo[];
  ai_tools: ToolInfo[];
}

export interface RequestLog {
  id: number;
  timestamp: number;
  channel: string;
  tool: string;
  model: string;
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
  price_usd: number;
  upstream_id: string;
}

export interface LogsResponse {
  logs: RequestLog[];
  total: number;
  limit: number;
  offset: number;
}

// Global logs types
export type LogLevel = 'debug' | 'info' | 'warn' | 'error';

export interface GlobalLogEntry {
  id: number;
  timestamp: number;
  level: LogLevel;
  source: string;
  message: string;
  metadata?: string;
}

export interface GlobalLogsResponse {
  logs: GlobalLogEntry[];
  total: number;
  limit: number;
  offset: number;
}

export interface GlobalLogsQuery {
  limit?: number;
  offset?: number;
  level?: LogLevel;
  source?: string;
  start_time?: number;
  end_time?: number;
}

export interface DeleteLogsRequest {
  ids?: number[];
  level?: LogLevel;
  source?: string;
  before_time?: number;
}

// Install logs types
export type InstallStatus = 'running' | 'success' | 'failed';

export interface InstallLog {
  id: number;
  tool_id: string;
  tool_name: string;
  package_manager: string;
  command: string;
  start_time: number;
  end_time?: number;
  status: InstallStatus;
  output: string;
}

export interface InstallLogsResponse {
  logs: InstallLog[];
  total: number;
  limit: number;
  offset: number;
}

// Auto config types
export interface ToolConfigStatus {
  configured: boolean;
  model?: string;
  baseUrl?: string;
}

export interface AutoConfigStatus {
  claude: ToolConfigStatus;
  codex: ToolConfigStatus;
  gemini: ToolConfigStatus;
}

export interface AutoConfigRequest {
  tool: "claude" | "codex" | "gemini";
  modelId: string;
  fastModelId?: string; // Optional fast model for Claude Code
  global: boolean;
}

// Tool config backup types
export interface ToolConfigBackup {
  id: string;
  tool: "claude" | "codex" | "gemini";
  timestamp: number | string;
  description: string;
  content: string;
  primary_path?: string;
  /** Additional files included in this backup (e.g., .env for gemini) */
  extra_files?: ExtraFileBackup[];
}

export interface ExtraFileBackup {
  filename: string;
  content: string;
  path?: string;
}

export interface ToolConfigBackupList {
  backups: ToolConfigBackup[];
}
