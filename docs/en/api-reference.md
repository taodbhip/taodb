# API Reference

> **Deployment note:** Most users only need **MCP tools** — your agent discovers taoDB via `.mcp.json` and communicates over local stdio. No server to run. No token to configure.
>
> **HTTP REST API** is for non-MCP integrations and server deployments. Run `taodb serve` to enable.


This page covers the **MCP tools** — the primary interface your agent uses automatically via local stdio. For the optional HTTP server mode, see [HTTP API](http-api.md).

## MCP Tools

### taodb_stats

Check memory engine state. Call at session start.

```
taodb_stats()
```

**Parameters:** none.

**Returns:**
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

Use `container_distribution` to align with existing naming conventions. Use `recent_containers` to see what tags are active.

### taodb_recent

Return N most recent memories by insertion order. No time window expansion — purely chronological tail.

```
taodb_recent(n: integer)
```

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `n` | integer | 10 | Number of recent memories |

**Returns:** Array of `{id, time_ns, containers, text, energy}`.

### taodb_recall

Multi-dimensional spatiotemporal recall. The primary recall tool.

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

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `query` | string | (required) | Intent description for LLM understanding; also used in text-matching component of scoring |
| `containers` | string[] | [] | Spatial filter tags |
| `narrative_span_days` | integer | 3650 | Narrative time window (days) |
| `min_energy` | number | 0.0 | Energy threshold for permanent knowledge |
| `top_k` | integer | 10 | Max results |
| `dimensions` | string[] | auto | Active scoring dimensions: `天`, `地`, `道`, `人`, `物` |

**Returns:**
```json
{
  "memories": [
    {
      "id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      "text": "Fixed token rotation race condition in auth.rs",
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

The `why` and `score` fields explain why each memory was returned. Read them to understand engine decisions.

### taodb_memorize

Store a memory. Use after completing work.

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

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `text` | string | (required) | Memory content. 50-200 chars recommended. |
| `time_ns` | integer | wall clock | Narrative timestamp. Auto-derived from containers if they match time patterns (`第N回`, `sprint-N`). |
| `containers` | string[] | [] | Spatial tags |
| `energy_floor` | number | 0.0 | 0.0=normal, 0.3=important, 0.5=semi-permanent, 0.7=permanent |
| `era` | string | "" | Narrative era label |
| `relative_time` | string | "" | Human-readable time reference |
| `body_state` | JSON string | null | Body state object for sensory dimension |
| `emotional_mark` | JSON string | null | Emotion object for sensory dimension |
| `potential_field` | JSON string | null | Potential field for dimension scoring |
| `topology` | JSON string | null | Spatial topology relations |
| `senses` | string[] | [] | Sensory anchors: `["rough", "dry", "cold"]` |
| `memory_type` | string | auto | `"constraint"` or `"narrative"`. Auto-set from energy_floor. |
| `chapter_index` | string | "" | Syuzhet index: `"卷二/第152回"` |

**Returns:**
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

`warnings` and `suggestions` help you improve: missing time_ns, container normalization, suggestions for body_state on important memories.

### taodb_forget

Delete a memory.

```
taodb_forget(memory_id: string)
```

| Param | Type | Description |
|-------|------|-------------|
| `memory_id` | string | ULID of the memory to delete |

### taodb_decay

Trigger energy decay across all memories. Memories below their energy_floor are protected.

```
taodb_decay()
```

### taodb_recall_constraints

Recall constraint-layer memories only. Use at session start to load permanent rules.

```
taodb_recall_constraints(min_floor?: number, top_k?: integer)
```

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `min_floor` | number | 0.5 | Minimum energy floor |
| `top_k` | integer | 50 | Max results |

**Returns:** Array of `{id, text, energy_floor, memory_type, containers}` sorted by `energy_floor` descending.

### taodb_recall_sensory

Sensory-triggered recall. Cross-container, cross-time, cross-character.

```
taodb_recall_sensory(
  senses: string[],
  top_k?: integer,
  narrative_span_days?: integer
)
```

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `senses` | string[] | (required) | Sensory impressions: `["rough", "cold", "sharp"]` |
| `top_k` | integer | 10 | Max results |
| `narrative_span_days` | integer | 0 | Time window (0 = unlimited) |

---


---

**HTTP server mode:** See [HTTP API](http-api.md) for `taodb serve` deployment.
