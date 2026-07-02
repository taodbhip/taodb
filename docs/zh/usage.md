# 使用指南

LLM agent 使用 taoDB 的实践工作流。

## 核心循环

```
召回 → 工作 → 记忆 → (重复)
```

agent 从 taoDB 读取上下文，工作，再存入新记忆。每次会话、每个任务如此循环。

## 会话启动（每次会话）

agent 应按以下顺序启动：

```
1. taodb_stats — 检查记忆数量和容器分布
2. 如果 count = 0: 提示用户导入项目内容
   如果 count > 0: 继续
3. taodb_recent(n=1) — 找到上次写到哪了
4. taodb_recall_constraints() — 加载永久规则到上下文
5. taodb_recall(within_days=5, top_k=10) — 近期上下文
6. taodb_recall(min_energy=0.3, top_k=5) — 永久知识
```

不要跳过 `taodb_stats`。它返回 `container_distribution`——agent 据此对齐已有的命名规范，避免写出不一致的标签。

## 工作前

两次独立的召回——不要合并成一次：

```bash
# 近期上下文：这个领域最近发生了什么？
taodb_recall(containers=["module:auth"], narrative_span_days=30, dimensions=["天","地"])

# 永久知识：规则、架构、原则
taodb_recall(query="认证架构", min_energy=0.5, dimensions=["道"])
```

为什么要分两次？因为时间窗召回和能量阈值召回有不同的评分动力学。合并会稀释两个信号。

## 工作后

每次工作会话存 3-5 条记忆。不多于这个数。要精选。

该存什么：
- 状态变化 — "User 模型新增了 `last_login_ip` 字段。"
- 做出的决策 — "选了 PostgreSQL 而不是 MongoDB，因为这个查询模式的 JOIN 性能是瓶颈。"
- 修过的 bug — "修复了 token rotation 竞态：之前从过期缓存读 token，现在原子读 DB。"
- 埋下的伏笔（写作）— "提到行会徽记实际上是地图碎片。"
- 揭示的规则 — "auth 中间件在 rate limiting 之前运行。顺序对 DDoS 防护很重要。"

不该存什么：
- 琐碎细节 — "把变量名从 `x` 改成 `user_count`。"
- 重复内容 — 已经存过？别再存。
- 每步操作 — 不是日志。不是流水账。是精选的记忆。

## 容器命名规范

一致的容器标签是好召回的关键。尽早建立规范，严格执行。

**编程项目：**
```
module:auth, module:database, module:api, module:frontend
feature:oauth, feature:rate-limiting, feature:search
sprint:2025-Q1, sprint:2025-Q2
bugfix, design-decision, tech-debt
```

**网文写作：**
```
人物:桑安歌, 人物:柏正则, 人物:葵儿
场景:邯郸酒肆, 场景:骊山, 场景:老槐院
物件:鼓, 物件:凿子, 物件:剑
第1回, 第2回 ... 第152回
world_rule, character_profile, plot_point
```

**知识管理：**
```
topic:rust, topic:system-design, topic:machine-learning
source:paper, source:meeting, source:article
project:taodb, project:hermes-agent
status:draft, status:published, status:archived
```

引擎支持模糊匹配，小笔误（`module:Auth` → `module:auth`）自动修正。用 `taodb_stats` 查看已有容器名称。

## 能量地板指南

记忆重要性 → 能量地板：

| 地板值 | 使用场景 | 示例 |
|--------|---------|------|
| `0.0` | 默认。普通内容。 | "把 auth.rs 里的变量名改了一下" |
| `0.3` | 值得记住。 | "修复了 token refresh 的竞态条件" |
| `0.5` | 重要参考。 | "Auth 模块架构：JWT + OAuth2" |
| `0.7` | 永久。永远不忘。 | "绝不在环境变量里存密钥——用 vault" |

经验法则：如果你会写到项目的 README 或 ARCHITECTURE.md 里，用 `0.7`。如果某个决策会影响未来的代码，用 `0.5`。如果是下周还想得起来的事件，用 `0.3`。

## 设置叙事时间

对于有顺序的内容，`time_ns` 从容器标签自动推导。TaoDB 识别：

- `第N回` → 基于章回编号计算 time_ns
- `sprint-N` → 未来：基于 sprint 编号

只有容器不包含时间模式时才需要手动设置 `time_ns`：

```bash
taodb_memorize({
  "text": "关键事件",
  "time_ns": 1701302400000000000,
  "containers": ["chapter:5"]
})
```

绝对值不重要。只有记忆之间的相对顺序重要。

## 定期衰减

在重要里程碑后触发：

```bash
taodb_decay
```

```bash

`energy_floor ≥ current_energy` 的记忆受保护。其他记忆向地板衰减。

频率：每个 sprint 结束、每卷结束（写作）、每月一次。不是每次会话——衰减是批量操作。

## 召回维度

`dimensions` 参数调整评分权重：

| 维度 | 含义 | 权重效果 |
|------|------|---------|
| `天` | 时间距离 | 提升时间近的记忆 |
| `地` | 空间容器重合 | 提升同容器的记忆 |
| `道` | 能量/永久性 | 提升高能永久记忆 |
| `人` | 身体/情感丰富度 | 提升有 body_state 或 emotional_mark 的记忆 |
| `物` | 物件链匹配 | 提升共享物件引用的记忆 |

未指定维度时的默认值：
- 有容器: `["天","地","道"]`
- 无容器: `["天","道"]`

按召回意图定制：

```bash
# "auth 最近发生了什么？" → 时间 + 空间
taodb_recall(containers=["module:auth"], dimensions=["天","地"])

# "核心规则是什么？" → 能量 + 永久性
taodb_recall(query="架构", min_energy=0.5, dimensions=["道"])

# "哪些角色时刻有情感分量？" → 感官 + 身体
taodb_recall(containers=["人物:桑安歌"], dimensions=["人"])
```

## 感官召回

感官召回应和叙事召回分开使用。当创意共鸣比时间邻近更重要时用它：

```bash
# 正在写一个粗糙、干燥质感的场景
taodb_recall_sensory(["涩", "干"])
# 返回所有有这些感官的记忆——不论发生在哪里、什么时候
```

感官锚点是字符串：`"涩"`、`"干"`、`"凉"`、`"颤"`、`"跳"`、`"紧"`、`"滑"`、`"糙"` 等。用你领域需要的感官词汇。

---

**下一步：** [API 参考](api-reference.md) — 完整工具和接口文档。
