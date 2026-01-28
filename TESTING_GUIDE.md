# 测试和验证指南

## 快速开始

### 1. 启动应用
```bash
cd src-tauri
cargo run
```

### 2. 查看日志

#### 查看错误摘要
```bash
python view_logs.py errors
```

#### 查看 Panic 日志
```bash
python view_logs.py panic
```

#### 查看流式请求日志
```bash
python view_logs.py stream
```

#### 实时跟踪日志
```bash
python view_logs.py follow
```

#### 查看特定级别的日志
```bash
python view_logs.py level error
python view_logs.py level info
python view_logs.py level debug
```

#### 查看特定来源的日志
```bash
python view_logs.py source openai
python view_logs.py source middleware
python view_logs.py source panic
python view_logs.py source forward_error
```

---

## 测试场景

### 场景 1：测试 GLM 流式请求

**目的**：验证 GLM `reasoning_content` 字段是否被正确处理

**步骤**：
1. 确保你的配置中有 GLM 模型配置
2. 运行测试脚本：
   ```bash
   python test_glm_stream.py
   ```
3. 查看日志：
   ```bash
   python view_logs.py stream
   ```

**预期结果**：
- ✅ 请求成功处理
- ✅ 日志中显示 "Starting stream request"
- ✅ 日志中显示 "Stream completed" 并包含 token 统计
- ✅ 没有 JSON 解析错误
- ✅ `reasoning_content` 字段的 token 被正确计数

---

### 场景 2：测试错误日志记录

**目的**：验证各种错误情况是否被正确记录

#### 2.1 测试无效模型
```bash
curl -X POST http://127.0.0.1:8787/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer test-token" \
  -d '{
    "model": "non-existent-model",
    "messages": [{"role": "user", "content": "Hello"}]
  }'
```

**预期日志**：
```
[ERROR] [middleware     ] Model lookup failed: model_id='non-existent-model', error=Model 'non-existent-model' not configured
[ERROR] [forward_error  ] Returning error response: status=404, type=model_not_found, message=Model 'non-existent-model' not configured
```

#### 2.2 测试缺少认证
```bash
curl -X POST http://127.0.0.1:8787/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "glm-4-plus",
    "messages": [{"role": "user", "content": "Hello"}]
  }'
```

**预期日志**（如果配置了 forward_token）：
```
[ERROR] [middleware     ] Authentication failed: Unauthorized: Missing authentication token
[ERROR] [forward_error  ] Returning error response: status=401, type=unauthorized, message=Missing authentication token
```

#### 2.3 测试无效 JSON
```bash
curl -X POST http://127.0.0.1:8787/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer test-token" \
  -d 'invalid json'
```

**预期日志**：
```
[ERROR] [middleware     ] Failed to extract model ID from payload: Invalid request: Missing or empty 'model' field
```

---

### 场景 3：测试 Panic 捕获

**目的**：验证 Rust panic 是否被捕获并记录

**注意**：这个测试需要你故意触发一个 panic（仅用于测试）

如果你想测试 panic 捕获，可以临时添加一个测试端点：

```rust
// 在 src-tauri/src/server.rs 中添加测试端点
async fn test_panic() -> impl IntoResponse {
    panic!("This is a test panic!");
}

// 在路由中添加
.route("/test/panic", get(test_panic))
```

然后访问：
```bash
curl http://127.0.0.1:8787/test/panic
```

**预期日志**：
```
[ERROR] [panic          ] PANIC occurred: message='This is a test panic!', location='src/server.rs:123:5', backtrace available via RUST_BACKTRACE=1
```

---

### 场景 4：测试完整的请求流程日志

**目的**：验证完整的请求处理流程是否有日志记录

**步骤**：
1. 清空或标记当前日志位置
2. 发送一个正常请求：
   ```bash
   curl -X POST http://127.0.0.1:8787/v1/chat/completions \
     -H "Content-Type: application/json" \
     -H "Authorization: Bearer your-token" \
     -d '{
       "model": "glm-4-plus",
       "messages": [{"role": "user", "content": "Hello"}],
       "stream": true
     }'
   ```
3. 查看日志：
   ```bash
   python view_logs.py source middleware
   python view_logs.py source openai
   ```

**预期日志流程**：
```
[DEBUG] [middleware     ] Building context for model: glm-4-plus
[DEBUG] [middleware     ] Looking up model: glm-4-plus
[DEBUG] [middleware     ] Found model: glm-4-plus (priority: 100, upstream: zai)
[INFO ] [middleware     ] Context built: model=glm-4-plus, upstream=zai, provider=openai, streaming=true
[INFO ] [openai         ] Starting stream request: model=glm-4-plus, upstream=zai, url=https://...
[DEBUG] [openai         ] Stream response status: 200
[INFO ] [openai         ] Stream completed: model=glm-4-plus, tokens=10/50
```

