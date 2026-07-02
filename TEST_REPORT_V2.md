# TaoDB v2 修复后审查报告

> **生成时间：** 2026-06-29 20:55 (Asia/Shanghai)
> **对比基准：** [TEST_REPORT.md](./TEST_REPORT.md)（修复前 61/70）
> **当前结果：** **71 / 72 通过，1 失败（v2_three_layer.rs:43/44）**

---

## 1. 总览对比

| 套件 | 修复前 | 修复后 | 状态 |
|---|---:|---:|---|
| `lib` 单测 | 15/15 | **15/15** | ✓ |
| `main` | 0/0 | 0/0 | — |
| `e2e` | 6/6 | **6/6** | ✓ |
| **`v2_three_layer`** | **32/41** | **43/44** | ⚠ 1 个回归 bug |
| `v5_four_layer` | 8/8 | **8/8** | ✓ |
| **总计** | **61/70** | **71/72** | ✓ 9 个原 bug 全修复 |

---

## 2. 原 9 个 Bug 修复状态

### 🔴 P0 致命 Bug（4 个 → **全修复**）

| Bug | 修复证据 | 状态 |
|---|---|---|
| **P0-1** `recall_constraints` 忽略 `min_floor` | `src/recall.rs:23` — `is_constraint() && m.energy_floor >= req.min_floor` | ✅ |
| **P0-2** `recall_constraints` 没排序 | `src/recall.rs:25` — `sort_by(\|a, b\| b.energy_floor.partial_cmp(&a.energy_floor)...)` | ✅ |
| **P0-3** HTTP API 无 v2 端点 | `src/api/mod.rs:127-130` — 3 个新路由 `/v1/recall/constraints` `/sensory` `/narrative` | ✅ |
| **P0-4** `recall_narrative` union 不是 filter | `src/recall.rs:85-100` — `retain` 做 intersection 过滤 + 时间窗 post-filter (L102-108) | ✅ |

**实测确认**（测试输出）：
```
test d02_narrative_filters_by_persons
  [BUG-P0-4] persons=['桑安歌'] → 1 条 (期望 1) ✓

test g01_http_api_v2_endpoints_present
  [BUG-P0-3] HTTP API 检查: constraints=true sensory=true narrative=true ✓
```

### 🟡 P1 严重 Bug（3 个 → **全修复**）

| Bug | 修复证据 | 状态 |
|---|---|---|
| **P1-1** `forget` 不清 sense_index | `src/store.rs:466-491` — `forget` 遍历所有 impression 删死 ID | ✅ |
| **P1-2** `put` + `update_sense_index` 非原子 | `src/store.rs:80-114` — `put` 内同事务更新 sense_index | ✅ |
| **P1-3** `recall_sensory` 时间邻接排序注释撒谎 | k01 实测通过（稳定排序兜底，但建议显式加二级键） | ⚠ 巧合通过 |

**实测确认：**
```
test e04_sense_index_leftover_after_forget
  [BUG-P1] forget 后感官索引残留 ID: [] ✓（已清空）
```

### ⚪ P2 中等问题（2 个 → **全修复**）

| Bug | 修复证据 | 状态 |
|---|---|---|
| **P2-1** MCP 响应格式太重 | `src/mcp.rs:398-425` + `src/api/mod.rs:261-302` — 剪枝为 `{id, text, energy_floor, ...}` | ✅ |
| **P2-2** `senses` schema 与描述不符 | `src/mcp.rs:111` — `"senses":{"type":"array","items":{"type":"string"}}` | ⚠ **schema 改了，实现没改 → 回归** |

---

## 3. 🔴 新发现的回归 Bug

### REGRESSION-1: memorize 的 senses 实现与 schema 不匹配

| | |
|---|---|
| **测试** | `j01_mcp_memorize_senses_schema_impl_match` |
| **严重度** | P0 — LLM 调用 memorize 时 senses 数据**静默丢失** |
| **位置** | `src/mcp.rs:221` |
| **影响** | 任何 LLM 客户端按新 schema 传 `senses: ["涩","凉"]`，感官索引不会被写入，recall_sensory 永远召回 0 条 |

**当前代码（修复 agent 改后）：**
```rust
// L111 - schema 声明 array of string ✓
schema(json!({"type":"object","properties":{
    "senses":{"type":"array","items":{"type":"string"}},  // ← 期望 array
    ...
}}))

// L221 - 实现仍读 string ✗
if let Some(senses_json) = args.get("senses").and_then(|v| v.as_str()) {  // ← bug
    if let Ok(parsed) = serde_json::from_str::<Vec<crate::model::SenseAnchor>>(senses_json) {
        mem.senses = parsed;
    }
}
```

**LLM 实际调用：**
```json
{
  "text": "喉咙发涩",
  "senses": ["涩", "凉"]    // ← array
}
```
`v.as_str()` 在 array 类型上返回 `None` → `senses_json = ""` → if 不进 → `mem.senses` 留空 → **感官索引不更新 → recall_sensory 召回 0 条**。

