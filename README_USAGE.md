# CCR Bug 修复 - 完整使用指南

## 📋 修复概述

本次修复解决了 GLM 流式响应处理中的关键问题，并大幅增强了日志记录功能。

### 修复的问题
- ✅ GLM `reasoning_content` 字段导致的解析错误
- ✅ 流式响应返回空内容
- ✅ Rust 后端崩溃无日志
- ✅ 错误信息不完整，难以诊断

### 新增功能
- ✅ 全面的错误日志记录
- ✅ Panic 捕获和记录
- ✅ 详细的请求流程日志
- ✅ 日志查询和分析工具

---

## 🚀 快速开始

### 1. 重新构建应用

```bash
cd src-tauri
cargo build --release
```

### 2. 启动应用

```bash
# 开发模式
cargo run

# 或者运行编译后的程序
./target/release/tauri-app
```

### 3. 验证修复

```bash
# 运行诊断工具
python diagnostic.py

# 查看日志
python view_logs.py errors
```

---

## 📊 日志查看工具

### view_logs.py - 日志查看器

#### 基本用法

```bash
# 查看错误摘要
python view_logs.py errors

# 查看 Panic 日志
python view_logs.py panic

# 查看流式请求日志
python view_logs.py stream

# 查看所有日志（默认 50 条）
python view_logs.py all

# 查看更多日志
python view_logs.py all 100

# 实时跟踪日志
python view_logs.py follow
```

#### 按级别查看

```bash
python view_logs.py level error
python view_logs.py level warn
python view_logs.py level info
python view_logs.py level debug
```

#### 按来源查看

```bash
python view_logs.py source openai      # OpenAI handler 日志
python view_logs.py source middleware  # 中间件日志
python view_logs.py source panic       # Panic 日志
python view_logs.py source forward_error  # 错误响应日志
python view_logs.py source client      # HTTP 客户端日志
```

---

## 🔍 诊断工具

### diagnostic.py - 系统诊断

自动检查应用健康状况：

```bash
python diagnostic.py
```

**检查项目**：
- ✅ API 连接性
- ✅ 最近的错误
- ✅ Panic 日志
- ✅ 日志容量
- ✅ 流式请求错误
- ✅ GLM 支持状态

**输出示例**：
```
================================================================================
CCR Application Diagnostic Report
================================================================================
Time: 2026-01-18 14:08:47
================================================================================

API Connectivity:
--------------------------------------------------------------------------------
✅ API connectivity: OK

Recent Errors:
--------------------------------------------------------------------------------
✅ Recent errors: None found

Panic Logs:
--------------------------------------------------------------------------------
✅ Panic logs: None found

...

Summary:
✅ PASS - API Connectivity
✅ PASS - Recent Errors
✅ PASS - Panic Logs
...

Overall: 6/6 checks passed
✅ All checks passed! Application is healthy.
================================================================================
```

---

## 🧪 测试工具

### test_glm_stream.py - GLM 流式测试

测试 GLM 流式响应处理：

```bash
python test_glm_stream.py
```

**功能**：
- 发送 GLM 流式请求
- 检测 `reasoning_content` 字段
- 验证响应完整性
- 检查错误日志

---

## 📖 日志 API 使用

### 直接使用 HTTP API

#### 查询所有日志
```bash
curl "http://127.0.0.1:8787/api/logs?limit=50"
```

#### 按级别过滤
```bash
# 只看错误
curl "http://127.0.0.1:8787/api/logs?level=error&limit=50"

# 只看警告
curl "http://127.0.0.1:8787/api/logs?level=warn&limit=50"

# 只看信息
curl "http://127.0.0.1:8787/api/logs?level=info&limit=50"

# 只看调试
curl "http://127.0.0.1:8787/api/logs?level=debug&limit=50"
```

#### 按来源过滤
```bash
curl "http://127.0.0.1:8787/api/logs?source=openai&limit=100"
curl "http://127.0.0.1:8787/api/logs?source=middleware&limit=100"
curl "http://127.0.0.1:8787/api/logs?source=panic&limit=100"
```

#### 组合过滤
```bash
# 查看 OpenAI handler 的错误日志
curl "http://127.0.0.1:8787/api/logs?level=error&source=openai&limit=50"
```

#### 使用 jq 格式化输出
```bash
curl -s "http://127.0.0.1:8787/api/logs?limit=10" | jq '.'
```

---

## 🔧 常见问题排查

### 问题 1：流式响应返回空内容

**症状**：
- 请求成功但响应为空
- 客户端超时

**排查步骤**：
```bash
# 1. 查看流式请求日志
python view_logs.py stream

# 2. 查看 OpenAI handler 错误
python view_logs.py source openai

# 3. 检查是否有 JSON 解析错误
curl -s "http://127.0.0.1:8787/api/logs?source=openai&limit=100" | grep -i "parse"
```

**可能的原因**：
- Upstream API 返回格式不正确
- 网络连接问题
- API 密钥无效

---

### 问题 2：GLM reasoning_content 错误

**症状**：
- GLM 请求失败
- 日志中有 JSON 解析错误

**排查步骤**：
```bash
# 查看 JSON 解析错误
python view_logs.py source openai | grep -i "parse"

# 查看完整的错误日志
python view_logs.py level error
```

**修复后的行为**：
- ✅ `reasoning_content` 字段被正确处理
- ✅ Token 正确计数
- ✅ 不会导致解析错误

---

### 问题 3：应用崩溃无日志

**症状**：
- 应用突然退出
- 没有错误信息

**排查步骤**：
```bash
# 1. 查看 Panic 日志
python view_logs.py panic

# 2. 查看最近的错误
python view_logs.py errors

# 3. 启用 backtrace 重新运行
RUST_BACKTRACE=1 cargo run
```

