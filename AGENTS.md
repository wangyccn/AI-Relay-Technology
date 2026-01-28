# CCR - Agent Guide

This guide provides essential information for agents working on the CCR (Cloud Control & Routing) project.

## Project Overview

CCR is a Tauri desktop application that serves as a unified control center for managing multiple AI model CLI tools. It provides a React/TypeScript frontend with a Rust backend.

**Tech Stack:**
- **Frontend:** React 19, TypeScript, Vite, Material-UI (MUI), Recharts
- **Backend:** Rust, Tauri 2, Axum (web server), SQLite
- **Build Tools:** Vite (frontend), Cargo (Rust)

## Essential Commands

### Development
```bash
# Start frontend dev server (on port 1420)
npm run dev

# Start full Tauri dev environment (frontend + backend)
npm run tauri dev

# Build for production
npm run build

# Build Tauri app
npm run tauri build

# TypeScript compilation
tsc
```

### Backend (Rust)
```bash
# Run standalone server (listens on 127.0.0.1:8787)
cd src-tauri
cargo run --bin ccr-server

# Run Tauri app
cargo run

# Build Rust
cargo build

# Run tests
cargo test

# Run specific test
cargo test health_ok

# Watch for changes during development
cargo watch -x run
```

### Production Deployment
```bash
# Build frontend first
npm run build

# Then build Tauri app
npm run tauri build
```

## Project Structure

```
CCR/
├── src/                          # React/TypeScript frontend
│   ├── api.ts                    # Centralized API client (fetch wrapper)
│   ├── types.ts                  # Shared TypeScript type definitions
│   ├── App.tsx                   # Main app with routing and layout
│   ├── main.tsx                  # React entry point
│   ├── App.css                   # Global styles (CSS custom properties)
│   ├── theme/                    # MUI theme configuration
│   │   ├── theme.ts             # Theme builder
│   │   ├── presets.ts           # Color presets
│   │   └── runtime.ts          # Theme runtime management
│   ├── pages/                    # Route components
│   │   ├── Dashboard.tsx        # Stats, charts, logs viewer
│   │   ├── Projects.tsx         # Project management
│   │   ├── Tools.tsx            # Tool installation and env detection
│   │   ├── Models.tsx           # Model configuration and routing
│   │   └── Settings.tsx         # Config editor (upstreams, proxy, etc.)
│   ├── components/              # Reusable UI components
│   │   ├── index.ts            # Central exports
│   │   ├── Button/             # Button variants
│   │   ├── Card/               # Card and StatCard
│   │   ├── Modal/              # Dialog component
│   │   ├── Table/              # Data table
│   │   ├── Input/              # Input and Select
│   │   ├── Badge/              # StatusBadge
│   │   ├── Toast/              # Toast notifications
│   │   └── Skeleton/           # Loading states
│   └── assets/                 # Static assets
├── src-tauri/                   # Rust backend
│   ├── src/
│   │   ├── lib.rs              # Tauri entry point
│   │   ├── main.rs             # Binary entry point
│   │   ├── bin/
│   │   │   └── server_main.rs  # Standalone server binary
│   │   ├── server.rs           # Axum HTTP server and routes
│   │   ├── config.rs           # Settings/Config management (with encryption)
│   │   ├── db.rs               # SQLite database operations
│   │   ├── logger.rs           # Global logging system
│   │   ├── projects.rs         # Project CRUD and opening
│   │   ├── tools.rs            # Tool detection and installation
│   │   ├── autoconfig.rs       # AI CLI auto-configuration
│   │   ├── pricing.rs          # Token pricing calculations
│   │   ├── forward/            # API forwarding layer
│   │   │   ├── mod.rs         # Public API and unified endpoints
│   │   │   ├── middleware.rs  # Request parsing and auth
│   │   │   ├── handlers/       # Provider-specific handlers
│   │   │   │   ├── openai.rs  # OpenAI-compatible
│   │   │   │   ├── anthropic.rs # Anthropic Messages API
│   │   │   │   ├── gemini.rs  # Gemini API
│   │   │   ├── client.rs      # HTTP client with retry
│   │   │   ├── context.rs     # Shared request/response types
│   │   │   ├── error.rs       # Error types
│   │   │   └── routing.rs    # Model routing with fallback
│   │   ├── routing/            # Latency testing
│   │   │   ├── mod.rs
│   │   │   └── latency.rs
│   │   └── adapters/           # Provider adapters
│   │       ├── mod.rs
│   │       ├── openai.rs
│   │       ├── anthropic.rs
│   │       └── gemini.rs
│   ├── Cargo.toml              # Rust dependencies
│   ├── tauri.conf.json         # Tauri configuration
│   └── resources/             # Tauri resources
├── package.json                # Frontend dependencies and scripts
├── vite.config.ts              # Vite configuration
├── tsconfig.json               # TypeScript configuration
└── index.html                  # HTML entry point
```

