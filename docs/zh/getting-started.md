# 快速入门

## 前提条件

- 终端（macOS 或 Linux）
- 一个 MCP 兼容的 LLM agent：Claude Code、Cursor、Windsurf，或任何 MCP 客户端
- 不需要 Rust 工具链。不需要 API Key。不需要注册账号。


> **不需要配置任何服务器或 API。** `taodb init` 后，agent 通过 `.mcp.json` 发现 taoDB，走本地 stdio 通信——零网络、零 token。

## 安装

```bash
curl -fsSL https://raw.githubusercontent.com/taodbhip/taodb/main/install.sh | bash
```

下载预编译二进制，安装到 `/usr/local/bin/taodb`。

验证：

```bash
taodb --version
```

## 初始化项目

```bash
cd your-project
taodb init
```

这会在项目目录创建：

```
your-project/
├── .mcp.json                  # agent 自动发现 taoDB
├── .taodb/instructions.md     # agent 行为指南（可编辑，可提交 git）
└── taodb-memory/              # 记忆存储目录（已加入 .gitignore）
```

`.mcp.json` 告诉 agent"这里有个 taoDB"。`.taodb/instructions.md` 告诉 agent **怎么用**——什么时候召回，什么时候记忆。

你可以按项目需求自定义 `.taodb/instructions.md`。它被提交到 git，整个团队共享同一套记忆模式。

## 首次会话

重启你的 agent。agent 通过 `.mcp.json` 发现 taoDB，按照 instructions 流程执行：

**第一步 — agent 检查记忆状态**

```
Agent 调用: taodb_stats
结果: memory_count = 0
```

**第二步 — agent 提示导入**

```
Agent 说: "taodb 记忆为空。要导入项目内容吗？"
你说: 好
```

**第三步 — agent 读取文件，记忆关键事实**

agent 对每个重要文件调用 `taodb_memorize()`：

```json
// 架构决策（永久）
taodb_memorize({
  "text": "项目采用 Clean Architecture。Service 层不直接访问数据库。",
  "containers": ["architecture", "design-decision"],
  "energy_floor": 0.7
})

// 模块说明（半永久）
taodb_memorize({
  "text": "Auth 模块处理 JWT token 和 OAuth2 流程。位于 src/auth/。",
  "containers": ["module:auth", "tech-stack"],
  "energy_floor": 0.5
})

// 已有章回（正常衰减，带时间）
taodb_memorize({
  "text": "第1回：主角进城，注意到城门上的行会徽记。",
  "containers": ["chapter", "chapter:1", "scene:city-gate"],
  "energy_floor": 0.0,
  "time_ns": 1700000000000000000
})
```

**第四步 — 之后每次会话，上下文自动加载**

```
会话开始:
  taodb_stats     → "224 条记忆, container_distribution: module:auth(12), chapter(150)..."
  taodb_recent(1) → "最新记忆: 修复了 auth.rs 里的 token rotation 竞态条件"
  taodb_recall(within_days=5)  → 近期上下文
  taodb_recall(min_energy=0.3) → 永久知识
```

## 自定义 Agent 行为

编辑 `.taodb/instructions.md`。模板覆盖了会话启动、写前召回、写后记忆。按你的工作流调整。

网文写作示例：

```markdown
## 写前
taodb_recall(query="<本回POV人物>", within_days=5, top_k=10)
taodb_recall(query="<涉及的世界设定>", min_energy=0.3, top_k=5)

## 写后（每回落盘）
taodb_memorize(text="<关键事件摘要>",
  containers=["chapter","人物:<POV>","场景:<地点>"],
  energy_floor=0.0)
```

Vibe coding 示例：

```markdown
## 写代码前
taodb_recall(containers=["<相关模块>"], within_days=30, dimensions=["天","地"])
taodb_recall(query="<架构决策>", min_energy=0.5, dimensions=["道"])

## 写代码后（重要决策）
taodb_memorize(text="<决策：为什么这样做>",
  containers=["design-decision","<相关模块>"],
  energy_floor=0.5)
```

## 常见问题

**Agent 没使用 taoDB 工具**
- 检查 `.mcp.json` 是合法 JSON
- 检查 `taodb` 在 PATH 中（`which taodb`）
- 重启 agent

**每次会话都提示"taodb memory is empty"**
- 检查 `taodb-memory/` 目录存在且有文件
- agent 可能无权读取该目录

**macOS 提示"无法验证开发者"**
```bash
xattr -d com.apple.quarantine /usr/local/bin/taodb
```

---

**下一步：** [核心概念](concepts.md) — 理解记忆模型。
