# ferroclaw 架构设计与 Rust 技术选型

> 本文档是技术选型的权威来源。引入新依赖前先在"完整依赖清单"中登记。
> 当前阶段（Phase 0）只需关注 § 2.2 CLI 框架 和 § 2.9 日志。

---

## 一、整体架构设计

### 1.1 系统架构图

```
┌─────────────────────────────────────────────────────────────────────┐
│                         ferroclaw 系统                              │
│                                                                     │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                    Gateway（核心控制平面）                     │  │
│  │                                                               │  │
│  │  ┌────────────┐  ┌─────────────┐  ┌──────────────────────┐  │  │
│  │  │  WS Server │  │ HTTP Server │  │  Session Manager     │  │  │
│  │  │ (tokio-    │  │   (axum)    │  │  (SQLite via sqlx)   │  │  │
│  │  │ tungstenite│  │             │  │                      │  │  │
│  │  └────────────┘  └─────────────┘  └──────────────────────┘  │  │
│  │                                                               │  │
│  │  ┌────────────────────────────────────────────────────────┐  │  │
│  │  │                   Message Router                       │  │  │
│  │  │         Channel → Session → Agent 路由引擎             │  │  │
│  │  └────────────────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                              │                                      │
│           ┌──────────────────┼──────────────────┐                  │
│           │                  │                  │                  │
│  ┌────────────────┐  ┌───────────────┐  ┌───────────────────┐      │
│  │ Channel Layer  │  │  Agent Layer  │  │  Tool Engine      │      │
│  │                │  │               │  │                   │      │
│  │ - Telegram     │  │ - LLM Client  │  │ - bash_exec       │      │
│  │ - Discord      │  │ - Tool Loop   │  │ - file_ops        │      │
│  │ - Slack        │  │ - Stream Mgr  │  │ - web_fetch       │      │
│  │ - WebChat      │  │ - Compaction  │  │ - memory_search   │      │
│  │ - (extensible) │  │               │  │ - (extensible)    │      │
│  └────────────────┘  └───────────────┘  └───────────────────┘      │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                      Memory Layer                            │   │
│  │    SQLite + sqlite-vec  |  Embedding Client  |  Search       │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────┐  ┌─────────────────────────────────────────┐  │
│  │    Skills / Plugins    │  │        Config System              │  │
│  │  (SKILL.md loader)     │  │  (~/.config/ferroclaw/config.toml)│  │
│  └──────────────────┘  └─────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘

外部访问：
  CLI (ferroclaw agent/chat/gateway)
  WebChat UI  (http://localhost:18789)
  Control UI  (http://localhost:18789/control)
  Mobile Apps (WS 连接)
```

### 1.2 Crate 组织（Cargo Workspace）

```
ferroclaw/
├── Cargo.toml                   # workspace 配置
├── crates/
│   ├── ferroclaw-core/          # 核心类型、错误、配置
│   ├── ferroclaw-gateway/       # Gateway WS/HTTP 服务
│   ├── ferroclaw-session/       # Session 管理（SQLite）
│   ├── ferroclaw-agent/         # LLM 代理运行时
│   ├── ferroclaw-channels/      # Channel 适配器框架
│   │   ├── src/lib.rs           # Channel trait + 路由
│   │   └── adapters/
│   │       ├── telegram.rs
│   │       ├── discord.rs
│   │       └── webchat.rs
│   ├── ferroclaw-tools/         # 工具执行引擎
│   ├── ferroclaw-memory/        # 内存/向量存储
│   ├── ferroclaw-skills/        # 技能加载系统
│   └── ferroclaw-cli/           # CLI 入口（bin crate）
├── cli/                         # CLI 可执行文件目录
│   └── src/
│       └── main.rs
└── docs/
    ├── 01-what-is-openclaw.md
    ├── 02-development-phases.md
    └── 03-architecture-and-tech.md
```

---

## 二、各层技术选型详解

### 2.1 异步运行时与网络层

| 功能 | 选型 | 理由 |
|------|------|------|
| 异步运行时 | `tokio` (multi-thread) | Rust 生态事实标准，性能优秀 |
| WebSocket 服务 | `tokio-tungstenite` | tokio 原生集成，轻量 |
| HTTP 框架 | `axum` | tokio 生态，ergonomic API，Tower 中间件兼容 |
| HTTP 客户端 | `reqwest` | 异步，支持 streaming，TLS 开箱即用 |
| TLS | `rustls` | 纯 Rust，无 OpenSSL 依赖 |

**Gateway WebSocket 核心结构**：