## Code Patterns & Conventions

### Frontend (TypeScript/React)

**Component Structure:**
```typescript
// Functional components with TypeScript interfaces
interface Props {
  title: string;
  onClick?: () => void;
}

export const MyComponent: React.FC<Props> = ({ title, onClick }) => {
  // Use hooks for state and side effects
  const [value, setValue] = useState('');
  useEffect(() => {
    // effect
  }, []);

  return <div>{title}</div>;
};
```

**API Usage:**
```typescript
// All API calls go through centralized api.ts
import { api } from './api';

// Pattern: api.category.action(params)
await api.projects.list();
await api.projects.create({ name: 'MyProject', path: '/path' });
await api.stats.summary('daily');
await api.tools.list();
```

**Component Organization:**
- Each page component lives in `src/pages/`
- Reusable components in `src/components/` with barrel exports (`index.ts`)
- Each component has its own folder with component file and `index.ts` for exports

**Styling:**
- Global styles use CSS custom properties in `src/App.css`
- Material-UI theme configured in `src/theme/`
- CSS variables follow `--md-sys-color-*` naming (Material Design 3)
- Use MUI's `sx` prop for inline styles, not `style` prop

**State Management:**
- Local state with `useState` hooks
- No global state library (Context API used for Toast)
- API responses managed per-component

**TypeScript:**
- Strict mode enabled
- Types defined in `src/types.ts`
- No `any` types - use proper interfaces

### Backend (Rust)

**Module Organization:**
```rust
// lib.rs is the library root with:
// - tauri::command functions (Tauri API)
// - Module declarations (mod foo;)
// - Server spawning
pub fn run() {
    crate::db::init();
    crate::logger::init();
    crate::server::spawn();
    tauri::Builder::default()
        // ...
        .run(tauri::generate_context!())
}

// server.rs defines Axum routes:
// - Health: /health
// - Stats: /api/stats/*
// - Projects: /api/projects/*
// - Tools: /api/tools/*
// - Config: /api/config
// - Unified API: /v1/chat/completions (auto-routes to provider)
// - Provider endpoints: /openai/v1/*, /anthropic/v1/*, /gemini/v1/*
```

**Error Handling:**
```rust
// Use Result<T, E> and ? operator for errors
use axum::{response::IntoResponse, Json};

async fn my_handler() -> impl IntoResponse {
    match some_operation() {
        Ok(result) => Json(result).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({"error": e}))).into_response(),
    }
}
```

**Database (SQLite):**
```rust
// Use rusqlite with prepared statements
use rusqlite::{params, Connection};

fn db_path() -> PathBuf {
    let mut p = data_dir().unwrap_or_else(|| PathBuf::from("."));
    p.push("CCR");
    std::fs::create_dir_all(&p).ok();
    p.push("ccr.db");
    p
}

fn open_conn() -> Connection {
    Connection::open(db_path()).unwrap()
}
```

**Configuration:**
- Settings stored in `~/.local/share/CCR/config.json` (Linux/Mac)
- Windows: `%APPDATA%\CCR\config.json`
- Uses serde for serialization/deserialization
- Sensitive data (API keys) encrypted with Windows DPAPI on Windows

**API Design:**
- RESTful endpoints with JSON payloads
- Query parameters for filters: `?limit=50&offset=0`
- POST for mutations, GET for queries, PUT for updates, DELETE for deletions
- CORS enabled via `tower-http`

**Async Runtime:**
- Tokio for async operations
- Tauri async runtime used for spawned tasks: `tauri::async_runtime::spawn`

## Testing

**Rust:**
```bash
# Run all tests
cargo test

# Run specific test
cargo test health_ok

# Tests are in-line in server.rs:533-547
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn health_ok() {
        // test code
    }
}
```

**Frontend:**
- No automated tests currently configured
- Manual testing via `npm run dev` or `npm run tauri dev`

