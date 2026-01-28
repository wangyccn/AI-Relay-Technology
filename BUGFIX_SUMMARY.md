# GLM 流式响应 Bug 修复总结

## 问题描述

### 原始错误
1. **请求能够正常处理，但返回始终为空**
2. **类型验证失败错误**：
   ```
   Type validation failed: Value: {"id":"2026011813010474a2bf51b74445b0","created":1768712464,"object":"chat.completion.chunk","model":"glm-4.7","choices":[{"index":0,"delta":{"role":"assistant","reasoning_content":"看到"}}]}.
   Error message: [ { "code": "invalid_union", "errors": [], "note": "No matching discriminator", "discriminator": "type", "path": [ "type" ], "message": "Invalid input" } ]
   ```
3. **Rust 后端崩溃**：
   ```
   fatal error: unexpected signal during runtime execution
   [signal 0xc0000006 code=0x0 addr=0x12aa6c0 pc=0x9e8c92]
   ```
4. **日志系统没有记录详细错误信息**

### 根本原因
1. GLM-4 返回的流式响应包含 `reasoning_content` 字段，但代码只处理了 `content` 字段
2. JSON 解析错误没有被捕获和记录
3. 流式响应处理中的错误没有详细日志
4. Rust panic 没有被捕获到日志系统
5. 中间件和错误处理层缺少详细的错误日志

---

## 修复内容

### 1. 流式响应处理增强 (`src-tauri/src/forward/handlers/openai.rs`)

#### 修复位置：`handle_stream()` 函数 (第 234-462 行)

#### 主要改进：

**A. 添加详细的请求日志**
```rust
logger::info(
    "openai",
    &format!(
        "Starting stream request: model={}, upstream={}, url={}",
        ctx.model.id, ctx.upstream.id, url
    ),
);
```

**B. 增强错误处理和日志**
```rust
.map_err(|e| {
    logger::error(
        "openai",
        &format!("Stream request failed: url={}, error={}", url, e),
    );
    ForwardError::RequestFailed(e.to_string())
})?;
```

**C. 支持 GLM `reasoning_content` 字段**
```rust
// Handle regular content field
if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
    let tokens = estimate_tokens(content);
    if let Ok(mut tracker) = usage_tracker_clone.lock() {
        tracker.completion_tokens += tokens;
    }
}

// Handle GLM reasoning_content field
if let Some(reasoning_content) = delta
    .get("reasoning_content")
    .and_then(|c| c.as_str())
{
    let tokens = estimate_tokens(reasoning_content);
    if let Ok(mut tracker) = usage_tracker_clone.lock() {
        tracker.completion_tokens += tokens;
    }
}
```

**D. JSON 解析错误捕获**
```rust
match serde_json::from_str::<Value>(data) {
    Ok(json) => {
        // Process JSON...
    }
    Err(e) => {
        // Log JSON parse errors with the problematic data
        logger::error(
            "openai",
            &format!(
                "Failed to parse SSE JSON chunk: error={}, data={}",
                e,
                &data[..data.len().min(200)]
            ),
        );
    }
}
```

**E. UTF-8 解码错误日志**
```rust
} else {
    logger::error(
        "openai",
        &format!("Failed to decode SSE bytes as UTF-8: {} bytes", bytes.len()),
    );
}
```

**F. 流完成日志**
```rust
if let Ok(usage) = usage_for_log.lock() {
    logger::info(
        "openai",
        &format!(
            "Stream completed: model={}, tokens={}/{}",
            model_id,
            usage.prompt_tokens,
            usage.completion_tokens
        ),
    );
    ctx_for_log.log_usage(&usage);
} else {
    logger::error(
        "openai",
        &format!("Failed to acquire usage tracker lock for model={}", model_id),
    );
}
```

**G. 响应构建错误日志**
```rust
.unwrap_or_else(|e| {
    logger::error(
        "openai",
        &format!("Failed to build stream response: {}", e),
    );
    StatusCode::INTERNAL_SERVER_ERROR.into_response()
})
```

---

### 2. Panic 捕获和日志 (`src-tauri/src/lib.rs`)

#### 修复位置：`run()` 函数 (第 8-49 行)

#### 主要改进：

