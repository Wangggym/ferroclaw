# ferroclaw

> A personal AI assistant built in Rust — inspired by OpenClaw.

ferroclaw（**ferro**=铁/Rust + **claw**）是 OpenClaw 的 Rust 复刻版，目标：单一二进制，低内存（<20MB），快速启动（<100ms）。

## MVP 能力

聊天 · 规划 · 记忆（短/长） · Linux 命令执行

## 快速开始

```bash
# 1. 初始化环境（首次，安装 rustfmt/clippy/cargo-watch，生成 .env）
make gen

# 2. 编辑 .env，填入 API Key
#    FERROCLAW_OPENAI_API_KEY=sk-...

# 3. 启动开发模式（文件变更自动重载）
make dev
```

常用命令：

| 命令 | 说明 |
|------|------|
| `make gen` | 首次环境初始化 |
| `make dev` | 开发模式（cargo watch，自动重载） |
| `make build` | debug 构建 |
| `make build-release` | 生产构建（单一二进制） |
| `make test` | 运行所有测试 |
| `make lint` | fmt + clippy（CI 模式） |
| `make fix` | cargo fmt 自动格式化 |

## 文档

- [ROADMAP](docs/ROADMAP.md) — 阶段规划总览
- [AGENTS](AGENTS.md) — AI agent 协作指南
- [架构与技术选型](docs/03-architecture-and-tech.md)
- [OpenClaw 功能梳理](docs/01-what-is-openclaw.md)

## 状态

✅ Phase 0 完成 — 进入 Phase 1 MVP
