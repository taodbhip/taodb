# TaoDB v2 测试报告

> **生成时间：** 2026-06-27 23:50 (Asia/Shanghai)
> **测试目标：** v2 三层召回 + 感官索引 + HTTP API + 集成
> **测试套件：** `tests/v2_three_layer.rs` (新增 41 测试) + 原有 29 测试
> **总结果：** **61 / 70 通过，9 失败（全部在 v2_three_layer）**

---

## 1. 总览

| 测试套件 | 数量 | 通过 | 失败 | 覆盖范围 |
|---|---:|---:|---:|---|
| `lib` 单测 (`src/model.rs`, `src/recall.rs`, `src/store.rs`) | 15 | **15** | 0 | v1 decay/anchor/multidimensional |
| `main` (`src/main.rs`) | 0 | 0 | 0 | — |
| `e2e` (`tests/e2e.rs`) | 6 | **6** | 0 | v0.3 端到端（store/recall_window） |
| **`v2_three_layer` (新增)** | **41** | **32** | **9** | **v2 三层召回 + 感官索引 + HTTP + MCP** |
| `v5_four_layer` (`tests/v5_four_layer.rs`) | 8 | **8** | 0 | v0.5 四层能量 + boost |
| Doc tests | 0 | 0 | 0 | — |
| **总计** | **70** | **61** | **9** | — |

**关键发现：** 9 个失败测试**全部是 v2 引入的缺陷**，不是 v1/v0.5 逻辑回归。修复后即可拿到 70/70。

---

## 2. v2 测试套件分组结果

```
tests/v2_three_layer.rs 共 41 测试，按 9 组分类：

A. is_constraint / MemoryType 边界         [OK×5]   全过
B. recall_constraints（核心 bug 区）         [OK×3] [BUG-P0×2]
C. recall_sensory（感官触发层）              [OK×9]   全过
D. recall_narrative（叙事时空层）            [OK×3] [BUG-P0×2]
E. 感官索引一致性（put/forget 原子性）       [OK×4] [BUG-P1×1]
F. chapter_index 持久化                      [OK×3]   全过
G. HTTP API v2 端点（API 对称性）            [BUG-P0×2]
H. 工具描述 / Schema 一致性                  [OK×1] [BUG-P1×1] [BUG-P2×2]
I. 集成场景                                 [OK×2]   全过
```

---

## 3. 🔴 P0 致命 Bug 清单（必修）

### BUG-P0-1: `recall_constraints` 完全忽略 `min_floor` 参数

| | |
|---|---|
| **测试** | `b01_recall_constraints_respects_min_floor` |
| **严重度** | P0 — 直接违背 MCP 工具描述 "Returns flat list of constraint memories" |
| **位置** | `src/recall.rs:19-26` |
| **影响** | 用户传 `min_floor=0.7` 想筛永久记忆，返回 floor=0.5/0.6 的所有约束，半永久规则污染 LLM 上下文 |

**当前代码：**
```rust
pub fn recall_constraints(store: &Store, req: &ConstraintRecallRequest) -> Vec<Memory> {
    store.all().into_iter()
        .filter(|m| m.is_constraint())    // ← is_constraint() 写死 0.5 阈值，忽略 req.min_floor
        .take(req.top_k)
        .collect()
}
```

**实测复现：**
```
写 3 条约束 floor=0.5/0.6/0.8，传 min_floor=0.7
→ 返回 3 条（实际应为 1 条：仅 floor=0.8）
```

**修复方向：**
```rust
pub fn recall_constraints(store: &Store, req: &ConstraintRecallRequest) -> Vec<Memory> {
    let mut result: Vec<Memory> = store.all().into_iter()
        .filter(|m| m.is_constraint() && m.energy_floor >= req.min_floor)
        .collect();
    result.sort_by(|a, b| b.energy_floor.partial_cmp(&a.energy_floor).unwrap_or(Equal));
    result.truncate(req.top_k);
    result
}
```

---

### BUG-P0-2: `recall_constraints` 完全没排序（先入先出）

| | |
|---|---|
| **测试** | `b02_recall_constraints_sorted_by_floor_desc` |
| **严重度** | P0 — 工具描述明确 "sorted by energy_floor DESC"，实际是 put 顺序 |
| **位置** | `src/recall.rs:19-26`（同 BUG-P0-1） |
| **影响** | top_k=10 时可能把 floor=0.95 的永久规则全部截掉，留下 10 条 floor=0.5 的边界规则 |