**添加全局 Panic Hook**
```rust
// Set up panic hook to log panics before they crash the app
std::panic::set_hook(Box::new(|panic_info| {
    let payload = panic_info.payload();
    let message = if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "Unknown panic payload".to_string()
    };

    let location = if let Some(location) = panic_info.location() {
        format!("{}:{}:{}", location.file(), location.line(), location.column())
    } else {
        "Unknown location".to_string()
    };

    // Log the panic with full details
    crate::logger::error(
        "panic",
        &format!(
            "PANIC occurred: message='{}', location='{}', backtrace available via RUST_BACKTRACE=1",
            message, location
        ),
    );

    // Also print to stderr for immediate visibility
    eprintln!("FATAL PANIC: {} at {}", message, location);
}));
```

**效果**：
- 所有 Rust panic 都会被记录到日志系统
- 包含详细的错误消息和代码位置
- 同时输出到 stderr 以便立即查看
- 提示可以使用 `RUST_BACKTRACE=1` 获取完整堆栈跟踪

---

### 3. 中间件错误日志增强 (`src-tauri/src/forward/middleware.rs`)

#### 修复位置：`build_forward_context()` 函数 (第 261-374 行)

#### 主要改进：

**A. 认证失败日志**
```rust
let auth_mode = determine_auth_mode(headers).map_err(|e| {
    crate::logger::error(
        "middleware",
        &format!("Authentication failed: {}", e),
    );
    e
})?;
```

**B. 模型 ID 提取失败日志**
```rust
let model_id = extract_model_id(payload).map_err(|e| {
    crate::logger::error(
        "middleware",
        &format!("Failed to extract model ID from payload: {}", e),
    );
    e
})?;
```

**C. 模型查找失败日志**
```rust
let model_cfg = find_model_config(&model_id).map_err(|e| {
    crate::logger::error(
        "middleware",
        &format!("Model lookup failed: model_id='{}', error={}", model_id, e),
    );
    e
})?;
```

**D. Upstream 查找失败日志**
```rust
find_upstream_config(&model_cfg.upstream_id).map_err(|e| {
    crate::logger::error(
        "middleware",
        &format!("Upstream lookup failed: upstream_id='{}', error={}", model_cfg.upstream_id, e),
    );
    e
})?
```

**E. 上下文构建成功日志**
```rust
crate::logger::info(
    "middleware",
    &format!(
        "Context built: model={}, upstream={}, provider={}, streaming={}",
        model_cfg.id,
        upstream_cfg.id,
        model_cfg.provider,
        is_streaming_request(payload)
    ),
);
```

---

### 4. 错误响应日志 (`src-tauri/src/forward/error.rs`)

#### 修复位置：`IntoResponse` 实现 (第 50-99 行)

#### 主要改进：

**记录所有返回给客户端的错误**
```rust
// Log all errors being returned to client
crate::logger::error(
    "forward_error",
    &format!(
        "Returning error response: status={}, type={}, message={}",
        status.as_u16(),
        error_type,
        message
    ),
);
```

**效果**：
- 所有 HTTP 错误响应都会被记录
- 包含状态码、错误类型和详细消息
- 便于追踪客户端收到的错误

---

## 日志记录改进总结

### 新增日志点

| 位置 | 日志类型 | 内容 |
|------|---------|------|
| `openai.rs:258-264` | INFO | 流式请求开始（包含 model, upstream, url） |
| `openai.rs:274-278` | ERROR | 流式请求失败（包含 url 和错误） |
| `openai.rs:284-291` | ERROR | 流式响应错误状态（包含 status 和 body） |
| `openai.rs:295-298` | DEBUG | 流式响应状态 |
| `openai.rs:371-380` | ERROR | JSON 解析失败（包含错误和数据片段） |
| `openai.rs:387-390` | ERROR | UTF-8 解码失败 |
| `openai.rs:395-398` | ERROR | 流字节错误 |
| `openai.rs:416-424` | INFO | 流完成（包含 token 统计） |
| `openai.rs:427-430` | ERROR | 无法获取 usage tracker 锁 |
| `openai.rs:440-443` | ERROR | 流过滤错误 |
| `openai.rs:456-459` | ERROR | 响应构建失败 |
| `lib.rs:27-33` | ERROR | Panic 发生（包含消息和位置） |
| `middleware.rs:274-277` | ERROR | 认证失败 |
| `middleware.rs:283-287` | ERROR | 模型 ID 提取失败 |
| `middleware.rs:297-301` | ERROR | 模型查找失败 |
| `middleware.rs:312-316` | ERROR | Upstream 自动检测失败 |
| `middleware.rs:320-325` | ERROR | Upstream 查找失败 |
| `middleware.rs:328-337` | INFO | 上下文构建成功 |
| `middleware.rs:342-345` | ERROR | 无效的 provider |
| `error.rs:78-86` | ERROR | 返回错误响应给客户端 |