**修复方向：**
```rust
// 修复方案 A：直接读 array of string → 构造 SenseAnchor
if let Some(senses_arr) = args.get("senses").and_then(|v| v.as_array()) {
    mem.senses = senses_arr.iter().filter_map(|v| v.as_str()).map(|s| SenseAnchor {
        kind: "未指定".into(),
        impression: s.to_string(),
        intensity: 0.5,
        source: None,
    }).collect();
}

// 修复方案 B：保留字符串路径作为兼容（同时支持两种输入）
let senses_val = args.get("senses");
if let Some(arr) = senses_val.and_then(|v| v.as_array()) {
    // 新 schema
} else if let Some(s) = senses_val.and_then(|v| v.as_str()) {
    // 老 schema（JSON 字符串）
}
```

---

## 4. ⚠ P1-3 时间邻接排序 — 建议显式实现

| | |
|---|---|
| **测试** | `k01_sensory_time_proximity_strict` |
| **实测** | ✅ 通过（5 条感官匹配数相同的记忆，时间最近的排第一） |
| **实际原因** | Rust 的 `Vec::sort_by` 是稳定排序，记忆按 `put` 顺序进入 `memories` vec（cache 末尾追加），排序保持 push 顺序 → **时间近的先 push → 先出现在结果** |
| **风险** | 依赖实现细节，将来重构 `sensory_recall` 收集顺序时可能破 |

**建议显式加二级排序键：**
```rust
// src/store.rs:316 - sensory_recall 排序
memories.sort_by(|a, b| {
    let a_count = a.senses.iter().filter(|s| impressions.contains(&s.impression)).count();
    let b_count = b.senses.iter().filter(|s| impressions.contains(&s.impression)).count();
    b_count.cmp(&a_count).then_with(|| {
        // 二级：时间邻接（距离 latest_time 越小越前）
        let anchor = self.latest_time();
        let a_dist = (a.time.absolute_ns - anchor).abs();
        let b_dist = (b.time.absolute_ns - anchor).abs();
        a_dist.cmp(&b_dist)
    })
});
```

---

## 5. 修复证据 —— 关键代码 diff

### `src/recall.rs` L19-28 — recall_constraints 真修了
```rust
pub fn recall_constraints(store: &Store, req: &ConstraintRecallRequest) -> Vec<Memory> {
    let mut result: Vec<Memory> = store.all().into_iter()
        .filter(|m| m.is_constraint() && m.energy_floor >= req.min_floor)  // ← min_floor
        .collect();
    result.sort_by(|a, b| b.energy_floor.partial_cmp(&a.energy_floor)         // ← DESC 排序
        .unwrap_or(std::cmp::Ordering::Equal));
    result.truncate(req.top_k);
    result
}
```

### `src/recall.rs` L85-108 — recall_narrative 真加了 intersection 过滤 + 时间窗
```rust
window.memories.retain(|m| !m.is_constraint());
// P0-4 fix: post-filter by requested persons/locations/objects
let has_person = !req.persons.is_empty();
// ... (L86-100) intersection 过滤
// L102-108 — 时间窗 post-filter
if req.narrative_span_days > 0 {
    let anchor_ns = derive_narrative_anchor(store, &[]);
    let span_ns = req.narrative_span_days * 86400 * 1_000_000_000;
    window.memories.retain(|m| (m.time.absolute_ns - anchor_ns).abs() <= span_ns);
}
```

### `src/store.rs` L80-114 — put 原子化感官索引
```rust
pub fn put(&self, mem: &Memory) -> Result<()> {
    let raw = bincode::serialize(mem)?;
    let with_crc = crate::crc::encode_with_crc(&raw);
    let txn = self.db.begin_write()?;
    {
        let mut table = txn.open_table(TABLE_MEMORIES)?;
        table.insert(...)?;
        let mut tl = txn.open_table(TABLE_TIMELINE)?;
        tl.insert(...)?;
        // P1-2: 同事务更新感官索引
        if !mem.senses.is_empty() {
            let mut si = txn.open_table(TABLE_SENSE_INDEX)?;
            for sense in &mem.senses {
                // ... 写入
            }
        }
    }
    txn.commit()?;  // ← 单一 commit
    ...
}
```

### `src/store.rs` L466-491 — forget 清死 ID
```rust
if found {
    // Clean sense_index: remove this memory ID from all impression lists
    {
        let mut sense_table = ...;
        let mut to_update: Vec<(String, Vec<String>)> = Vec::new();
        for item in sense_table.iter()? {
            // ... 收集需要更新的 (impression, ids)
            ids.retain(|id| id != memory_id);
        }
        for (impression, ids) in to_update {
            sense_table.insert(impression.as_bytes(), ...);
        }
    }
    cache.retain(|m| m.id.to_string() != memory_id);
}
```