---

## 日志 API 使用

### 查询所有日志
```bash
curl "http://127.0.0.1:8787/api/logs?limit=50"
```

### 按级别过滤
```bash
curl "http://127.0.0.1:8787/api/logs?level=error&limit=50"
curl "http://127.0.0.1:8787/api/logs?level=warn&limit=50"
curl "http://127.0.0.1:8787/api/logs?level=info&limit=50"
curl "http://127.0.0.1:8787/api/logs?level=debug&limit=50"
```

### 按来源过滤
```bash
curl "http://127.0.0.1:8787/api/logs?source=openai&limit=100"
curl "http://127.0.0.1:8787/api/logs?source=middleware&limit=100"
curl "http://127.0.0.1:8787/api/logs?source=panic&limit=100"
curl "http://127.0.0.1:8787/api/logs?source=forward_error&limit=100"
```

### 按时间范围过滤
```bash
# 最近 1 小时
curl "http://127.0.0.1:8787/api/logs?since=1h&limit=100"

# 最近 24 小时
curl "http://127.0.0.1:8787/api/logs?since=24h&limit=100"
```

### 组合过滤
```bash
curl "http://127.0.0.1:8787/api/logs?level=error&source=openai&limit=50"
```

---

## 常见问题排查

### 问题 1：看不到日志

**可能原因**：
1. 应用没有启动
2. 日志 API 端口不对
3. 日志级别过滤太严格

**解决方法**：
```bash
# 检查应用是否运行
curl http://127.0.0.1:8787/api/stats

# 查看所有级别的日志
python view_logs.py all 100

# 检查数据库文件
# Windows: %APPDATA%\CCR\ccr.db
# Linux/Mac: ~/.local/share/CCR/ccr.db
```

---

### 问题 2：日志太多

**解决方法**：
```bash
# 只看错误
python view_logs.py errors

# 只看特定来源
python view_logs.py source openai

# 限制数量
python view_logs.py all 10
```

---

### 问题 3：找不到特定错误

**解决方法**：
```bash
# 查看所有错误
python view_logs.py level error

# 查看特定来源的所有日志
python view_logs.py source middleware

# 实时跟踪
python view_logs.py follow
```

---

## 性能监控

### 监控错误率
```bash
# 每 5 秒检查一次错误数量
while true; do
  echo "=== $(date) ==="
  curl -s "http://127.0.0.1:8787/api/logs?level=error&limit=10" | python -m json.tool | grep -c "id"
  sleep 5
done
```

### 监控特定错误
```bash
# 监控 JSON 解析错误
while true; do
  curl -s "http://127.0.0.1:8787/api/logs?source=openai&limit=50" | grep "parse"
  sleep 2
done
```

---

## 调试技巧

### 1. 启用详细日志
在开发环境中，可以设置环境变量：
```bash
RUST_LOG=debug cargo run
```

### 2. 启用 Backtrace
如果遇到 panic：
```bash
RUST_BACKTRACE=1 cargo run
```

### 3. 使用 jq 格式化 JSON
```bash
curl -s "http://127.0.0.1:8787/api/logs?limit=10" | jq '.'
```

### 4. 导出日志到文件
```bash
curl -s "http://127.0.0.1:8787/api/logs?level=error&limit=1000" > errors.json
```

---

## 验证清单

在部署到生产环境前，请确认：

- [ ] 所有测试场景都通过
- [ ] 错误日志包含足够的上下文信息
- [ ] Panic 能被正确捕获和记录
- [ ] 流式请求的完整流程有日志
- [ ] JSON 解析错误被正确记录
- [ ] GLM `reasoning_content` 字段被正确处理
- [ ] 日志查询 API 正常工作
- [ ] 日志不会影响性能（异步写入）
- [ ] 日志文件大小在可控范围内

---

## 下一步

1. **生产环境部署**：
   - 确保日志级别设置合理（生产环境建议 INFO）
   - 配置日志轮转或清理策略
   - 设置监控告警

2. **持续改进**：
   - 根据实际使用情况调整日志内容
   - 添加更多有用的上下文信息
   - 优化日志性能

3. **文档更新**：
   - 更新用户文档，说明如何查看日志
   - 添加常见问题的排查指南
   - 记录日志格式和字段说明
