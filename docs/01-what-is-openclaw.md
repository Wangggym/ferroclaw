# OpenClaw 功能全景梳理

> 本文档用于分析 OpenClaw 的核心能力，为 Rust 复刻版（ferroclaw）的功能范围规划提供依据。
>
> OpenClaw 源码：`external/openclaw-main/`（workspace root: `/Users/wangyimin/project/yiminlab/`）
> 本文档是对源码的高层总结，具体实现细节直接读 TS 源码，速查表见 `AGENTS.md`。

---

## 一、OpenClaw 是什么

OpenClaw 是一个**自托管的个人 AI 助手**，设计目标是：

- 运行在用户自己的设备上（本地优先）
- 对接用户已在使用的所有消息渠道
- 遵循用户设定的规则
- 支持真实任务执行（computer-use、bash、浏览器控制等）

简单一句话概括：**跑在你设备上、接你消息、用 AI 帮你做事的自动化助手。**

---

## 二、核心架构概览

```
消息渠道（WhatsApp/Telegram/Slack/Discord/...）
               │
               ▼
┌───────────────────────────────┐
│            Gateway            │  ← 控制平面（WebSocket，默认 :18789）
│       (control plane)         │
│  会话 / 配置 / 工具 / 事件    │
└──────────────┬────────────────┘
               │
       ┌───────┴────────┐
       │                │
  Pi Agent (RPC)    CLI (openclaw …)
  AI 推理引擎       命令行接口
       │
  WebChat UI / macOS App / iOS / Android
```

### 关键子系统

| 子系统 | 描述 |
|--------|------|
| **Gateway** | WebSocket 控制平面，单进程管理所有连接、会话、工具调用 |
| **Pi Agent Runtime** | RPC 模式 AI 推理引擎，支持工具流式传输和 block 流式传输 |
| **Channel 层** | 对接 20+ 消息平台的适配器 |
| **Session 模型** | 每个渠道/群组隔离的会话管理（main/group/peer） |
| **Memory** | 可插拔的记忆存储（向量检索、嵌入） |
| **Plugin/Skills** | 可扩展的工具插件，支持 npm 分发 |
| **CLI** | 完整命令行界面，wizard/doctor/send/agent 等 |
| **Daemon** | launchd/systemd 后台服务，保持 Gateway 常驻 |

---

## 三、功能模块详解

### 3.1 Gateway（核心控制平面）

- **WebSocket 服务**：所有客户端（CLI、App、WebChat）通过 WS 连接到同一个 Gateway
- **HTTP 端点**：Control UI（管理界面）、OpenAI 兼容 HTTP 接口（`/openai/`）、Webhook 接收
- **会话管理**：创建/恢复/归档/剪枝，支持 `main`（直聊）和群组隔离模式
- **配置热重载**：运行时修改配置无需重启
- **Cron 调度**：内置定时任务，支持 wakeup 提醒
- **认证与权限**：token/password 认证，DM 配对策略，角色 allowlist
- **健康检查**：`openclaw doctor` 诊断命令

### 3.2 消息渠道（Channels）

OpenClaw 支持 **20+ 渠道适配器**：

| 渠道 | 协议/库 | 备注 |
|------|---------|------|
| WhatsApp | Baileys | 无需官方 API |
| Telegram | grammY | |
| Slack | Bolt | |
| Discord | discord.js | |
| Google Chat | Chat API | |
| Signal | signal-cli | |
| iMessage（BlueBubbles）| BlueBubbles | 推荐路径 |
| iMessage（legacy）| imsg | macOS 原生 |
| IRC | 原生协议 | |
| Microsoft Teams | Graph API | |
| Matrix | matrix-js-sdk | |
| Feishu | 飞书 API | |
| LINE | Messaging API | |
| Mattermost | API | |
| Nextcloud Talk | API | |
| Nostr | 去中心化 | |
| Synology Chat | API | |
| WebChat | 内置 | Gateway 自带 |
| 其他 | Tlon/Twitch/Zalo/Zalo Personal | |

**渠道通用能力**：
- 群组消息路由（mention 门控、reply tag）
- DM 配对策略（pairing/open）
- 分块发送（chunking）和重试策略
- typing indicator 模拟
- 媒体附件（图片/音频/视频）收发

### 3.3 AI 代理运行时（Pi Agent / Pi Embedded Runner）

- **LLM 提供商支持**：OpenAI、Anthropic、Google Gemini、GitHub Copilot、Ollama、HuggingFace、Bedrock、Cloudflare、Moonshot、BytePlus、Doubao、Qwen、Venice、Together 等
- **工具调用**：bash、browser、canvas、节点命令、session 管理等
- **多 Agent 架构**：subagent spawn/registry，支持 Agent 嵌套（带深度限制）
- **上下文窗口管理**：自动 compaction（历史压缩），token 预算守卫
- **流式传输**：block 级别流式回复
- **模型故障转移**：auth profile 轮转，cooldown/failover 策略

### 3.4 工具系统（Tools）

