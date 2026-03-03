# ferroclaw TODO

> 认领任务：`[ ]` → `[~]`（加时间戳）；完成：`[~]` → `[x]`
> 路线图：`docs/ROADMAP.md` | 技术选型：`docs/03-architecture-and-tech.md`
> OpenClaw 源码速查：`AGENTS.md`

---

## Phase 0 — 脚手架 🚧

依赖关系：P0-A → P0-B → P0-C

**P0-A：Cargo workspace**（不可拆分，其他所有任务的前提）

- [ ] 创建 `Cargo.toml`（workspace，members: `["cli"]`）
- [ ] 创建 `cli/Cargo.toml`（依赖：`clap`、`tokio`、`tracing`、`tracing-subscriber`、`anyhow`）
- [ ] `cli/src/main.rs`：`clap` 解析 `--version`/`--help`，初始化 `tracing-subscriber`
- [ ] 验证：`cargo build` 通过

**P0-B：开发配套**（P0-A 完成后可并行开始）

- [ ] `.gitignore`（`target/`、`.env`、`*.db`）
- [ ] `.rustfmt.toml`
- [ ] `Justfile`（`just b`=build，`just t`=test，`just l`=clippy）

**P0-C：CI**（P0-A 完成后可开始，与 P0-B 并行）

- [ ] `.github/workflows/ci.yml`：`cargo clippy -- -D warnings` + `cargo test`
- [ ] 触发：push to main + PR

---

## Phase 1 — MVP ⏳

前置：Phase 0 全部完成。

---

### 依赖图

```
P1-A: ferroclaw-core（公共类型）
  ↓
P1-B: ferroclaw-agent（LLM 客户端）─────────────────────────┐
  ↓                                                          │
P1-C: ferroclaw-tools（Tool 框架）                           │
  ↓                                                          │
P1-D: bash_exec 工具（依赖 P1-C）                            │
  ↓                                                          ↓
P1-E: Agent loop / 规划（依赖 P1-B + P1-D）       P1-F: ferroclaw-session（短记忆，依赖 P1-A，可与 P1-B~D 并行）
  ↓                                                          ↓
P1-G: ferroclaw-memory（长记忆，依赖 P1-E + P1-F）
  ↓
P1-H: CLI 命令收尾（onboard / chat TUI，依赖 P1-E + P1-G）
```

**可并行组**：
- 第一组（全部串行，必须按序）：P1-A → P1-B → P1-C → P1-D → P1-E
- 第二组（与第一组并行）：P1-F 可在 P1-A 完成后立即开始
- 汇合点：P1-G 等待 P1-E 和 P1-F 都完成后开始
- 最后：P1-H 等待 P1-G 完成

---

### P1-A：ferroclaw-core（不可拆分，所有 crate 的基础）

> OpenClaw 参考：`src/agents/pi-embedded-messaging.ts`

- [ ] 创建 `crates/ferroclaw-core/`，加入 workspace members
- [ ] 定义 `Message`、`Role`、`MessageContent`、`ToolCall`、`ToolResult`
- [ ] 定义 `SessionId`（newtype），`ConversationHistory`
- [ ] 定义 `FerroError`（`thiserror`）

---

### P1-B：ferroclaw-agent — LLM 客户端（依赖 P1-A）

> OpenClaw 参考：`src/agents/pi-embedded-runner.ts`、`src/agents/models-config.ts`

可拆分为 3 个并行子任务（B1 必须先完成，B2/B3 可并行）：

**B1：LlmProvider trait + config**（不可拆分，B2/B3 的前提）
- [ ] 创建 `crates/ferroclaw-agent/`
- [ ] `LlmProvider` trait：`complete(messages, tools, opts) → impl Stream<Token>`
- [ ] config 结构：从 `~/.config/ferroclaw/config.toml` 读取（`dotenvy` + `serde`）

**B2：OpenAI 实现**（B1 完成后可与 B3 并行）
- [ ] SSE streaming（`reqwest`），tool call 格式解析

**B3：Ollama 实现**（B1 完成后可与 B2 并行）
- [ ] 与 OpenAI 格式兼容，配置 base_url

---

### P1-C：ferroclaw-tools — Tool 框架（依赖 P1-A）

> OpenClaw 参考：`src/agents/tool-policy.ts`