**实测复现：**
```
put 顺序: low(0.5) → high(0.9) → mid(0.7)
返回顺序: low → high → mid（即 put 顺序）
期望顺序: high(0.9) → mid(0.7) → low(0.5)（DESC）
```

**修复方向：** 与 BUG-P0-1 同一处加 `sort_by`。

---

### BUG-P0-3: HTTP API 完全没暴露 v2 三层召回

| | |
|---|---|
| **测试** | `g01_http_api_v2_endpoints_present`, `g02_http_router_registered` |
| **严重度** | P0 — MCP 客户端能用 v2，HTTP 客户端只能 v1，**API 不对称** |
| **位置** | `src/api/mod.rs:115-128`（router）+ 缺 handler |
| **影响** | 任何通过 HTTP 接入的 agent 都用不上约束层 / 感官层 |

**当前 router：**
```rust
pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(handle_health))
        .route("/v1/memories", post(handle_ingest))
        .route("/v1/memories/:memory_id", delete(handle_forget))
        .route("/v1/recall", post(handle_recall))      // ← 仅 v1
        .route("/v1/recent", get(handle_recent))
        .route("/v1/decay", post(handle_decay))
        .route("/v1/stats", get(handle_stats))
        .route("/v1/projects", post(handle_create_project).get(handle_list_projects))
        .route("/v1/projects/:project_id", get(handle_get_project))
        .route("/v1/users", post(handle_create_user).get(handle_list_users))
        .with_state(state)
}
```

**修复方向：**
1. 在 `router` 加 3 个路由：
   ```rust
   .route("/v1/recall/constraints", post(handle_recall_constraints))
   .route("/v1/recall/sensory", post(handle_recall_sensory))
   .route("/v1/recall/narrative", post(handle_recall_narrative))
   ```
2. 加 3 个 handler，request/response 结构如下：
   ```rust
   #[derive(Debug, Deserialize)]
   struct ConstraintRecallHttpRequest {
       min_floor: Option<f32>,
       top_k: Option<usize>,
   }
   // handle_recall_constraints: 调 crate::recall::recall_constraints
   // handle_recall_sensory: 调 crate::recall::recall_sensory
   // handle_recall_narrative: 调 crate::recall::recall_narrative
   ```

---

### BUG-P0-4: `recall_narrative` 是 union 不是 filter（**测试时新发现**）

| | |
|---|---|
| **测试** | `d02_narrative_filters_by_persons`, `d03_narrative_filters_by_objects` |
| **严重度** | P0 — 行为完全违背用户意图 |
| **位置** | `src/recall.rs:44-84`（`recall_narrative`）+ `src/recall.rs:127-506`（`recall_multidimensional`） |
| **影响** | LLM 调用 `recall_narrative(persons=["桑安歌"])` 期望只看桑安歌的记忆，实际返回所有人 |

**实测复现：**
```
写 3 条叙事: A(人物:桑安歌) / B(人物:柏正则) / C(人物:葵儿)
recall_narrative(persons=["桑安歌"]) → 3 条（实际应为 1 条：仅 A）
```

**根因分析：**
- `recall_narrative` 把 persons/locations/objects 转成 `context_space.containers`
- 调 `recall_multidimensional(..., dimensions=[])`，空 dimensions → 激活全部维度
- `recall_multidimensional` 激活"天"维时按时间窗返回**所有**记忆（不看容器），"地"维按容器匹配返回 → **union**
- 用户期望的是 intersection

**修复方向（两条路任选）：**
- **方案 A（推荐，改 recall_multidimensional）：** 给 `recall_multidimensional` 加 `require_container_match: bool` 参数，true 时把"天"维也按容器过滤
- **方案 B（改 recall_narrative）：** 不用 `recall_multidimensional`，自己实现"按 persons/locations/objects 过滤 → 多维评分"逻辑

---

## 4. 🟡 P1 严重 Bug

### BUG-P1-1: `forget` 不清 sense_index（死 ID 残留）

| | |
|---|---|
| **测试** | `e04_sense_index_leftover_after_forget` |
| **位置** | `src/store.rs:442-457`（`forget`）+ `src/store.rs:254-275`（`update_sense_index`） |
| **影响** | 长期累积感官索引里的死 ID 会越来越多，影响召回效率 |

**实测复现：**
```
写一条凉感记忆 → update_sense_index → forget
感官索引里仍残留该 ID: ["01KW4WCBQWYG2NG0N7CGQ6SP0V"]
```

**修复方向：**
- 给 `SenseAnchor` / `Memory` 维护反向索引 `sense_id_to_memories`
- 或者在 `forget` 时遍历所有 sense_impression 表，删该 ID
- 或者 `sensory_recall` 查询后过滤 `get_by_id().is_some()`