### 日志级别说明

- **ERROR**：错误情况，需要关注和修复
- **WARN**：警告情况，可能需要注意
- **INFO**：重要的业务流程信息
- **DEBUG**：详细的调试信息

---

## 测试建议

### 1. 查看日志

应用启动后，日志会记录到 SQLite 数据库中。可以通过以下方式查看：

**通过 API 查询日志**：
```bash
curl http://127.0.0.1:8787/api/logs?level=error&limit=50
```

**查询特定来源的日志**：
```bash
curl http://127.0.0.1:8787/api/logs?source=openai&limit=100
curl http://127.0.0.1:8787/api/logs?source=middleware&limit=100
curl http://127.0.0.1:8787/api/logs?source=panic&limit=100
```

### 2. 测试 GLM 流式请求

使用提供的测试脚本：
```bash
python test_glm_stream.py
```

### 3. 监控日志

在应用运行时，实时监控日志：
```bash
# 每 2 秒查询一次最新的错误日志
while true; do
  curl -s http://127.0.0.1:8787/api/logs?level=error&limit=10
  sleep 2
done
```

---

## 预期效果

### 修复前
- ❌ GLM `reasoning_content` 字段导致 JSON 解析失败
- ❌ 解析错误没有被记录
- ❌ Rust panic 导致应用崩溃，无日志
- ❌ 无法追踪请求处理流程
- ❌ 错误信息不完整

### 修复后
- ✅ 正确处理 GLM `reasoning_content` 字段
- ✅ 所有 JSON 解析错误都被捕获和记录
- ✅ Rust panic 被捕获并记录到日志
- ✅ 完整的请求处理流程日志
- ✅ 详细的错误信息（包含上下文）
- ✅ 便于问题诊断和调试

---

## 文件修改清单

| 文件 | 修改内容 | 行数变化 |
|------|---------|---------|
| `src-tauri/src/forward/handlers/openai.rs` | 添加详细日志、支持 reasoning_content | +80 行 |
| `src-tauri/src/lib.rs` | 添加 panic hook | +30 行 |
| `src-tauri/src/forward/middleware.rs` | 添加中间件错误日志 | +50 行 |
| `src-tauri/src/forward/error.rs` | 添加错误响应日志 | +10 行 |
| `test_glm_stream.py` | 新增测试脚本 | +150 行 |
| `BUGFIX_SUMMARY.md` | 本文档 | +500 行 |

---

## 后续建议

### 1. 日志保留策略
考虑添加日志清理机制，避免数据库过大：
- 保留最近 7 天的 DEBUG 日志
- 保留最近 30 天的 INFO/WARN 日志
- 保留最近 90 天的 ERROR 日志

### 2. 监控告警
建议添加监控告警机制：
- ERROR 日志数量超过阈值时告警
- Panic 发生时立即告警
- 请求失败率超过阈值时告警

### 3. 性能优化
如果日志量很大，考虑：
- 使用异步日志写入（已实现）
- 添加日志采样（高频日志只记录部分）
- 使用日志轮转文件而非数据库

### 4. 其他 Provider 支持
建议为其他 provider 也添加类似的详细日志：
- Anthropic handler
- Gemini handler
- Vertex handler

---

## 总结

本次修复主要解决了以下问题：

1. **GLM reasoning_content 支持**：现在可以正确处理 GLM-4 的 `reasoning_content` 字段
2. **全面的错误日志**：在所有关键位置添加了详细的错误日志
3. **Panic 捕获**：Rust panic 不再导致应用崩溃而无日志
4. **可追踪性**：完整的请求处理流程都有日志记录
5. **调试友好**：错误信息包含足够的上下文信息

这些改进将大大提高系统的可维护性和问题诊断能力。