```rust
// crates/ferroclaw-gateway/src/lib.rs
use axum::{Router, routing::get};
use tokio_tungstenite::WebSocketStream;

pub struct Gateway {
    config: GatewayConfig,
    session_manager: Arc<SessionManager>,
    channel_router: Arc<ChannelRouter>,
    tool_registry: Arc<ToolRegistry>,
}

impl Gateway {
    pub async fn serve(self, addr: SocketAddr) -> Result<()> {
        let app = Router::new()
            .route("/ws", get(ws_handler))
            .route("/health", get(health_handler))
            .route("/api/sessions", get(list_sessions))
            .layer(AuthLayer::new(self.config.auth_token.clone()));
        
        axum::serve(TcpListener::bind(addr).await?, app).await?;
        Ok(())
    }
}
```

---

### 2.2 CLI 框架

| 功能 | 选型 | 理由 |
|------|------|------|
| CLI 参数解析 | `clap` v4（derive 宏） | 功能完整，derive API 简洁，生成补全脚本 |
| 交互式 TUI | `ratatui` | 活跃维护，功能丰富，替代停更的 tui-rs |
| 终端着色 | `colored` 或 `owo-colors` | 轻量，跨平台 |
| 交互式 prompt | `inquire` | 选择/输入/确认 prompt，onboard 向导必需 |
| 进度条/加载 | `indicatif` | 流式输出进度显示 |
| 语法高亮 | `syntect` | 代码块高亮（可选） |

**CLI 结构示例**：

```rust
// cli/src/main.rs
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ferroclaw", version, about = "Personal AI assistant")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the Gateway server
    Gateway(GatewayArgs),
    /// Send a message to the AI agent
    Agent(AgentArgs),
    /// Interactive chat session
    Chat(ChatArgs),
    /// Run onboarding wizard
    Onboard,
    /// Diagnose configuration issues
    Doctor,
    /// Manage channels
    Channels(ChannelsArgs),
    /// Manage sessions
    Sessions(SessionsArgs),
}
```

---

### 2.3 配置系统

| 功能 | 选型 | 理由 |
|------|------|------|
| 配置格式 | TOML | 人类友好，Rust 官方 Cargo.toml 同格式 |
| 配置解析 | `config` crate + `serde` | 支持多来源合并（文件/环境变量） |
| 序列化 | `serde` + `serde_json` + `toml` | 生态标准 |
| 路径处理 | `dirs` crate | 跨平台获取 home/config 目录 |
| 热重载 | `notify` crate | 文件系统监听 |

**配置结构**：

```toml
# ~/.config/ferroclaw/config.toml
[gateway]
port = 18789
auth_token = "your-secret-token"
data_dir = "~/.local/share/ferroclaw"

[models]
default = "gpt-4o"

[[models.providers]]
name = "openai"
api_key = "sk-..."
base_url = "https://api.openai.com/v1"

[[models.providers]]
name = "ollama"
base_url = "http://localhost:11434"

[channels.telegram]
bot_token = "123456:ABC..."
dm_policy = "pairing"

[channels.discord]
token = "..."
dm_policy = "pairing"

[memory]
enabled = true
embedding_provider = "openai"
```

---

### 2.4 数据持久化

| 功能 | 选型 | 理由 |
|------|------|------|
| 关系型存储 | `sqlx` + SQLite | 异步，编译期 SQL 检查，零配置部署 |
| 向量存储 | `sqlite-vec`（SQLite 扩展）| 与主存储合一，无需额外服务 |
| 迁移管理 | `sqlx migrate` | 内置迁移支持 |
| 备选向量 | `lancedb`（嵌入式）| 如果向量性能需求增长 |

**Session 表结构**：

```sql
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    channel TEXT NOT NULL,
    peer_id TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    metadata TEXT  -- JSON
);

CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id),
    role TEXT NOT NULL,  -- 'user' | 'assistant' | 'tool'
    content TEXT NOT NULL,
    tool_calls TEXT,  -- JSON
    created_at INTEGER NOT NULL
);

CREATE TABLE memory_entries (
    id TEXT PRIMARY KEY,
    content TEXT NOT NULL,
    embedding BLOB,  -- sqlite-vec F32 vector
    created_at INTEGER NOT NULL,
    accessed_at INTEGER,
    decay_score REAL DEFAULT 1.0
);
```

---

### 2.5 LLM 客户端层

| 功能 | 选型 | 理由 |
|------|------|------|
| OpenAI 兼容 API | 手写（基于 `reqwest`）| 控制 streaming 行为，避免重量级 SDK |
| Anthropic API | 手写（基于 `reqwest`）| 同上 |
| SSE 流式解析 | `eventsource-stream` 或手写 | SSE 流式 token 接收 |
| JSON Schema | `schemars` | 工具定义 JSON Schema 生成 |
| 备选 SDK | `async-openai` | 成熟 OpenAI Rust SDK，功能完整 |