- [ ] 创建 `crates/ferroclaw-tools/`
- [ ] `Tool` trait：`name() / description() / input_schema() → Value / execute(input, ctx) → ToolResult`
- [ ] `ToolRegistry`：注册 + 按名查找

---

### P1-D：bash_exec 工具（依赖 P1-C）

> OpenClaw 参考：`src/agents/bash-tools.ts`、`src/agents/bash-tools.exec.ts`

- [ ] `tokio::process::Command` 执行，异步等待
- [ ] 超时控制（默认 30s，可配置）
- [ ] stdout + stderr 合并输出，超 10KB 截断
- [ ] 危险命令检测（`rm -rf /`、`sudo` 等）打印警告
- [ ] 注册到 `ToolRegistry`

---

### P1-E：Agent loop / 规划（依赖 P1-B + P1-D）

> OpenClaw 参考：`src/agents/pi-embedded-subscribe.handlers.tools.ts`

- [ ] Tool call 循环：LLM 返回 `tool_use` → 执行 → 追加结果 → 继续调用，直到纯文本回复
- [ ] 最大步数限制（默认 20 步）
- [ ] 实时打印每步：`[tool] bash_exec: ls -la`
- [ ] System prompt 中注入工具描述（JSON Schema）
- [ ] `ferroclaw agent -m "..."` 命令（单次，streaming 输出）

---

### P1-F：ferroclaw-session — 短记忆（依赖 P1-A，与 P1-B~D 并行）

> OpenClaw 参考：`src/gateway/session-utils.ts`

- [ ] 创建 `crates/ferroclaw-session/`
- [ ] SQLite schema（`sqlx` migrate）：`sessions` + `messages` 两张表
- [ ] `SessionManager`：创建 session、追加消息、加载历史（返回 `Vec<Message>`）
- [ ] `ferroclaw sessions list / clear` 命令

---

### P1-G：ferroclaw-memory — 长记忆（依赖 P1-E + P1-F）

> OpenClaw 参考：`src/memory/manager.ts`、`src/memory/embeddings.ts`、`src/memory/mmr.ts`、`src/memory/temporal-decay.ts`

可拆分（G1 必须先完成，G2/G3 可并行）：

**G1：存储层**（不可拆分）
- [ ] 创建 `crates/ferroclaw-memory/`
- [ ] `sqlite-vec` 扩展集成
- [ ] `memory_entries(id, content, embedding BLOB, created_at, accessed_at, score)` 表

**G2：嵌入客户端**（G1 完成后可与 G3 并行）
- [ ] `EmbeddingProvider` trait
- [ ] OpenAI `text-embedding-3-small` 实现
- [ ] Ollama 嵌入实现（备选）

**G3：检索逻辑**（G1 完成后可与 G2 并行）
- [ ] 余弦相似度检索，top-k 结果
- [ ] 时间衰减：`score * exp(-0.1 * days_since_access)`

**G4：集成**（G2 + G3 完成后）
- [ ] 对话结束：LLM 提取摘要 → 向量化 → 写入
- [ ] 对话开始：语义检索 top-5 → 注入 system prompt
- [ ] `ferroclaw memory list / search "..." / forget <id>` 命令

---

### P1-H：CLI 收尾（依赖 P1-G）

- [ ] `ferroclaw onboard`：引导配置 API key + 模型，写入 config.toml
- [ ] `ferroclaw chat`：`ratatui` TUI，多轮对话（集成 session + memory）
- [ ] Session 在每次对话后自动保存历史到 SQLite

---

## Phase 2~5（待 Phase 1 完成后展开）

- **Phase 2**：Gateway WS + Telegram + Discord + WebChat
- **Phase 3**：PTY + 浏览器 CDP + 文件工具
- **Phase 4**：SKILL.md + Cron
- **Phase 5**：Daemon + Docker + 托盘

---

## 决策日志

| 日期 | 决策 | 理由 |
|------|------|------|
| 2026-03-03 | 命名 ferroclaw（全小写）| Rust crate 惯例 |
| 2026-03-03 | MVP = 4 核心能力 | 去掉渠道和 WebChat，先让功能可用 |
| 2026-03-03 | SQLite（sqlx）| 零依赖部署，本地工具不需要 PG |
| 2026-03-03 | sqlite-vec 向量 | 保持单文件，不引入额外服务 |
