# CRT - 全能的AI编程辅助工具

Cloud Relay Technology - 全能的AI编程辅助工具

## 功能特性

- 多 AI 模型支持（Anthropic、OpenAI、Gemini 等）
- 智能路由和负载均衡
- 实时延迟监控
- 项目管理
- 工具集成

## 技术栈

- **前端**: React + TypeScript + Vite
- **桌面框架**: Tauri 2.0
- **UI 组件**: Material-UI (MUI)
- **后端**: Rust
- **状态管理**: React Context

## 快速开始

### 安装依赖

```bash
npm install
```

### 开发模式

```bash
npm run tauri dev
```

### 构建应用

```bash
npm run tauri build
```

## 推荐的 IDE 设置

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## 许可证

Copyright © 2026 CRT Team

## API Endpoints

- `POST /v1/chat/completions`
- `POST /v1/responses`
- `GET /v1/models`
- `POST /openai/v1/chat/completions`
- `POST /openai/v1/responses`
- `POST /anthropic/v1/messages`
- `POST /gemini/v1beta/*`

## Limits Config (settings.toml)

```toml
[limits]
# requests per minute (global)
rpm = 120
# max concurrent requests (global)
max_concurrent = 8
# max concurrent requests per session (x-ccr-session-id)
max_concurrent_per_session = 2
# budgets in USD
budget_daily_usd = 5.0
budget_weekly_usd = 25.0
budget_monthly_usd = 100.0
```
