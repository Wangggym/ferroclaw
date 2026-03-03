# ferroclaw 开发路线图

> 读这一个文件就能理解项目全貌。详细任务在 `TODO.md`。

---

## MVP 目标（Phase 0 + 1）

ferroclaw 的第一个可用版本必须支持这 4 个能力：

| 能力 | 描述 |
|------|------|
| 聊天 | 多轮对话，streaming 输出 |
| 规划 | AI 自主多步 tool call 循环 |
| 记忆 | 短记忆（session 上下文）+ 长记忆（向量跨 session 检索）|
| 命令 | AI 调用 bash 执行 Linux 命令 |

---

## 阶段列表

| Phase | 名称 | 交付内容 | 状态 |
|-------|------|---------|------|
| **0** | 脚手架 | workspace + CI + `ferroclaw --help` | 🚧 进行中 |
| **1** | MVP | 4 核心能力（聊天 + 规划 + 记忆 + 命令）| ⏳ |
| 2 | 多渠道 | Gateway WS + Telegram + Discord | ⏳ |
| 3 | 工具扩展 | PTY + 浏览器 + 文件工具 | ⏳ |
| 4 | 技能系统 | SKILL.md + Cron + Webhook | ⏳ |
| 5 | 生态 | Daemon + Docker + 托盘 | ⏳ |

**Phase 0+1 预计**：5～7 周

---

## Phase 0 — 脚手架

完成标准：`cargo build` 通过，`ferroclaw --help` 可运行。

详细任务：`TODO.md` § Phase 0

---

## Phase 1 — MVP

完成标准：
```bash
ferroclaw agent -m "帮我分析系统资源，给出建议"
# AI 自动规划 → 调用 bash 命令 → 汇总回答
# 第二次对话能记住上次聊的内容
```

详细任务：`TODO.md` § Phase 1

> OpenClaw 参考实现：`external/openclaw-main/src/agents/`、`src/memory/`

---

## Phase 2~5（待 Phase 1 完成后展开）

- **Phase 2**：axum Gateway WS 控制平面，`teloxide`（Telegram），`serenity`（Discord），WebChat UI
- **Phase 3**：PTY（`portable-pty`），浏览器 CDP（`chromiumoxide`），文件读写，高危命令审批
- **Phase 4**：`.skills/` 目录扫描，SKILL.md system prompt 注入，`tokio-cron-scheduler`
- **Phase 5**：launchd/systemd 服务安装，Docker 镜像，系统托盘（`tauri`）