| 工具类别 | 具体工具 |
|----------|---------|
| **Bash 执行** | bash_exec（沙箱/宿主两种模式，PTY 支持，审批机制） |
| **浏览器控制** | Chrome/Chromium CDP 控制，截图、操作、表单填充、扩展 |
| **Canvas/A2UI** | Agent 驱动的可视化工作区，push/reset/eval/snapshot |
| **节点命令** | 摄像头快照/录制、屏幕录制、位置获取、通知推送 |
| **Session 工具** | 创建/停止/列出/归档 session，发送消息到 channel |
| **文件系统** | 读写文件，glob 模式，path policy 安全控制 |
| **内存搜索** | 向量语义搜索，时间衰减，混合检索（MMR）|
| **Webhook** | 接收外部事件触发 |
| **Gmail Pub/Sub** | 邮件触发 |

### 3.5 内存系统（Memory）

- **可插拔架构**：同时只能激活一个内存插件
- **向量嵌入**：支持 OpenAI、Gemini、Mistral、Ollama、Voyage 等嵌入模型
- **本地存储**：SQLite + sqlite-vec 向量扩展
- **批量处理**：嵌入批次上传，并发控制
- **语义检索**：MMR（最大边际相关性），query expansion，temporal decay
- **文件监听**：workspace 目录变更自动同步到内存

### 3.6 技能系统（Skills）

- **三类技能**：Bundled（内置）、Managed（通过 ClawHub 安装）、Workspace（本地 `.skills/`）
- **技能结构**：SKILL.md + 可选 hooks
- **Skills prompt 注入**：在系统 prompt 中自动合并激活的技能描述
- **安装门控**：安装确认，版本管理

### 3.7 CLI 命令行接口

| 命令 | 功能 |
|------|------|
| `openclaw onboard` | 引导向导（wizard），首次配置 Gateway、渠道、技能 |
| `openclaw gateway` | 启动/管理 Gateway |
| `openclaw agent` | 向 Agent 发送消息，获取回复 |
| `openclaw message send` | 发送消息到指定渠道联系人 |
| `openclaw pairing` | 管理 DM 配对码 |
| `openclaw doctor` | 诊断配置问题，检查 DM 策略风险 |
| `openclaw update` | 更新版本，切换 stable/beta/dev 渠道 |
| `openclaw browser` | 浏览器控制 CLI |

### 3.8 伴侣应用（Companion Apps）

- **macOS App**：菜单栏控制平面，Voice Wake/PTT，Talk Mode 覆盖层，WebChat
- **iOS Node**：Canvas、语音唤醒、Talk Mode、摄像头、屏幕录制、Bonjour 配对
- **Android Node**：连接、聊天、语音、Canvas、摄像头；Android 设备命令（通知/位置/短信/照片/联系人/日历）

### 3.9 运维与安全

- **Daemon 管理**：launchd（macOS）/ systemd（Linux）用户服务
- **Docker 支持**：Dockerfile、sandbox 容器、sandbox-browser
- **Tailscale 集成**：Serve（tailnet）/ Funnel（公网）自动配置
- **SSH 隧道**：远程访问 Gateway
- **安全默认**：DM pairing，tool policy pipeline，path policy，prompt injection 防护
- **日志系统**：结构化日志，WS 日志流

### 3.10 ACP（Agent Collaboration Protocol）

- 跨 Agent 通信协议，支持 Agent spawn 和 binding 架构
- 子 Agent 注册表，生命周期管理（announce/complete/archive）
- 深度限制防止无限嵌套

---

## 四、OpenClaw 的设计哲学

1. **本地优先**：所有数据和运行在用户自己的机器上
2. **安全默认**：强默认值，高危操作需明确 opt-in
3. **终端优先**：CLI 是第一公民，setup 流程透明
4. **可扩展**：Plugin/Skills 机制，MCP 通过 mcporter 桥接
5. **多渠道**：不绑定任何单一平台
6. **TypeScript 实现**：易于黑客/修改，生态广泛

---

## 五、核心数据流

```
[用户消息（任意渠道）]
    │
    ▼
[Channel Adapter] ─→ 规范化消息格式
    │
    ▼
[Gateway] ─→ 路由到对应 Session
    │
    ▼
[Session Manager] ─→ 加载历史上下文
    │
    ▼
[Pi Agent Runtime] ─→ 构建 system prompt + 工具列表
    │
    ▼
[LLM Provider] ─→ streaming token 回传
    │
    ▼
[Tool Executor] ─→ bash/browser/canvas/node...
    │
    ▼
[Response Chunker] ─→ 分块 + typing indicator
    │
    ▼
[Channel Adapter] ─→ 回送到原渠道
```

---

## 六、OpenClaw 能力边界总结

### 已实现（TypeScript 版）
- ✅ 20+ 消息渠道对接
- ✅ 多 LLM 提供商支持（15+ 家）
- ✅ Bash 工具执行（沙箱 + 宿主，PTY）
- ✅ 浏览器自动化（Chrome CDP）
- ✅ 向量记忆系统
- ✅ 多 Agent 编排（subagent spawn/registry）
- ✅ 技能/插件系统
- ✅ Cron 定时任务
- ✅ WebChat + Control UI
- ✅ macOS/iOS/Android 伴侣应用
- ✅ Daemon 进程管理
- ✅ 认证安全体系

### Rust 复刻重点关注
- 🎯 Gateway 核心（WebSocket 控制平面）
- 🎯 CLI 命令行工具
- 🎯 Channel 适配器框架
- 🎯 Session 管理
- 🎯 LLM 请求代理层
- 🎯 工具执行引擎（bash 优先）
- 🎯 内存/嵌入系统（SQLite + 向量）
- 🎯 Daemon/进程管理