**注意：** 当前感官召回结果**不受影响**（因为 `get_by_id` 跳过了 None），但感官索引内部脏了。

---

### BUG-P1-2: `put` + `update_sense_index` 非原子

| | |
|---|---|
| **测试** | `e05_put_then_update_sense_index_atomic`（文档性质） |
| **位置** | `src/mcp.rs:231-238` + `src/store.rs:80-100` + `src/store.rs:254-275` |
| **影响** | put 提交后到 update_sense_index 之间崩溃 → 记忆在，感官索引缺失 |

**当前 MCP 流程：**
```rust
match self.store.put(&mem) {           // ← 事务 1 提交
    Ok(()) => {
        if !mem.senses.is_empty() {
            if let Err(e) = self.store.update_sense_index(&mem) {  // ← 事务 2
                eprintln!("[mcp] sense index update warning: {}", e);
            }
        }
        ...
    }
}
```

**修复方向：**
- 方案 A：把 `update_sense_index` 的逻辑移到 `store.put` 内部，同一事务提交
- 方案 B：在 `mcp.rs` 里把 `put` + `update_sense_index` 包成一个 `store.put_with_senses(&mem)` 方法

---

### BUG-P1-3: `recall_sensory` 时间邻接排序注释撒谎

| | |
|---|---|
| **测试** | `h04_recall_sensory_time_proximity_sort`（实测竟然通过，建议保留） |
| **位置** | `src/store.rs:301-306`（注释 + sort_by） |
| **影响** | 注释承诺"按匹配感官数 DESC + 时间邻接 DESC"，代码只按感官数 |

**注意：** 实测 h04 通过是因为感官匹配数差 1 时自然有时间近的排第一。**这是个潜在 bug**，建议加 `时间邻接` 作为排序键。

---

## 5. ⚪ P2 中等问题

### BUG-P2-1: `recall_constraints` / `recall_sensory` MCP 响应格式太重

| | |
|---|---|
| **测试** | `h02_mcp_recall_constraints_response_format`, `h03_mcp_recall_sensory_response_format` |
| **位置** | `src/mcp.rs:397-419` |
| **影响** | LLM 上下文里塞满了 ulid/space/topology/potential 等无关字段，浪费 token |

**当前响应：** 直接返回完整 `Vec<Memory>`（含 ulid, time, space, events, bodies, emotion, potential, potential_energy, energy_floor, memory_type, chapter_index, senses）。

**期望响应：** 跟 `taodb_recall` 一致：
```json
{
  "count": 3,
  "memories": [
    {"id": "...", "text": "...", "energy_floor": 0.7, "memory_type": "constraint", "why": "..."}
  ]
}
```

---

### BUG-P2-2: `senses` MCP schema 与描述不符（**已通过，仅文档**）

| | |
|---|---|
| **测试** | `h01_mcp_senses_schema_vs_description` |
| **位置** | `src/mcp.rs:221-225` |
| **当前实现** | schema: `"senses":{"type":"string"}`（要 JSON 字符串 `Vec<SenseAnchor>`） |
| **描述承诺** | "input sensory impressions like `['涩','凉','颤']`"（看着像字符串数组） |

**修复方向（二选一）：**
- 改 schema 为 `"senses":{"type":"array","items":{"type":"string"}}`，LLM 传 `["涩","凉"]`，内部自动构造 `SenseAnchor { kind: "未指定", impression: x, intensity: 0.5 }`
- 或者改描述，明确写 "JSON string of `Vec<SenseAnchor>`"

**建议方案一**（更友好）。

---

## 6. 🟢 已知通过的 32 个 v2 测试

| 测试 | 描述 |
|---|---|
| `a01-a05` | `is_constraint` 边界 / 三种 Request 默认值 |
| `b03` | `recall_constraints` top_k 硬限制 |
| `b04` | `recall_constraints` 空 store |
| `b05` | `recall_constraints` 排除叙事层 |
| `b06` | 显式 `memory_type=Constraint` 即使 floor=0 也召回 |
| `c01-c09` | 感官召回 9 个测试全过（端到端、跨容器、时间窗、多感官排序、top_k、中文、空、不存在、约束记忆带感官） |
| `d01` | 叙事层排除约束层 |
| `d04-d05` | top_k 限制、空 store |
| `e01-e03, e05` | 感官索引空跳过、bincode 往返、幂等 |
| `f01-f03` | `chapter_index` 持久化、默认空、序列化往返 |
| `i01-i02` | 三层召回联动 + 500 条批量 |

