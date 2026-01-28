import type {
  ChannelStats,
  ModelStats,
  Project,
  ProjectInput,
  StatsSeries,
  StatsSummary,
  ToolInfo,
  ToolInstallPlan,
  InstallResult,
  Settings,
  EnvironmentReport,
  UpstreamLatency,
  LogsResponse,
  AutoConfigStatus,
  AutoConfigRequest,
  ToolConfigBackupList,
  ToolConfigBackup,
  GlobalLogsResponse,
  GlobalLogsQuery,
  DeleteLogsRequest,
  InstallLogsResponse,
  InstallLog,
} from "./types";

type RequestOptions = Omit<RequestInit, "body"> & { body?: any };

const defaultBase =
  (window as any).__CCR_API_BASE__ ||
  import.meta.env.VITE_API_BASE ||
  "http://127.0.0.1:8787";

const DEFAULT_HEADERS: Record<string, string> = {
  "Content-Type": "application/json",
  "X-CCR-Channel": "dashboard",
  "X-CCR-Tool": "webui",
};

async function request<T = any>(path: string, options: RequestOptions = {}): Promise<T> {
  const { body, headers, ...rest } = options;
  const finalBody =
    body === undefined || body === null
      ? undefined
      : typeof body === "string"
        ? body
        : JSON.stringify(body);
  const mergedHeaders = {
    ...DEFAULT_HEADERS,
    ...(headers as Record<string, string> | undefined),
  };
  const storedToken = (() => {
    try {
      return localStorage.getItem("ccr_forward_token");
    } catch {
      return null;
    }
  })();
  if (storedToken && !mergedHeaders["X-CCR-Forward-Token"]) {
    mergedHeaders["X-CCR-Forward-Token"] = storedToken;
  }

  const response = await fetch(`${defaultBase}${path}`, {
    ...rest,
    headers: mergedHeaders,
    body: finalBody,
  });

  if (!response.ok) {
    const text = await response.text();
    throw new Error(text || response.statusText);
  }

  if (response.status === 204) {
    return undefined as T;
  }
  return (await response.json()) as T;
}

export const api = {
  stats: {
    summary: (range: string) => request<StatsSummary>(`/api/stats/summary?range=${range}`),
    tokens: (days = 30) => request<StatsSeries>(`/api/stats/series?days=${days}`),
    price: (days = 30) => request<StatsSeries>(`/api/stats/series?metric=price&days=${days}`),
    channels: () => request<{ channels: ChannelStats[] }>("/api/stats/channels"),
    models: (range: string) => request<{ models: ModelStats[] }>(`/api/stats/models?range=${range}`),
    logs: (limit = 50, offset = 0) => request<LogsResponse>(`/api/stats/logs?limit=${limit}&offset=${offset}`),
  },
  projects: {
    list: () => request<Project[]>("/api/projects"),
    create: (payload: ProjectInput) =>
      request<Project>("/api/projects", { method: "POST", body: payload }),
    update: (id: number, payload: ProjectInput) =>
      request<Project>(`/api/projects/${id}`, { method: "PUT", body: payload }),
    remove: (id: number) => request<void>(`/api/projects/${id}`, { method: "DELETE" }),
    open: (id: number, where: string) =>
      request<void>(`/api/projects/${id}/open?where=${encodeURIComponent(where)}`, {
        method: "POST",
      }),
  },
  tools: {
    list: () => request<ToolInfo[]>("/api/tools"),
    install: (id: string) =>
      request<ToolInstallPlan>("/api/tools/install", { method: "POST", body: { id } }),
    executeInstall: (id: string, manager: string) =>
      request<InstallResult>("/api/tools/execute-install", { method: "POST", body: { id, manager } }),
    openHomepage: (id: string) =>
      request<void>(`/api/tools/${id}/open-homepage`, { method: "POST" }),
    openConfig: (id: string) =>
      request<void>(`/api/tools/${id}/open-config`, { method: "POST" }),
    openConfigPath: (id: string) =>
      request<void>(`/api/tools/${id}/open-config-path`, { method: "POST" }),
    openCli: (id: string) => request<void>(`/api/tools/${id}/open-cli`, { method: "POST" }),
  },
  environment: {
    report: () => request<EnvironmentReport>("/api/environment"),
  },
  config: {
    get: () => request<Settings>("/api/config"),
    save: (payload: Settings) => request<void>("/api/config", { method: "PUT", body: payload }),
  },
  upstreams: {
    latency: (id: string) => request<UpstreamLatency>(`/api/upstreams/${id}/latency`),
    testUrls: (urls: string[]) => request<UpstreamLatency>("/api/latency/test", { method: "POST", body: urls }),
  },
  providers: () => request<{ providers: string[] }>("/api/providers"),
  forward: {
    token: () => request<{ token: string }>("/api/forward/token"),
    refreshToken: () => request<{ token: string }>("/api/forward/token", { method: "POST" }),
  },
  export: {
    backup: () => request<any>("/api/export/backup"),
    restore: (data: any) => request<void>("/api/export/restore", { method: "POST", body: data }),
  },
  data: {
    clear: () => request<void>("/api/data/clear", { method: "POST" }),
  },
  autoConfig: {
    status: () => request<AutoConfigStatus>("/api/auto-config/status"),
    configure: (payload: AutoConfigRequest) =>
      request<void>("/api/auto-config/configure", { method: "POST", body: payload }),
    listBackups: (tool: "claude" | "codex" | "gemini") =>
      request<ToolConfigBackupList>(`/api/auto-config/backups/${tool}`),
    createBackup: (tool: "claude" | "codex" | "gemini", description?: string) =>
      request<ToolConfigBackup>("/api/auto-config/backup", {
        method: "POST",
        body: { tool, description },
      }),
    restoreBackup: (backupId: string) =>
      request<void>(`/api/auto-config/backup/${backupId}/restore`, { method: "POST" }),
    deleteBackup: (backupId: string) =>
      request<void>(`/api/auto-config/backup/${backupId}`, { method: "DELETE" }),
  },
  // Global logs API
  logs: {
    query: (query: GlobalLogsQuery = {}) => {
      const params = new URLSearchParams();
      if (query.limit) params.set('limit', String(query.limit));
      if (query.offset) params.set('offset', String(query.offset));
      if (query.level) params.set('level', query.level);
      if (query.source) params.set('source', query.source);
      if (query.start_time) params.set('start_time', String(query.start_time));
      if (query.end_time) params.set('end_time', String(query.end_time));
      return request<GlobalLogsResponse>(`/api/logs?${params.toString()}`);
    },
    count: (query: GlobalLogsQuery = {}) => {
      const params = new URLSearchParams();
      if (query.level) params.set('level', query.level);
      if (query.source) params.set('source', query.source);
      return request<{ count: number }>(`/api/logs/count?${params.toString()}`);
    },
    delete: (id: number) => request<void>(`/api/logs/${id}`, { method: "DELETE" }),
    deleteBatch: (req: DeleteLogsRequest) =>
      request<{ deleted: number }>("/api/logs/delete", { method: "POST", body: req }),
    clear: () => request<{ deleted: number }>("/api/logs", { method: "DELETE" }),
  },
  // Install logs API
  installLogs: {
    list: (limit = 50, offset = 0) =>
      request<InstallLogsResponse>(`/api/install-logs?limit=${limit}&offset=${offset}`),
    get: (id: number) => request<InstallLog>(`/api/install-logs/${id}`),
  },
};

export type { StatsSummary, StatsSeries };