## Important Gotchas

### Frontend
1. **API Base URL:** Default is `http://127.0.0.1:8787`, can be overridden with `VITE_API_BASE` env var or `window.__CCR_API_BASE__`
2. **Tauri Dev Server:** Vite runs on port 1420, configured in `vite.config.ts`
3. **Port Conflicts:** Tauri requires port 1420, backend uses 8787 - ensure these are available
4. **Component Exports:** Always use barrel exports (`index.ts`) for components
5. **MUI Theme:** Theme is built dynamically - use `resolveTheme()` and `buildMuiTheme()`

### Backend
1. **Database Location:** Different per platform - use `dirs::data_dir()` from `dirs` crate
2. **SQLite Bundled:** Uses `rusqlite` with "bundled" feature (bundled SQLite lib)
3. **Encryption:** Windows uses DPAPI for sensitive config - needs Windows-specific features
4. **Server Binary:** Standalone server at `src-tauri/src/bin/server_main.rs` for running without Tauri UI
5. **Logging:** Custom logger module - use `crate::logger::info()`, `crate::logger::error()`, etc.
6. **Model Routing:** Special reserved models for Claude Code use hardcoded IDs

### Cross-Platform
1. **Path Separators:** Use forward slashes `/` in paths for cross-platform compatibility
2. **Data Directory:** Use `dirs` crate for platform-appropriate data directory
3. **Shell Commands:** Rust's `std::process::Command` works across platforms

## API Endpoints Reference

### Unified API (Auto-routing)
- `POST /v1/chat/completions` - Routes to appropriate provider based on model
- `GET /v1/models` - List all available models

### Provider-Specific Endpoints
- `POST /openai/v1/chat/completions` - OpenAI API
- `POST /anthropic/v1/messages` - Anthropic Messages API
- `POST /gemini/v1beta/*` - Gemini API (wildcard)

### Management API
- `GET /api/stats/summary?range=daily|weekly|monthly` - Usage summary
- `GET /api/stats/series?metric=price&days=30` - Time series data
- `GET /api/stats/channels` - Channel breakdown
- `GET /api/stats/models?range=daily` - Model costs
- `GET/POST /api/projects` - Project CRUD
- `GET/POST /api/tools` - Tool management
- `GET/PUT /api/config` - Configuration
- `GET /api/environment` - Environment report

### Logs API
- `GET /api/logs?limit=50&offset=0&level=info&source=db` - Query global logs
- `DELETE /api/logs/:id` - Delete specific log entry
- `DELETE /api/logs` - Clear all logs
- `GET /api/install-logs` - Tool installation logs

## Key Files Reference

| File | Purpose |
|------|---------|
| `src/api.ts` | Centralized HTTP client, all API calls |
| `src/types.ts` | Shared TypeScript interfaces |
| `src/App.tsx` | Main app, routing, sidebar layout |
| `src-tauri/src/server.rs` | All HTTP route handlers |
| `src-tauri/src/forward/mod.rs` | API forwarding architecture |
| `src-tauri/src/config.rs` | Settings management with encryption |
| `src-tauri/src/db.rs` | SQLite database operations |
| `src-tauri/src/logger.rs` | Global logging system |
| `src-tauri/src/autoconfig.rs` | AI CLI auto-configuration |

## Development Workflow

1. **Start Backend:** `cd src-tauri && cargo run --bin ccr-server` (for API only)
2. **Start Frontend:** `npm run dev` (Vite dev server on 1420)
3. **Full Dev:** `npm run tauri dev` (both frontend and Tauri app)
4. **Type Check:** Run `tsc` to catch TypeScript errors
5. **Test Backend:** `cargo test` to run Rust tests
6. **Build Production:** `npm run build && npm run tauri build`

## Adding New Features

### Frontend Feature
1. Create page in `src/pages/`
2. Add route in `src/App.tsx` Routes section
3. Use existing API pattern from `src/api.ts`
4. Import types from `src/types.ts`
5. Use reusable components from `src/components/`
6. Style with CSS custom properties or MUI theme

### Backend Feature
1. Add handler function in `src-tauri/src/server.rs`
2. Register route in `app()` Router
3. Use existing patterns for JSON responses and error handling
4. Add types to shared config if needed
5. Update database schema in `db.rs` if needed
6. Add logging with `crate::logger`