**修复后的行为**：
- ✅ 所有 Panic 都会被记录
- ✅ 包含详细的错误位置
- ✅ 应用不会无声崩溃

---

### 问题 4：找不到特定错误

**解决方法**：
```bash
# 实时跟踪日志
python view_logs.py follow

# 在另一个终端发送请求
curl -X POST http://127.0.0.1:8787/v1/chat/completions ...

# 查看特定时间段的日志
curl "http://127.0.0.1:8787/api/logs?since=1h&limit=100"
```

---

## 📈 性能监控

### 监控错误率

```bash
# 每 5 秒检查一次错误数量
while true; do
  echo "=== $(date) ==="
  ERROR_COUNT=$(curl -s "http://127.0.0.1:8787/api/logs?level=error&limit=100" | python -c "import sys, json; print(len(json.load(sys.stdin)['logs']))")
  echo "Error count: $ERROR_COUNT"
  sleep 5
done
```

### 监控特定错误

```bash
# 监控 JSON 解析错误
while true; do
  curl -s "http://127.0.0.1:8787/api/logs?source=openai&limit=50" | grep -i "parse" | tail -5
  sleep 2
done
```

### 导出日志分析

```bash
# 导出最近的错误日志
curl -s "http://127.0.0.1:8787/api/logs?level=error&limit=1000" > errors_$(date +%Y%m%d_%H%M%S).json

# 使用 jq 分析
cat errors_*.json | jq '.logs[] | {source: .source, message: .message}' | less
```

---

## 🎯 最佳实践

### 1. 日志级别设置

**开发环境**：
```bash
# 启用 DEBUG 级别
RUST_LOG=debug cargo run
```

**生产环境**：
```bash
# 使用 INFO 级别（默认）
cargo run --release
```

### 2. 日志查看习惯

**每日检查**：
```bash
# 查看昨天的错误
python view_logs.py errors

# 检查是否有 Panic
python view_logs.py panic
```

**问题排查**：
```bash
# 1. 先看错误摘要
python view_logs.py errors

# 2. 查看特定来源
python view_logs.py source <source_name>

# 3. 实时跟踪
python view_logs.py follow
```

### 3. 日志清理

定期清理旧日志（建议）：
```sql
-- 连接到 SQLite 数据库
sqlite3 ~/.local/share/CCR/ccr.db  # Linux/Mac
sqlite3 %APPDATA%\CCR\ccr.db       # Windows

-- 删除 30 天前的 DEBUG 日志
DELETE FROM global_logs
WHERE level = 'debug'
AND timestamp < strftime('%s', 'now', '-30 days');

-- 删除 90 天前的 INFO 日志
DELETE FROM global_logs
WHERE level = 'info'
AND timestamp < strftime('%s', 'now', '-90 days');

-- 保留所有 ERROR 日志
```

---

## 📚 文档索引

### 核心文档
- **BUGFIX_SUMMARY.md** - 详细的修复说明
- **TESTING_GUIDE.md** - 测试和验证指南
- **VERIFICATION_REPORT.md** - 验证报告
- **README_USAGE.md** - 本文档

### 工具脚本
- **view_logs.py** - 日志查看工具
- **diagnostic.py** - 系统诊断工具
- **test_glm_stream.py** - GLM 测试脚本

---

## 🔄 更新日志

### 2026-01-18 - v1.0.0
- ✅ 添加 GLM `reasoning_content` 支持
- ✅ 实现全面的错误日志记录
- ✅ 添加 Panic 捕获机制
- ✅ 增强中间件错误日志
- ✅ 创建日志查看和诊断工具

---

## 💡 提示和技巧

### 快速诊断流程

1. **运行诊断工具**
   ```bash
   python diagnostic.py
   ```

2. **如果有错误，查看详情**
   ```bash
   python view_logs.py errors
   ```

3. **实时监控**
   ```bash
   python view_logs.py follow
   ```

### 调试技巧

1. **启用详细日志**
   ```bash
   RUST_LOG=debug cargo run
   ```

2. **启用 Backtrace**
   ```bash
   RUST_BACKTRACE=1 cargo run
   ```

3. **使用 jq 分析 JSON**
   ```bash
   curl -s "http://127.0.0.1:8787/api/logs?limit=100" | jq '.logs[] | select(.level=="error")'
   ```

---

## 🆘 获取帮助

### 查看日志来源列表
```bash
curl -s "http://127.0.0.1:8787/api/logs?limit=1000" | jq '.logs[].source' | sort -u
```

### 查看日志级别分布
```bash
curl -s "http://127.0.0.1:8787/api/logs?limit=1000" | jq '.logs[].level' | sort | uniq -c
```

### 查找特定关键词
```bash
curl -s "http://127.0.0.1:8787/api/logs?limit=1000" | jq '.logs[] | select(.message | contains("stream"))'
```

---

## ✅ 验证清单

部署前请确认：

- [ ] 应用成功编译
- [ ] 诊断工具显示健康状态
- [ ] 没有 Panic 日志
- [ ] 错误日志在可接受范围内
- [ ] 流式请求正常工作
- [ ] GLM `reasoning_content` 正确处理
- [ ] 日志查询 API 正常
- [ ] 工具脚本可以运行

---

## 📞 支持

如果遇到问题：

1. 运行诊断工具：`python diagnostic.py`
2. 查看错误日志：`python view_logs.py errors`
3. 查看完整文档：`BUGFIX_SUMMARY.md`
4. 查看测试指南：`TESTING_GUIDE.md`

---

**文档版本**: 1.0.0
**最后更新**: 2026-01-18
**作者**: Claude Code
