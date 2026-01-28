import { useCallback, useEffect, useMemo, useState } from "react";
import React from "react";
import {
  Area,
  AreaChart,
  CartesianGrid,
  Legend,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
  BarChart,
  Bar,
  TooltipProps,
} from "recharts";
import { api } from "../api";
import { Modal, useModal } from "../components";
import type { ChannelStats, ModelStats, StatsSummary, GlobalLogEntry, LogLevel } from "../types";
import { Info, ChevronDown, ChevronUp } from "lucide-react";

type RangeType = "daily" | "weekly" | "monthly";
type ChartRangeType = 7 | 14 | 30 | 90;

const ranges: Array<RangeType> = ["daily", "weekly", "monthly"];
const rangeLabels: Record<RangeType, string> = {
  daily: "今日",
  weekly: "近 7 天",
  monthly: "近 30 天",
};

const chartRangeOptions: Array<{ value: ChartRangeType; label: string }> = [
  { value: 7, label: "近 7 天" },
  { value: 14, label: "近 14 天" },
  { value: 30, label: "近 30 天" },
  { value: 90, label: "近 90 天" },
];

// 生成空数据用于显示空图表
const generateEmptyData = (days: number) => {
  const data = [];
  const today = new Date();
  for (let i = days - 1; i >= 0; i--) {
    const date = new Date(today);
    date.setDate(date.getDate() - i);
    const dateStr = `${date.getMonth() + 1}/${date.getDate()}`;
    data.push({ day: dateStr, tokens: 0, price: 0 });
  }
  return data;
};

const formatTokenCount = (value: number) => {
  if (value >= 1000000) {
    return `${(value / 1000000).toFixed(1)}M`;
  }
  if (value >= 1000) {
    return `${(value / 1000).toFixed(1)}k`;
  }
  return value.toLocaleString();
};

// 自定义 Tooltip 组件
const CustomTooltip = React.memo(({ active, payload, label }: TooltipProps<number, string>) => {
  if (active && payload && payload.length) {
    return (
      <div className="chart-tooltip">
        <p className="chart-tooltip-label">{label}</p>
        {payload.map((entry, index) => (
          <p key={index} className="chart-tooltip-item" style={{ color: entry.color }}>
            {entry.name}: {entry.name === "Spend ($)" ? `$${Number(entry.value).toFixed(4)}` : entry.value?.toLocaleString()}
          </p>
        ))}
      </div>
    );
  }
  return null;
});

// 空状态组件
const EmptyChart = React.memo(({ message = "暂无数据" }: { message?: string }) => (
  <div className="chart-empty">
    <div className="chart-empty-icon">📊</div>
    <p className="chart-empty-text">{message}</p>
  </div>
));