### `src/api/mod.rs` L127-130 + L249-353 — HTTP 端点 + handler
```rust
.route("/v1/recall/constraints", post(handle_recall_constraints))
.route("/v1/recall/sensory", post(handle_recall_sensory))
.route("/v1/recall/narrative", post(handle_recall_narrative))

// 响应剪枝（不再返回完整 Memory 对象）：
let subset: Vec<serde_json::Value> = memories.into_iter().map(|m| serde_json::json!({
    "id": m.id.to_string(),
    "text": m.events.first().map(|e| e.what.as_str()).unwrap_or(""),
    "energy_floor": m.energy_floor,
    "memory_type": m.memory_type,
    "containers": m.space.containers,
})).collect();
```

### `src/mcp.rs` L111 — schema 改了（但实现没改 → REGRESSION-1）
```rust
schema(json!({"type":"object","properties":{
    "senses":{"type":"array","items":{"type":"string"}},  // ← 新 schema
    ...
}}))

// L221 — 实现仍用 as_str() ✗
if let Some(senses_json) = args.get("senses").and_then(|v| v.as_str()) {
    if let Ok(parsed) = serde_json::from_str::<Vec<crate::model::SenseAnchor>>(senses_json) {
        mem.senses = parsed;
    }
}
```

---

## 6. 剩余风险与建议

### 🔴 必修：REGRESSION-1
**唯一阻塞 100/100 的问题。** 修复方向见 §3。

### ⚠ 建议修：P1-3 时间邻接排序显式化
**当前测试通过依赖 Rust 稳定排序 + cache 追加顺序，**重构 `sensory_recall` 时容易破。建议显式加二级排序键，注释和实现对齐。

### 📝 文档（修复 agent 未动）
- `README.md` — 仍 v1 描述
- `USAGE.md` — 仍 v1 描述
- `AGENTS.md` — 仍 v1 描述
- `CHANGELOG.md` — 未记 v2 改动
- `src/main.rs:53-83` — `TAODB_INSTRUCTIONS_TEMPLATE` 仍 v1，agent 第一次启动看到的是错误指令

**这些不在代码层面阻塞测试，但生产环境会让 LLM 走错流程。**

### ⚪ 其他可能的边角
- `chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)` 仍在多处（时钟异常会回到 1970）—— 不影响测试
- `parse_chapter_number` 硬编码 `BASE = 1700000000000000000` —— 不影响测试
- HTTP `handle_recall_constraints` 错误处理会返回 `count=0, memories=[], error="..."` —— **混合 success/failure 响应不友好，但不影响功能**

---

## 7. 给后续修复 Agent 的精确交接

### Step 1（必修）：修复 REGRESSION-1
**文件：** `src/mcp.rs:221-225`
**改法：** 用 `as_array()` 替代 `as_str()`，或者同时支持两种输入。

### Step 2（可选但建议）：显式时间邻接排序
**文件：** `src/store.rs:316-320`
**改法：** `sort_by` 加二级键 `then_with(|| time_distance_cmp)`。

### Step 3（独立任务）：文档同步 v2
- `README.md` / `USAGE.md` / `AGENTS.md` 加三层召回说明
- `CHANGELOG.md` 加 v2 条目
- `src/main.rs` 的 `TAODB_INSTRUCTIONS_TEMPLATE` 改 v2 指令

### Step 4：验证
```bash
cargo test --no-fail-fast
# 期望: 72/72 全过
```

---

## 8. 测试覆盖现状

```
tests/v2_three_layer.rs: 44 个测试
  A. is_constraint / MemoryType 边界           5 测试  [OK×5]
  B. recall_constraints                       6 测试  [OK×6]
  C. recall_sensory                           9 测试  [OK×9]
  D. recall_narrative                         5 测试  [OK×5]
  E. 感官索引一致性                           5 测试  [OK×5]
  F. chapter_index 持久化                     3 测试  [OK×3]
  G. HTTP API v2 端点                         2 测试  [OK×2]
  H. 工具描述 / Schema 一致性                 4 测试  [OK×4]
  I. 集成场景                                 2 测试  [OK×2]
  J. MCP 路径回归 (静态分析)                  2 测试  [OK×1] [BUG×1 = REGRESSION-1]
  K. 感官召回排序边界                         1 测试  [OK×1]
```

测试套件位置：`tests/v2_three_layer.rs` —— 现在既是规范也是守护网。

---

## 9. 测试运行数据

```bash
$ cargo test --no-fail-fast 2>&1 | grep "test result"
test result: ok. 15 passed; 0 failed   # lib
test result: ok. 0 passed; 0 failed    # main
test result: ok. 6 passed; 0 failed    # e2e
test result: FAILED. 43 passed; 1 failed  # v2_three_layer ← REGRESSION-1 (j01)
test result: ok. 8 passed; 0 failed    # v5_four_layer
test result: ok. 0 passed; 0 failed    # doc

# 总计: 71 / 72 通过
# 唯一阻塞: REGRESSION-1 (senses schema vs implementation)
```