---

## 7. 🛠 给修复 Agent 的交接清单

### 修复顺序建议

```
Step 1 (P0 必修):  BUG-P0-1 + BUG-P0-2 (同一函数，一起修)
Step 2 (P0 必修):  BUG-P0-4 (recall_narrative filter 行为)
Step 3 (P0 必修):  BUG-P0-3 (HTTP API 加 3 个端点)
Step 4 (P1 建议):  BUG-P1-1 (forget 清 sense_index)
Step 5 (P1 建议):  BUG-P1-2 (put + update_sense_index 原子化)
Step 6 (P2 可选):  BUG-P2-1 (MCP 响应剪枝)
Step 7 (P2 可选):  BUG-P2-2 (senses schema 友好化)
Step 8 (可选):     BUG-P1-3 (时间邻接排序)
```

### 每个 Bug 的精确修复点

| Bug | 文件 | 函数 / 行号 | 改动摘要 |
|---|---|---|---|
| P0-1 | `src/recall.rs` | `recall_constraints` (L19-26) | filter 改成 `is_constraint() && floor >= req.min_floor` + sort_by DESC + truncate |
| P0-2 | `src/recall.rs` | 同 P0-1 | 同上 |
| P0-3 | `src/api/mod.rs` | `router` (L115-128) | 加 3 个 `.route(...)` + 加 3 个 handler |
| P0-4 | `src/recall.rs` | `recall_multidimensional` 或 `recall_narrative` (L44-84) | 加 `require_container_match` 参数 OR 重写 `recall_narrative` 自己做过滤 |
| P1-1 | `src/store.rs` | `forget` (L442-457) + `update_sense_index` (L254-275) | forget 时遍历所有 sense 表删除该 ID |
| P1-2 | `src/mcp.rs` (L231-238) 或 `src/store.rs` (`put` L80-100) | 把 `update_sense_index` 移到 `put` 内部同事务 |
| P2-1 | `src/mcp.rs` (L397-419) | 改响应 JSON 形状 |
| P2-2 | `src/mcp.rs` (L221-225 + tools L111) | 改 schema 或改描述 |
| P1-3 | `src/store.rs` (`sensory_recall` L278-309) | sort_by 加二级键（时间距离） |

### 修复后验证命令

```bash
# 1. 全部 v2 测试
cargo test --test v2_three_layer

# 2. 确认 9 个失败都转通过 + 没有引入回归
cargo test --no-fail-fast 2>&1 | grep "test result"

# 3. 期望输出：
#    unittests src/lib.rs            ... ok. 15 passed
#    tests/e2e.rs                   ... ok. 6 passed
#    tests/v2_three_layer.rs        ... ok. 41 passed  ← 关键
#    tests/v5_four_layer.rs         ... ok. 8 passed
#    总计: 70/70 通过
```

---

## 8. 文档陈旧情况（建议同步更新）

不是代码 bug，但和 v2 接口不一致：

| 文件 | 状态 | 应更新 |
|---|---|---|
| `README.md` | v1，6 工具，4 层能量模型 | 改为 v2 三层召回 |
| `USAGE.md` | v1，`recall(query, within_days, min_energy)` 单接口 | 加 `recall_constraints/recall_sensory/recall_narrative` 用法 |
| `AGENTS.md` | v1，4 层能量 | 同上 |
| `CHANGELOG.md` | 1.0.0，未记 v2 | 加 `[Unreleased] v2` 条目 |
| `src/main.rs` (L53-83 `TAODB_INSTRUCTIONS_TEMPLATE`) | v1 模板 | agent 第一次启动会看到 v1 指令，**这是用户感受到的第一印象** |

---

## 9. 附录：测试运行数据

```bash
$ cargo test --no-fail-fast 2>&1 | grep "test result"
test result: ok. 15 passed; 0 failed; 0 ignored   # lib
test result: ok. 0 passed; 0 failed; 0 ignored    # main
test result: ok. 6 passed; 0 failed; 0 ignored    # e2e
test result: FAILED. 32 passed; 9 failed; ...      # v2_three_layer  ← 9 个 bug
test result: ok. 8 passed; 0 failed; 0 ignored    # v5_four_layer
test result: ok. 0 passed; 0 failed; 0 ignored    # doc

# 总计: 61 / 70 通过
```

测试套件位置：`tests/v2_three_layer.rs`（41 个测试，每个测试名前缀 `[OK]` / `[BUG-P0]` / `[BUG-P1]` / `[BUG-P2]`）。

修复 agent 可以直接 `cargo test --test v2_three_layer` 看进度。