export default function Dashboard() {
  const [summary, setSummary] = useState<Record<string, StatsSummary>>({});
  const [tokenSeries, setTokenSeries] = useState<{ days: string[]; tokens: number[] }>({
    days: [],
    tokens: [],
  });
  const [priceSeries, setPriceSeries] = useState<{ days: string[]; price: number[] }>({
    days: [],
    price: [],
  });
  const [channels, setChannels] = useState<ChannelStats[]>([]);
  const [modelRange, setModelRange] = useState<RangeType>("daily");
  const [models, setModels] = useState<ModelStats[]>([]);
  const [status, setStatus] = useState<string>("");
  const [loading, setLoading] = useState(true);

  // 图表时间范围状态
  const [chartRange, setChartRange] = useState<ChartRangeType>(30);
  const [channelRange, setChannelRange] = useState<RangeType>("monthly");

  // 请求日志状态
  const [logs, setLogs] = useState<GlobalLogEntry[]>([]);
  const [logsTotal, setLogsTotal] = useState(0);
  const [logsPage, setLogsPage] = useState(0);
  const [logsLoading, setLogsLoading] = useState(false);
  const [logsLevel, setLogsLevel] = useState<LogLevel | ''>('');
  const [logsSource, setLogsSource] = useState<string>('');
  const [logsRefreshKey, setLogsRefreshKey] = useState(0);
  const [selectedLog, setSelectedLog] = useState<GlobalLogEntry | null>(null);
  const [expandedLogIds, setExpandedLogIds] = useState<Set<number>>(new Set());
  const logsPageSize = 20;

  // 加载统计摘要
  useEffect(() => {
    ranges.forEach((range) =>
      api.stats
        .summary(range)
        .then((data) =>
          setSummary((prev) => ({
            ...prev,
            [range]: data,
          })),
        )
        .catch(() => {}),
    );
  }, []);

  // 加载图表数据
  useEffect(() => {
    setLoading(true);
    Promise.all([api.stats.tokens(chartRange), api.stats.price(chartRange)])
      .then(([tokens, price]) => {
        setTokenSeries({
          days: tokens.days || [],
          tokens: tokens.tokens || [],
        });
        setPriceSeries({
          days: price.days || [],
          price: price.price || [],
        });
      })
      .catch(() => setStatus("无法加载数据，请确认 CCR 服务已运行"))
      .finally(() => setLoading(false));
  }, [chartRange]);

  // 加载渠道数据
  useEffect(() => {
    api.stats
      .channels()
      .then((channelResp) => {
        setChannels(channelResp.channels || []);
      })
      .catch(() => setChannels([]));
  }, [channelRange]);

  // 加载模型数据
  useEffect(() => {
    api.stats
      .models(modelRange)
      .then((resp) => setModels(resp.models || []))
      .catch(() => setModels([]));
  }, [modelRange]);

  // 加载请求日志
  useEffect(() => {
    setLogsLoading(true);
    api.logs
      .query({
        limit: logsPageSize,
        offset: logsPage * logsPageSize,
        level: logsLevel || undefined,
        source: logsSource || undefined,
      })
      .then((resp) => {
        setLogs(resp.logs || []);
        setLogsTotal(resp.total || 0);
      })
      .catch(() => {
        setLogs([]);
        setLogsTotal(0);
      })
      .finally(() => setLogsLoading(false));
  }, [logsPage, logsLevel, logsSource, logsRefreshKey]);

  // 格式化时间戳
  const formatTimestamp = useCallback((ts: number) => {
    const date = new Date(ts * 1000);
    return date.toLocaleString('zh-CN', {
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    });
  }, []);

  // 计算总页数
  const totalPages = Math.ceil(logsTotal / logsPageSize);

  // 处理图表数据
  const chartData = useMemo(() => {
    if (tokenSeries.days.length === 0) {
      return generateEmptyData(chartRange);
    }
    return tokenSeries.days.map((day, idx) => ({
      day,
      tokens: tokenSeries.tokens[idx] || 0,
      price: priceSeries.price[idx] || 0,
    }));
  }, [tokenSeries, priceSeries, chartRange]);

  // 检查是否有实际数据
  const hasChartData = useMemo(() => {
    return chartData.some((d) => d.tokens > 0 || d.price > 0);
  }, [chartData]);

  const hasChannelData = channels.length > 0;

  // 生成空渠道数据
  const channelChartData = useMemo(() => {
    if (channels.length === 0) {
      return [
        { channel: "无数据", price_usd: 0, tokens: 0 },
      ];
    }
    return channels;
  }, [channels]);

  const { showConfirm } = useModal();

  // 处理清空日志确认
  const handleClearLogs = useCallback(async () => {
    const confirmed = await showConfirm(
      "确认清空日志",
      "确定要清空所有系统日志吗？清空后将无法恢复。",
      {
        confirmText: "确认清空",
        cancelText: "取消",
      }
    );

    if (confirmed) {
      try {
        await api.logs.clear();
        setLogs([]);
        setLogsTotal(0);
        setLogsPage(0);
      } catch (e) {
        console.error('清空日志失败:', e);
      }
    }
  }, [showConfirm]);

  const handleDeleteLog = useCallback(async (logId: number) => {
    try {
      await api.logs.delete(logId);
      setLogs(logs.filter(l => l.id !== logId));
      setLogsTotal(t => t - 1);
    } catch (e) {
      console.error('删除日志失败:', e);
    }
  }, [logs]);

  const handleLogsFilterChange = useCallback((level: LogLevel | '') => {
    setLogsLevel(level);
    setLogsPage(0);
  }, []);

  const handleLogsSourceChange = useCallback((source: string) => {
    setLogsSource(source);
    setLogsPage(0);
  }, []);

  const handleLogsRefresh = useCallback(() => {
    setLogsRefreshKey((prev) => prev + 1);
  }, []);

  // 切换日志展开/收起
  const toggleLogExpand = useCallback((logId: number) => {
    setExpandedLogIds(prev => {
      const newSet = new Set(prev);
      if (newSet.has(logId)) {
        newSet.delete(logId);
      } else {
        newSet.add(logId);
      }
      return newSet;
    });
  }, []);

  // 打开日志详情弹窗
  const openLogDetail = useCallback((log: GlobalLogEntry) => {
    setSelectedLog(log);
  }, []);

  // 关闭日志详情弹窗
  const closeLogDetail = useCallback(() => {
    setSelectedLog(null);
  }, []);

  // 解析日志元数据
  const parseLogMetadata = useCallback((metadata?: string) => {
    if (!metadata) return null;
    try {
      return JSON.parse(metadata);
    } catch {
      return metadata;
    }
  }, []);

  return (
    <div className="page">
      {status && (
        <div className="status-banner status-error">
          <span className="status-icon">⚠️</span>
          {status}
        </div>
      )}

      {/* 统计卡片 */}
      <div className="card-grid">
        {ranges.map((range) => {
          const spend = summary[range]?.price_usd ?? 0;
          const requests = summary[range]?.requests ?? 0;
          const tokens = summary[range]?.tokens ?? 0;
          return (
            <div key={range} className="card stat-card">
              <div className="stat-header">
                <h3>{rangeLabels[range]}</h3>
                <span className="stat-icon">
                  {range === "daily" ? "📅" : range === "weekly" ? "📆" : "🗓️"}
                </span>
              </div>
              <div className="stat">{requests.toLocaleString()} <span className="stat-unit">次请求</span></div>
              <div className="stat-details">
                <div className="stat-item">
                  <span className="stat-label">Tokens</span>
                  <span className="stat-value" title={tokens.toLocaleString()}>
                    {formatTokenCount(tokens)}
                  </span>
                </div>
                <div className="stat-item">
                  <span className="stat-label">费用</span>
                  <span className="stat-value">${spend.toFixed(2)}</span>
                </div>
              </div>
            </div>
          );
        })}
      </div>

      {/* 图表区域 */}
      <div className="chart-grid">
        {/* Tokens 与费用趋势图 */}
        <div className="section chart-section">
          <div className="section-header">
            <div className="section-title-group">
              <h2>Tokens 与费用趋势</h2>
              {!hasChartData && <span className="no-data-badge">暂无数据</span>}
            </div>
            <div className="chart-controls">
              <select
                value={chartRange}
                onChange={(e) => setChartRange(Number(e.target.value) as ChartRangeType)}
                className="chart-range-select"
              >
                {chartRangeOptions.map((opt) => (
                  <option key={opt.value} value={opt.value}>{opt.label}</option>
                ))}
              </select>
            </div>
          </div>
          <div className="chart-container">
            {loading ? (
              <div className="chart-loading">
                <div className="loading-spinner"></div>
                <p>加载中...</p>
              </div>
            ) : (
              <ResponsiveContainer width="100%" height={300}>
                <AreaChart data={chartData} margin={{ top: 10, right: 30, left: 0, bottom: 0 }}>
                  <defs>
                    <linearGradient id="colorTokens" x1="0" y1="0" x2="0" y2="1">
                      <stop offset="5%" stopColor="var(--md-sys-color-primary)" stopOpacity={0.8} />
                      <stop offset="95%" stopColor="var(--md-sys-color-primary)" stopOpacity={0.05} />
                    </linearGradient>
                    <linearGradient id="colorSpend" x1="0" y1="0" x2="0" y2="1">
                      <stop offset="5%" stopColor="var(--md-sys-color-secondary)" stopOpacity={0.8} />
                      <stop offset="95%" stopColor="var(--md-sys-color-secondary)" stopOpacity={0.05} />
                    </linearGradient>
                  </defs>
                  <CartesianGrid
                    strokeDasharray="3 3"
                    stroke="var(--md-sys-color-outline-variant)"
                    vertical={false}
                  />
                  <XAxis
                    dataKey="day"
                    axisLine={false}
                    tickLine={false}
                    tick={{ fill: 'var(--md-sys-color-on-surface-variant)', fontSize: 12 }}
                    dy={10}
                  />
                  <YAxis
                    yAxisId="left"
                    axisLine={false}
                    tickLine={false}
                    tick={{ fill: 'var(--md-sys-color-on-surface-variant)', fontSize: 12 }}
                    dx={-10}
                  />
                  <YAxis
                    yAxisId="right"
                    orientation="right"
                    axisLine={false}
                    tickLine={false}
                    tick={{ fill: 'var(--md-sys-color-on-surface-variant)', fontSize: 12 }}
                    dx={10}
                  />
                  <Tooltip content={<CustomTooltip />} />
                  <Legend
                    wrapperStyle={{ paddingTop: '20px' }}
                    iconType="circle"
                  />
                  <Area
                    type="monotone"
                    dataKey="tokens"
                    name="Tokens"
                    stroke="var(--md-sys-color-primary)"
                    strokeWidth={2}
                    fillOpacity={1}
                    fill="url(#colorTokens)"
                    yAxisId="left"
                    dot={false}
                    activeDot={{ r: 6, strokeWidth: 2, fill: 'var(--md-sys-color-surface)' }}
                  />
                  <Area
                    type="monotone"
                    dataKey="price"
                    name="Spend ($)"
                    stroke="var(--md-sys-color-secondary)"
                    strokeWidth={2}
                    fillOpacity={1}
                    fill="url(#colorSpend)"
                    yAxisId="right"
                    dot={false}
                    activeDot={{ r: 6, strokeWidth: 2, fill: 'var(--md-sys-color-surface)' }}
                  />
                </AreaChart>
              </ResponsiveContainer>
            )}
            {!loading && !hasChartData && (
              <div className="chart-empty-overlay">
                <EmptyChart message="选定时间范围内暂无数据" />
              </div>
            )}
          </div>
        </div>

        {/* 渠道占比图 */}
        <div className="section chart-section">
          <div className="section-header">
            <div className="section-title-group">
              <h2>渠道费用占比</h2>
              {!hasChannelData && <span className="no-data-badge">暂无数据</span>}
            </div>
            <div className="chart-controls">
              <select
                value={channelRange}
                onChange={(e) => setChannelRange(e.target.value as RangeType)}
                className="chart-range-select"
              >
                <option value="daily">今日</option>
                <option value="weekly">近 7 天</option>
                <option value="monthly">近 30 天</option>
              </select>
            </div>
          </div>
          <div className="chart-container">
            <ResponsiveContainer width="100%" height={300}>
              <BarChart data={channelChartData} margin={{ top: 10, right: 30, left: 0, bottom: 0 }}>
                <CartesianGrid
                  strokeDasharray="3 3"
                  stroke="var(--md-sys-color-outline-variant)"
                  vertical={false}
                />
                <XAxis
                  dataKey="channel"
                  axisLine={false}
                  tickLine={false}
                  tick={{ fill: 'var(--md-sys-color-on-surface-variant)', fontSize: 12 }}
                  dy={10}
                />
                <YAxis
                  axisLine={false}
                  tickLine={false}
                  tick={{ fill: 'var(--md-sys-color-on-surface-variant)', fontSize: 12 }}
                  dx={-10}
                />
                <Tooltip content={<CustomTooltip />} />
                <Legend
                  wrapperStyle={{ paddingTop: '20px' }}
                  iconType="circle"
                />
                <Bar
                  dataKey="price_usd"
                  name="费用 (USD)"
                  fill="var(--md-sys-color-tertiary)"
                  radius={[6, 6, 0, 0]}
                  maxBarSize={60}
                />
                <Bar
                  dataKey="tokens"
                  name="Tokens"
                  fill="var(--md-sys-color-primary)"
                  radius={[6, 6, 0, 0]}
                  maxBarSize={60}
                />
              </BarChart>
            </ResponsiveContainer>
            {!hasChannelData && (
              <div className="chart-empty-overlay">
                <EmptyChart message="暂无渠道数据" />
              </div>
            )}
          </div>
        </div>
      </div>

      {/* 模型费用表格 */}
      <div className="section">
        <div className="section-header">
          <div className="section-title-group">
            <h2>模型费用明细</h2>
            <span className="muted">{models.length} 个模型</span>
          </div>
          <div className="actions">
            <select value={modelRange} onChange={(e) => setModelRange(e.target.value as RangeType)}>
              <option value="daily">今天</option>
              <option value="weekly">近 7 天</option>
              <option value="monthly">近 30 天</option>
            </select>
          </div>
        </div>
        <div className="table-container">
          <table>
            <thead>
              <tr>
                <th>模型名称</th>
                <th className="text-right">请求数</th>
                <th className="text-right">Tokens</th>
                <th className="text-right">费用 (USD)</th>
              </tr>
            </thead>
            <tbody>
              {models.length === 0 ? (
                <tr>
                  <td colSpan={4} className="table-empty">
                    <div className="table-empty-content">
                      <span className="table-empty-icon">📋</span>
                      <p>暂无模型数据</p>
                    </div>
                  </td>
                </tr>
              ) : (
                models.map((m, index) => (
                  <tr key={m.model} style={{ animationDelay: `${index * 50}ms` }} className="table-row-animate">
                    <td>
                      <span className="model-name">{m.model}</span>
                    </td>
                    <td className="text-right font-mono">{m.requests.toLocaleString()}</td>
                    <td className="text-right font-mono" title={m.tokens.toLocaleString()}>
                      {formatTokenCount(m.tokens)}
                    </td>
                    <td className="text-right font-mono">${m.price_usd.toFixed(4)}</td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* 全局日志 */}
      <div className="section">
        <div className="section-header">
          <div className="section-title-group">
            <h2>📋 系统日志</h2>
            <span className="muted">共 {logsTotal.toLocaleString()} 条记录</span>
          </div>
          <div className="actions" style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
            <select
              value={logsLevel}
              onChange={(e) => handleLogsFilterChange(e.target.value as LogLevel | '')}
              className="chart-range-select"
            >
              <option value="">全部级别</option>
              <option value="debug">Debug</option>
              <option value="info">Info</option>
              <option value="warn">Warn</option>
              <option value="error">Error</option>
            </select>
            <input
              type="text"
              placeholder="按来源筛选..."
              value={logsSource}
              onChange={(e) => handleLogsSourceChange(e.target.value)}
              className="log-source-input"
              style={{ padding: '6px 12px', borderRadius: '6px', border: '1px solid var(--border)', background: 'var(--bg-secondary)', fontSize: '14px' }}
            />
            <button
              className="secondary sm"
              onClick={handleLogsRefresh}
              disabled={logsLoading}
            >
              刷新
            </button>
            <button
              className="danger sm"
              onClick={handleClearLogs}
              disabled={logsLoading || logsTotal === 0}
            >
              清空
            </button>
          </div>
        </div>
        <div className="table-container logs-table">
          {logsLoading ? (
            <div className="chart-loading" style={{ minHeight: '200px' }}>
              <div className="loading-spinner"></div>
              <p>加载中...</p>
            </div>
          ) : (
            <table>
              <thead>
                <tr>
                  <th style={{ width: '140px' }}>时间</th>
                  <th style={{ width: '70px' }}>级别</th>
                  <th style={{ width: '100px' }}>来源</th>
                  <th>消息</th>
                  <th style={{ width: '60px' }}>操作</th>
                </tr>
              </thead>
              <tbody>
                {logs.length === 0 ? (
                  <tr>
                    <td colSpan={5} className="table-empty">
                      <div className="table-empty-content">
                        <span className="table-empty-icon">📋</span>
                        <p>暂无日志记录</p>
                      </div>
                    </td>
                  </tr>
                ) : (
                  logs.map((log, index) => {
                    const isExpanded = expandedLogIds.has(log.id);
                    const hasMetadata = !!log.metadata;

                    return (
                      <React.Fragment key={log.id}>
                        <tr
                          style={{ animationDelay: `${index * 30}ms` }}
                          className={`table-row-animate log-row ${isExpanded ? 'log-row-expanded' : ''}`}
                          onClick={() => hasMetadata && toggleLogExpand(log.id)}
                        >
                          <td className="log-time">
                            <span className="font-mono">{formatTimestamp(log.timestamp)}</span>
                          </td>
                          <td>
                            <span className={`log-level log-level-${log.level}`}>
                              {log.level.toUpperCase()}
                            </span>
                          </td>
                          <td>
                            <span className="log-source">{log.source}</span>
                          </td>
                          <td className="log-message-cell">
                            <div className="log-message-wrapper">
                              <span className="log-message" title={log.message}>
                                {log.message}
                              </span>
                              {hasMetadata && (
                                <button
                                  className="log-expand-btn"
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    toggleLogExpand(log.id);
                                  }}
                                  title={isExpanded ? "收起详情" : "展开详情"}
                                >
                                  {isExpanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
                                </button>
                              )}
                            </div>
                          </td>
                          <td>
                            <div className="log-actions">
                              <button
                                className="icon-btn"
                                onClick={(e) => {
                                  e.stopPropagation();
                                  openLogDetail(log);
                                }}
                                title="查看详情"
                              >
                                <Info size={14} />
                              </button>
                              <button
                                className="icon-btn danger"
                                onClick={(e) => {
                                  e.stopPropagation();
                                  handleDeleteLog(log.id);
                                }}
                                title="删除"
                              >
                                🗑️
                              </button>
                            </div>
                          </td>
                        </tr>
                        {isExpanded && hasMetadata && (
                          <tr className="log-detail-row">
                            <td colSpan={5}>
                              <div className="log-detail-content">
                                <div className="log-detail-header">
                                  <span>详细信息</span>
                                </div>
                                <pre className="log-detail-metadata">
                                  {(() => {
                                    const parsed = parseLogMetadata(log.metadata);
                                    if (typeof parsed === 'object') {
                                      return JSON.stringify(parsed, null, 2);
                                    }
                                    return parsed;
                                  })()}
                                </pre>
                              </div>
                            </td>
                          </tr>
                        )}
                      </React.Fragment>
                    );
                  })
                )}
              </tbody>
            </table>
          )}
        </div>
        {/* 分页控制 */}
        {logsTotal > logsPageSize && (
          <div className="pagination">
            <button
              className="secondary sm"
              onClick={() => setLogsPage((p) => Math.max(0, p - 1))}
              disabled={logsPage === 0 || logsLoading}
            >
              ← 上一页
            </button>
            <span className="pagination-info">
              第 {logsPage + 1} / {totalPages} 页
            </span>
            <button
              className="secondary sm"
              onClick={() => setLogsPage((p) => Math.min(totalPages - 1, p + 1))}
              disabled={logsPage >= totalPages - 1 || logsLoading}
            >
              下一页 →
            </button>
          </div>
        )}
      </div>

      {/* 日志详情弹窗 */}
      <Modal
        open={!!selectedLog}
        onClose={closeLogDetail}
        title="日志详情"
        size="lg"
      >
        {selectedLog && (
          <div className="log-detail-modal">
            <div className="log-detail-modal-header">
              <div className="log-detail-modal-meta">
                <span className={`log-level log-level-${selectedLog.level}`}>
                  {selectedLog.level.toUpperCase()}
                </span>
                <span className="log-source">{selectedLog.source}</span>
                <span className="log-time font-mono">
                  {new Date(selectedLog.timestamp * 1000).toLocaleString('zh-CN')}
                </span>
              </div>
            </div>

            <div className="log-detail-modal-section">
              <div className="log-detail-modal-label">消息内容</div>
              <div className="log-detail-modal-message">{selectedLog.message}</div>
            </div>

            {selectedLog.metadata && (
              <div className="log-detail-modal-section">
                <div className="log-detail-modal-label">详细信息 / 元数据</div>
                <pre className="log-detail-modal-metadata">
                  {(() => {
                    const parsed = parseLogMetadata(selectedLog.metadata);
                    if (typeof parsed === 'object') {
                      return JSON.stringify(parsed, null, 2);
                    }
                    return parsed;
                  })()}
                </pre>
              </div>
            )}

            <div className="log-detail-modal-footer">
              <button
                type="button"
                className="secondary"
                onClick={closeLogDetail}
              >
                关闭
              </button>
              <button
                type="button"
                className="danger"
                onClick={() => {
                  handleDeleteLog(selectedLog.id);
                  closeLogDetail();
                }}
              >
                删除此日志
              </button>
            </div>
          </div>
        )}
      </Modal>
    </div>
  );
}
