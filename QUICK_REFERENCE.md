# 🚀 CCR 日志系统 - 快速参考卡片

## 📋 常用命令速查

### 日志查看
```bash
# 查看错误摘要（最常用）
python view_logs.py errors

# 实时跟踪日志
python view_logs.py follow

# 查看流式请求日志
python view_logs.py stream

# 查看 Panic 日志
python view_logs.py panic

# 查看所有日志
python view_logs.py all 50
```

### 按来源查看
```bash
python view_logs.py source openai       # OpenAI handler
python view_logs.py source middleware   # 中间件
python view_logs.py source panic        # Panic
python view_logs.py source forward_error # 错误响应
python view_logs.py source client       # HTTP 客户端
```

### 按级别查看
```bash
python view_logs.py level error   # 只看错误
python view_logs.py level warn    # 只看警告
python view_logs.py level info    # 只看信息
python view_logs.py level debug   # 只看调试
```

### 系统诊断
```bash
python diagnostic.py   # 运行完整诊断
```

---

## 🔍 问题排查流程

### 1. 应用无法启动
```bash
# 检查端口占用
netstat -ano | findstr "8787"

# 查看启动日志
python view_logs.py source app

# 查看 Panic 日志
python view_logs.py panic
```

### 2. 请求失败
```bash
# 1. 运行诊断
python diagnostic.py

# 2. 查看错误
python view_logs.py errors

# 3. 查看中间件日志
python view_logs.py source middleware

# 4. 查看 handler 日志
python view_logs.py source openai
```

### 3. 流式响应问题
```bash
# 查看流式日志
python view_logs.py stream

# 查看 JSON 解析错误
curl -s "http://127.0.0.1:8787/api/logs?source=openai&limit=100" | grep -i "parse"

# 实时监控
python view_logs.py follow
```

### 4. GLM 相关问题
```bash
# 测试 GLM 流式请求
python test_glm_stream.py

# 查看 GLM 相关日志
curl -s "http://127.0.0.1:8787/api/logs?source=openai&limit=100" | grep -i "reasoning"
```

---

## 🌐 日志 API 速查

### 基本查询
```bash
# 查询最近 50 条日志
curl "http://127.0.0.1:8787/api/logs?limit=50"

# 查询错误日志
curl "http://127.0.0.1:8787/api/logs?level=error&limit=50"

# 查询特定来源
curl "http://127.0.0.1:8787/api/logs?source=openai&limit=100"

# 组合查询
curl "http://127.0.0.1:8787/api/logs?level=error&source=openai&limit=50"
```

### 使用 jq 格式化
```bash
# 格式化输出
curl -s "http://127.0.0.1:8787/api/logs?limit=10" | jq '.'

# 只看错误消息
curl -s "http://127.0.0.1:8787/api/logs?level=error&limit=50" | jq '.logs[] | .message'

# 按来源分组
curl -s "http://127.0.0.1:8787/api/logs?limit=100" | jq '.logs | group_by(.source) | map({source: .[0].source, count: length})'
```

---

## 📊 日志来源说明

| 来源 | 说明 | 常见日志 |
|------|------|---------|
| `app` | 应用启动/关闭 | Application started |
| `middleware` | 请求中间件 | Model lookup, Authentication |
| `openai` | OpenAI handler | Stream request, JSON parse |
| `forward_error` | 错误响应 | Returning error response |
| `client` | HTTP 客户端 | Request failed, Retry |
| `panic` | Rust panic | PANIC occurred |

---

## 🎯 日志级别说明

| 级别 | 用途 | 示例 |
|------|------|------|
| `ERROR` | 错误情况 | 请求失败、解析错误 |
| `WARN` | 警告情况 | 配置问题、性能警告 |
| `INFO` | 重要信息 | 请求完成、上下文构建 |
| `DEBUG` | 调试信息 | 详细的处理流程 |

---

## 🔧 常见错误及解决方案

### 错误 1: Model not found
```
[ERROR] [middleware] Model lookup failed: model_id='xxx', error=Model not found
```
**解决**: 检查配置文件中的模型配置

### 错误 2: Upstream not found
```
[ERROR] [middleware] Upstream lookup failed: upstream_id='xxx', error=Upstream not found
```
**解决**: 检查配置文件中的 upstream 配置

### 错误 3: Authentication failed
```
[ERROR] [middleware] Authentication failed: Unauthorized: Missing authentication token
```
**解决**: 检查请求头中的 Authorization 或 x-ccr-forward-token

### 错误 4: JSON parse error
```
[ERROR] [openai] Failed to parse SSE JSON chunk: error=xxx, data=xxx
```
**解决**: 检查 upstream API 返回格式，查看完整错误日志

