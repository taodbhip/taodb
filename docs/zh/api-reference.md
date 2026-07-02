# API 参考

> **使用注意：** 大多数用户只需要 **MCP 工具**——agent 通过 `.mcp.json` 发现 taoDB，走本地 stdio 通信。不需要启动服务器。不需要配 token。
>
> **HTTP REST API** 用于非 MCP 集成和服务器部署场景。运行 `taodb serve` 启用。


本页涵盖 **MCP 工具**——agent 通过本地 stdio 自动使用的主要接口。可选 HTTP 服务器模式见 [HTTP API](http-api.md)。

## MCP 工具

### taodb_stats

检查记忆引擎状态。会话启动时调用。

```
taodb_stats()
```

**参数：** 无。

**返回：**
```json
{
  "memory_count": 224,
  "user_id": "default",
  "project_id": "default",
  "container_distribution": [
    {"name": "module:auth", "count": 12, "latest_time_ns": 1701648000000000000},
    {"name": "chapter", "count": 152, "latest_time_ns": 1782532456372428000}
  ],
  "time_range": {"min_ns": 1700000000000000000, "max_ns": 1782532456372428000},
  "energy_floor_distribution": [
    {"floor": 0.0, "count": 180},
    {"floor": 0.5, "count": 30},
    {"floor": 0.7, "count": 14}
  ],
  "recent_containers": ["module:auth", "chapter", "sprint:2025-Q1"]
}
```

用 `container_distribution` 对齐已有命名规范。用 `recent_containers` 了解活跃标签。

### taodb_recent

返回最近 N 条记忆，按插入顺序。不展开时间窗——纯时间排序尾部。

```
taodb_recent(n: integer)
```

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `n` | integer | 10 | 最近几条 |

**返回：** `{id, time_ns, containers, text, energy}` 数组。

### taodb_recall

多维时空召回。主要召回工具。

```
taodb_recall(
  query: string,
  containers?: string[],
  narrative_span_days?: integer,
  min_energy?: number,
  top_k?: integer,
  dimensions?: string[]
)
```

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `query` | string | (必填) | 意图描述，供 LLM 理解；也用于文本匹配评分 |
| `containers` | string[] | [] | 空间过滤标签 |
| `narrative_span_days` | integer | 3650 | 叙事时间窗（天） |
| `min_energy` | number | 0.0 | 能量阈值，用于永久知识 |
| `top_k` | integer | 10 | 最多返回条数 |
| `dimensions` | string[] | 自动 | 激活的评分维度：`天`、`地`、`道`、`人`、`物` |

**返回：**
```json
{
  "memories": [
    {
      "id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      "text": "修复了 auth.rs 中 token rotation 的竞态条件",
      "time_ns": 1701648000000000000,
      "era": "sprint-5",
      "containers": ["module:auth", "bugfix"],
      "energy": 0.85,
      "energy_floor": 0.3,
      "has_body": false,
      "has_emotion": false,
      "score": 3.42,
      "why": "时间距离近(分:3.0); 空间重合2个容器"
    }
  ],
  "count": 5,
  "anchor_ns": 1701648000000000000,
  "dimensions_used": ["天","地","道"],
  "recall_paths": ["anchor: 1701648000000000000", "天: time_range → 12 hits", "地: container_overlap → 8 hits"],
  "scoring_breakdown": [...]
}
```

`why` 和 `score` 字段解释每条记忆为何被返回。阅读它们以理解引擎决策。

### taodb_memorize

存储记忆。在完成工作后使用。

```
taodb_memorize(
  text: string,
  time_ns?: integer,
  containers?: string[],
  energy_floor?: number,
  era?: string,
  relative_time?: string,
  body_state?: string,
  emotional_mark?: string,
  potential_field?: string,
  topology?: string,
  senses?: string[],
  memory_type?: string,
  chapter_index?: string
)
```

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `text` | string | (必填) | 记忆内容。建议 50-200 字。 |
| `time_ns` | integer | 墙上时钟 | 叙事时间戳。容器含时间模式（`第N回`、`sprint-N`）时自动推导。 |
| `containers` | string[] | [] | 空间标签 |
| `energy_floor` | number | 0.0 | 0.0=普通, 0.3=重要, 0.5=半永久, 0.7=永久 |
| `era` | string | "" | 叙事纪元标签 |
| `relative_time` | string | "" | 人类可读时间参考 |
| `body_state` | JSON string | null | 身体状态对象，用于感官维度 |
| `emotional_mark` | JSON string | null | 情感标记对象，用于感官维度 |
| `potential_field` | JSON string | null | 势场，用于维度评分 |
| `topology` | JSON string | null | 空间拓扑关系 |
| `senses` | string[] | [] | 感官锚点：`["涩","干","凉"]` |
| `memory_type` | string | 自动 | `"constraint"` 或 `"narrative"`。由 energy_floor 自动设定。 |
| `chapter_index` | string | "" | Syuzhet 索引：`"卷二/第152回"` |

**返回：**
```json
{
  "ok": true,
  "memory_id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
  "energy_floor": 0.3,
  "time_ns": 1701648000000000000,
  "containers": ["module:auth", "bugfix"],
  "warnings": [...],
  "suggestions": [...]
}
```

`warnings` 和 `suggestions` 帮你改进：缺失 time_ns、容器规范化、重要记忆建议添加 body_state。

### taodb_forget

删除一条记忆。

```
taodb_forget(memory_id: string)
```

| 参数 | 类型 | 说明 |
|------|------|------|
| `memory_id` | string | 要删除记忆的 ULID |

### taodb_decay

触发全局能量衰减。被 energy_floor 保护的记忆不受影响。

```
taodb_decay()
```

### taodb_recall_constraints

仅召回约束层记忆。会话启动时加载永久规则。

```
taodb_recall_constraints(min_floor?: number, top_k?: integer)
```

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `min_floor` | number | 0.5 | 最低能量地板 |
| `top_k` | integer | 50 | 最多返回条数 |

**返回：** `{id, text, energy_floor, memory_type, containers}` 数组，按 `energy_floor` 降序排列。

### taodb_recall_sensory

感官触发召回。跨容器、跨时间、跨人物。

```
taodb_recall_sensory(
  senses: string[],
  top_k?: integer,
  narrative_span_days?: integer
)
```

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `senses` | string[] | (必填) | 感官印象：`["涩","凉","颤"]` |
| `top_k` | integer | 10 | 最多返回条数 |
| `narrative_span_days` | integer | 0 | 时间窗（0 = 不限） |

---


---

**HTTP 服务器模式：** 见 [HTTP API](http-api.md)。