**LLM Client trait**：

```rust
// crates/ferroclaw-agent/src/llm.rs
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(
        &self,
        messages: Vec<Message>,
        tools: Vec<ToolDef>,
        options: CompletionOptions,
    ) -> Result<CompletionStream>;
    
    fn name(&self) -> &str;
    fn supports_vision(&self) -> bool { false }
    fn supports_tools(&self) -> bool { true }
}

pub struct CompletionOptions {
    pub model: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub thinking: Option<ThinkingConfig>,
}
```

---

### 2.6 Channel 适配器层

| 功能 | 选型 | 理由 |
|------|------|------|
| Telegram | `frankenstein`（API 绑定）+ 手写 polling/webhook | 轻量，功能完整 |
| Discord | `serenity` | 官方风格，成熟 |
| Slack | `slack-morphism` 或 HTTP webhook | Slack Bolt 无 Rust 实现，用 HTTP 模拟 |
| WebSocket（WebChat）| `tokio-tungstenite` | Gateway 内置 |

**Channel trait**：

```rust
// crates/ferroclaw-channels/src/lib.rs
#[async_trait]
pub trait Channel: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    
    async fn connect(&self) -> Result<()>;
    async fn disconnect(&self) -> Result<()>;
    
    /// 接收消息流
    fn incoming(&self) -> BoxStream<'_, Result<IncomingMessage>>;
    
    /// 发送消息
    async fn send(&self, msg: OutgoingMessage) -> Result<()>;
    
    /// 发送 typing indicator
    async fn send_typing(&self, peer: &PeerId) -> Result<()> { Ok(()) }
}

pub struct IncomingMessage {
    pub id: String,
    pub channel_id: String,
    pub peer: PeerId,
    pub group: Option<GroupId>,
    pub content: MessageContent,
    pub timestamp: DateTime<Utc>,
    pub attachments: Vec<Attachment>,
}
```

---

### 2.7 工具执行引擎

| 功能 | 选型 | 理由 |
|------|------|------|
| 进程执行 | `tokio::process` | 异步进程管理 |
| PTY 支持 | `portable-pty` | 跨平台 PTY，支持交互式程序 |
| 沙箱（可选）| `nsjail` / Docker exec | 隔离危险命令 |
| 文件监听 | `notify` | 工作区文件变更检测 |
| 正则/glob | `regex` + `globset` | 路径策略匹配 |

**Tool trait**：

```rust
// crates/ferroclaw-tools/src/lib.rs
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> serde_json::Value;  // JSON Schema
    
    async fn execute(
        &self,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult>;
    
    fn requires_approval(&self, input: &serde_json::Value) -> bool {
        false
    }
}

pub struct ToolContext {
    pub session_id: String,
    pub workspace_dir: PathBuf,
    pub approval_tx: Option<ApprovalSender>,
}
```

---

### 2.8 内存与嵌入系统

| 功能 | 选型 | 理由 |
|------|------|------|
| 向量存储 | `sqlite-vec`（SQLite 扩展）| 零额外服务，与 Session 同库 |
| 嵌入 API | `reqwest`（调用 OpenAI/Ollama）| 与 LLM 客户端复用 |
| MMR 算法 | 自实现（基于余弦相似度）| 简单，完全控制 |
| 时间衰减 | 自实现 | 指数衰减函数 |
| 文本分块 | `text-splitter` crate | 语义感知分块 |

---

### 2.9 日志与可观测性

| 功能 | 选型 | 理由 |
|------|------|------|
| 日志框架 | `tracing` | 结构化日志，async-aware span |
| 日志输出 | `tracing-subscriber` | 控制台/文件多输出 |
| 错误追踪 | `sentry` SDK（可选）| 生产环境错误上报 |
| 指标（可选）| `metrics` + `metrics-exporter-prometheus` | Prometheus 兼容 |

---

### 2.10 进程管理与 Daemon

| 功能 | 选型 | 理由 |
|------|------|------|
| 信号处理 | `tokio::signal` | SIGTERM/SIGINT 优雅退出 |
| PID 文件 | 自实现 | 简单文件锁 |
| systemd 集成 | `systemd` crate 或模板生成 | 生成 .service 文件 |
| launchd 集成 | plist 模板生成 | 生成 .plist 文件 |
| 自更新 | `self-replace` crate | 原地替换二进制 |