### 错误 5: Request failed
```
[ERROR] [client] Request failed: error sending request for url (xxx)
```
**解决**: 检查网络连接、API 密钥、upstream 地址

---

## 💡 实用技巧

### 1. 监控错误率
```bash
# 每 5 秒检查一次错误数量
while true; do
  echo "=== $(date) ==="
  python view_logs.py errors | grep "Found" | head -1
  sleep 5
done
```

### 2. 导出日志分析
```bash
# 导出最近的错误日志
curl -s "http://127.0.0.1:8787/api/logs?level=error&limit=1000" > errors_$(date +%Y%m%d_%H%M%S).json

# 分析错误分布
cat errors_*.json | jq '.logs[] | .source' | sort | uniq -c
```

### 3. 查找特定关键词
```bash
# 查找包含 "stream" 的日志
curl -s "http://127.0.0.1:8787/api/logs?limit=1000" | jq '.logs[] | select(.message | contains("stream"))'

# 查找包含 "parse" 的错误
curl -s "http://127.0.0.1:8787/api/logs?level=error&limit=100" | jq '.logs[] | select(.message | contains("parse"))'
```

### 4. 实时监控特定错误
```bash
# 监控 JSON 解析错误
while true; do
  curl -s "http://127.0.0.1:8787/api/logs?source=openai&limit=10" | jq '.logs[] | select(.message | contains("parse")) | .message'
  sleep 2
done
```

---

## 🚨 紧急情况处理

### 应用崩溃
```bash
# 1. 查看 Panic 日志
python view_logs.py panic

# 2. 查看最近的错误
python view_logs.py errors

# 3. 启用 backtrace 重启
RUST_BACKTRACE=1 cargo run
```

### 大量错误
```bash
# 1. 运行诊断
python diagnostic.py

# 2. 查看错误分布
curl -s "http://127.0.0.1:8787/api/logs?level=error&limit=100" | jq '.logs[] | .source' | sort | uniq -c

# 3. 查看最新错误
python view_logs.py errors
```

### 性能问题
```bash
# 1. 检查日志量
curl -s "http://127.0.0.1:8787/api/logs?limit=1" | jq '.total'

# 2. 查看警告日志
python view_logs.py level warn

# 3. 检查是否有大量 DEBUG 日志
curl -s "http://127.0.0.1:8787/api/logs?level=debug&limit=1" | jq '.total'
```

---

## 📱 快捷键（Windows）

### PowerShell 别名设置
```powershell
# 添加到 PowerShell 配置文件
function Show-Errors { python view_logs.py errors }
function Show-Logs { python view_logs.py follow }
function Run-Diagnostic { python diagnostic.py }

Set-Alias -Name logs-errors -Value Show-Errors
Set-Alias -Name logs-follow -Value Show-Logs
Set-Alias -Name logs-check -Value Run-Diagnostic
```

使用：
```powershell
logs-errors   # 查看错误
logs-follow   # 实时跟踪
logs-check    # 运行诊断
```

---

## 📚 文档索引

| 文档 | 用途 |
|------|------|
| `COMPLETION_SUMMARY.md` | 修复完成总结 |
| `BUGFIX_SUMMARY.md` | 详细修复说明 |
| `TESTING_GUIDE.md` | 测试指南 |
| `VERIFICATION_REPORT.md` | 验证报告 |
| `README_USAGE.md` | 使用指南 |
| `QUICK_REFERENCE.md` | 本快速参考 |

---

## ✅ 每日检查清单

### 早上检查
- [ ] 运行诊断工具：`python diagnostic.py`
- [ ] 查看昨天的错误：`python view_logs.py errors`
- [ ] 检查 Panic 日志：`python view_logs.py panic`

### 部署前检查
- [ ] 编译成功：`cargo build --release`
- [ ] 诊断通过：`python diagnostic.py`
- [ ] 无 Panic 日志
- [ ] 错误日志正常

### 问题排查
- [ ] 运行诊断：`python diagnostic.py`
- [ ] 查看错误：`python view_logs.py errors`
- [ ] 实时监控：`python view_logs.py follow`
- [ ] 查看特定来源日志

---

## 🎓 记住这些

### 最常用的 3 个命令
```bash
python diagnostic.py              # 1. 诊断
python view_logs.py errors        # 2. 查看错误
python view_logs.py follow        # 3. 实时监控
```

### 最重要的 3 个日志来源
- `middleware` - 请求处理流程
- `openai` - 流式响应处理
- `forward_error` - 错误响应

### 最关键的 3 个检查
1. 有没有 Panic？
2. 有没有错误？
3. 流式请求正常吗？

---

**打印此卡片，放在手边！** 📌

**版本**: 1.0.0
**更新**: 2026-01-18