### New API Endpoint
```rust
// 1. Define handler
async fn my_endpoint() -> Json<Value> {
    Json(json!({"status": "ok"}))
}

// 2. Register in app() function
Router::new()
    .route("/api/my-endpoint", get(my_endpoint))
```

## Configuration Files

| File | Purpose |
|------|---------|
| `package.json` | Frontend dependencies and npm scripts |
| `src-tauri/Cargo.toml` | Rust dependencies and crate config |
| `vite.config.ts` | Vite bundler configuration |
| `tsconfig.json` | TypeScript compiler options |
| `src-tauri/tauri.conf.json` | Tauri app configuration |
| `src-tauri/src/config.rs` | Runtime configuration structure |

## Theme System

The app uses Material Design 3 theming:
- **Presets:** Pre-defined color schemes in `src/theme/presets.ts`
- **Runtime:** Theme loading and switching in `src/theme/runtime.ts`
- **Builder:** Converts presets to MUI theme in `src/theme/theme.ts`
- **Colors:** CSS custom properties follow `--md-sys-color-*` naming

Theme config includes:
- Mode: `light`, `dark`, or `auto`
- Presets: Light and dark preset names
- Custom: Optional JSON theme overrides

## Proxy Configuration

Proxy settings support:
- **System proxy:** Uses OS proxy settings
- **Custom proxy:** User-specified URL, username, password
- **Bypass list:** Hosts/patterns to skip proxy

Configured in `src/pages/Settings.tsx`, stored in `ProxyConfig` struct.

## Logging

**Frontend:** Browser console logs (for dev debugging)

**Backend:** Custom logger in `src-tauri/src/logger.rs`:
- Levels: `debug`, `info`, `warn`, `error`
- Stored in SQLite database
- Queryable via `/api/logs` endpoint
- Timestamps stored as Unix timestamps

Use `crate::logger::info!(source, message)` and similar functions.

## Auto-Configuration

AI CLI tools (Claude Code, Codex, Gemini) can be auto-configured:
- Detects tool installation and config files
- Updates tool config with CCR endpoint and API key
- Creates backup of original config
- Special reserved models for Claude Code with fixed IDs

See `src-tauri/src/autoconfig.rs` for implementation.

## Tool Integration

Tools are defined in `src-tauri/src/tools.rs`:
- Detection via `which` or registry on Windows
- Installation commands for package managers
- Config file locations
- Homepage and CLI launch commands

Add new tools by extending the tools list and adding install commands.

## Model Routing

Models are configured with:
- Priority (0-100, higher = preferred)
- Provider (anthropic, openai, gemini)
- Upstream connection
- Pricing (prompt and completion per 1k tokens)

Routing supports:
- **Direct:** Use specific model
- **Auto:** Select highest priority available model
- **Priority Fallback:** Automatically try lower priority models on failure

See `src-tauri/src/forward/routing.rs` for routing logic.

## Dependencies

**Frontend:**
- React 19
- TypeScript 5.8
- Vite 7.0
- Material-UI (MUI) 7.3
- Recharts 2.12 (charts)
- React Router DOM 6.27 (routing)
- Lucide React 0.561 (icons)
- Tauri API 2

**Backend (Rust):**
- Tauri 2
- Axum 0.7 (web framework)
- tokio 1 (async runtime)
- serde/serde_json 1 (serialization)
- rusqlite 0.31 (SQLite)
- reqwest 0.12 (HTTP client)
- tower-http 0.5 (CORS)
- dirs 5 (data directories)
- chrono 0.4 (timestamps)
- regex 1 (pattern matching)

## Notes for Agents

1. **Always read files before editing** - Check exact formatting, indentation, and whitespace
2. **Use existing patterns** - Don't introduce new libraries without checking what's already used
3. **Match code style** - Follow existing TypeScript/Rust patterns in the codebase
4. **Test after changes** - Run tests for Rust, verify frontend in browser
5. **Cross-platform paths** - Use forward slashes and the `dirs` crate for data directories
6. **TypeScript strict mode** - Type safety is enforced - don't use `any`
7. **API centralization** - All frontend API calls should go through `src/api.ts`
8. **Component exports** - Use barrel exports in `index.ts` files
9. **Theme consistency** - Use CSS custom properties and MUI theme, not hardcoded colors
10. **Error handling** - Frontend shows user-friendly errors, backend returns appropriate HTTP status codes
