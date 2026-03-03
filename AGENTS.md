# AGENTS.md — ferroclaw 协作指南

> 所有参与开发的 AI agent 的入口文档。先读这里，再去 `TODO.md` 认领任务。

---

## 项目身份

- **名称**：ferroclaw（全小写）
- **定位**：OpenClaw 的 Rust 复刻版，本地优先，单二进制，4 核心能力 MVP
- **当前状态**：🚧 Phase 0 — 工程脚手架

---

## 你需要读的文件

```
docs/ROADMAP.md                   # 阶段总览（先读）
TODO.md                           # 当前任务 + 依赖关系（认领任务）
docs/03-architecture-and-tech.md  # Rust crate 选型
docs/01-what-is-openclaw.md       # OpenClaw 功能背景
```

OpenClaw 源码（workspace root: `/Users/wangyimin/project/yiminlab/`）：

```
external/openclaw-main/src/agents/   # LLM 运行时、tool call、bash 工具、记忆
external/openclaw-main/src/memory/   # 向量存储、嵌入、MMR
external/openclaw-main/src/gateway/  # Gateway、Session 管理
external/openclaw-main/src/channels/ # 渠道适配器
```

---

## 协作规则

- 认领任务：`[ ]` → `[~] 时间戳`；完成：`[x]`
- 完成后更新 `TODO.md`，不要让状态过期
- 实现前先查 OpenClaw 对应 TS 源码（速查表见下方）
- 不要引入 `03-architecture-and-tech.md` 未列出的重量级依赖

**代码规范**：
- crate 命名：`ferroclaw-<模块>`（全小写连字符）
- lib crate 错误：`thiserror`；binary/应用层：`anyhow`
- 异步：统一 `tokio`

---

## 目录结构

```
external/ferroclaw/
├── AGENTS.md
├── TODO.md
├── docs/
│   ├── ROADMAP.md
│   ├── 01-what-is-openclaw.md
│   └── 03-architecture-and-tech.md
├── cli/src/main.rs              # binary 入口
└── crates/
    ├── ferroclaw-core/          # 公共类型（P1-A）
    ├── ferroclaw-agent/         # LLM 客户端 + Agent loop（P1-B, P1-E）
    ├── ferroclaw-tools/         # Tool 框架 + bash_exec（P1-C, P1-D）
    ├── ferroclaw-session/       # SQLite 短记忆（P1-F）
    └── ferroclaw-memory/        # 向量长记忆（P1-G）
```

---

## OpenClaw 源码速查

| 实现内容 | 参考文件 |
|---------|---------|
| LLM 调用 + streaming | `src/agents/pi-embedded-runner.ts` |
| Tool call 循环 | `src/agents/pi-embedded-subscribe.handlers.tools.ts` |
| Tool 框架 | `src/agents/tool-policy.ts` |
| bash 工具 | `src/agents/bash-tools.ts`、`bash-tools.exec.ts` |
| 消息格式 | `src/agents/pi-embedded-messaging.ts` |
| Session 管理 | `src/gateway/session-utils.ts` |
| 内存管理 | `src/memory/manager.ts`、`embeddings.ts` |
| 向量检索 MMR | `src/memory/mmr.ts`、`search-manager.ts` |
| 时间衰减 | `src/memory/temporal-decay.ts` |
| 嵌入 API | `src/memory/embeddings-openai.ts`、`embeddings-ollama.ts` |
| 模型配置 | `src/agents/models-config.ts` |