---

## 三、关键设计决策

### 3.1 单二进制发布

ferroclaw 目标是**单一可执行文件**，包含：
- Gateway 服务器
- CLI 工具
- 内嵌 WebChat 前端（静态资源打包进 binary）
- 所有 channel 适配器（feature flags 可选编译）

```toml
# Cargo.toml features
[features]
default = ["telegram", "discord", "webchat", "memory"]
telegram = ["dep:frankenstein"]
discord = ["dep:serenity"]
webchat = []
memory = ["dep:sqlite-vec"]
browser = ["dep:chromiumoxide"]
```

### 3.2 Actor 模式

Gateway 内部使用 **tokio + channel（mpsc/broadcast）** 实现 Actor 模式：
- 每个 Channel 适配器 → 独立 tokio task
- 每个 Session → 独立 tokio task（消息队列）
- 工具执行 → tokio task pool
- 消息通过 `tokio::sync::mpsc` 传递

避免引入 `actix` 等 Actor 框架，保持架构简洁。

### 3.3 WebChat 前端打包

使用 `include_dir!` 或 `rust-embed` 将前端静态资源打包进 binary：

```rust
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "frontend/dist/"]
struct WebAssets;
```

前端使用轻量框架（Preact + Vite 或 pure Web Components），通过 `bun build` 打包。

### 3.4 配置 vs OpenClaw 对比

| 特性 | OpenClaw (TS) | ferroclaw (Rust) |
|------|--------------|-----------------|
| 内存占用 | ~200MB+ (Node.js) | ~20MB（预期） |
| 启动时间 | 2~5s | <100ms |
| 单文件发布 | ❌（需 Node.js） | ✅ |
| 配置格式 | JS/YAML | TOML |
| 插件系统 | npm packages | WASM 或 native .so |
| 类型安全 | TypeScript | Rust |

---

## 四、完整依赖清单（Cargo.toml 草稿）

```toml
[workspace.dependencies]
# 异步
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = { version = "0.21", features = ["rustls-tls-native-roots"] }

# HTTP
axum = { version = "0.7", features = ["ws", "multipart"] }
reqwest = { version = "0.12", features = ["json", "stream", "rustls-tls"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "trace", "auth"] }

# 序列化
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"

# CLI
clap = { version = "4", features = ["derive", "env"] }
ratatui = "0.26"
inquire = "0.7"
indicatif = "0.17"

# 数据库
sqlx = { version = "0.7", features = ["sqlite", "runtime-tokio-rustls", "migrate"] }

# 日志
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# 错误处理
thiserror = "1"
anyhow = "1"

# 工具
async-trait = "0.1"
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
dirs = "5"
notify = "6"
regex = "1"
globset = "0.4"

# 进程
portable-pty = "0.8"

# 静态资源嵌入
rust-embed = "8"

# Channels
frankenstein = "0.30"         # Telegram（feature: telegram）
serenity = "0.12"             # Discord（feature: discord）

# 内存/向量（feature: memory）
text-splitter = "0.13"
```

---

## 五、目录结构最终预览

```
ferroclaw/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── docs/
│   ├── 01-what-is-openclaw.md
│   ├── 02-development-phases.md
│   └── 03-architecture-and-tech.md
├── cli/                          # CLI 二进制 crate
│   ├── Cargo.toml
│   └── src/
│       └── main.rs
├── crates/
│   ├── ferroclaw-core/           # 核心类型、配置
│   ├── ferroclaw-gateway/        # Gateway WS/HTTP
│   ├── ferroclaw-session/        # Session 管理
│   ├── ferroclaw-agent/          # LLM 代理层
│   ├── ferroclaw-channels/       # Channel 框架 + 适配器
│   ├── ferroclaw-tools/          # 工具执行引擎
│   ├── ferroclaw-memory/         # 向量记忆
│   └── ferroclaw-skills/         # 技能系统
├── frontend/                     # WebChat + Control UI 前端
│   ├── package.json
│   └── src/
└── scripts/
    ├── install.sh
    └── release.sh
```

---

## 六、开发工具链

| 工具 | 用途 |
|------|------|
| `rustup` | Rust 工具链管理 |
| `cargo` | 构建/测试/依赖 |
| `cargo clippy` | Lint |
| `cargo fmt` | 格式化 |
| `cargo test` | 单元 + 集成测试 |
| `cargo nextest` | 更快的测试运行器 |
| `cargo dist` | 跨平台二进制发布 |
| `bun` | 前端构建（WebChat UI） |
| `just` | 命令别名（Justfile）|
| `bacon` | 开发热重载监听 |
