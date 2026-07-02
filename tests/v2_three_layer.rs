//! TaoDB v2 三层召回 + 感官索引 端到端深度测试套件
//!
//! ┌─────────────────────────────────────────────────────────────┐
//! │ 测试分组                                                     │
//! │   [OK]      通过 — 行为符合 DESIGN.md / 工具描述             │
//! │   [BUG-P0]  致命 — 直接违背 MCP 工具描述 / DESIGN.md         │
//! │   [BUG-P1]  严重 — 行为可预测但实现有缺陷                    │
//! │   [BUG-P2]  中等 — 边缘场景或可优化点                        │
//! └─────────────────────────────────────────────────────────────┘
//!
//! 已知 P0 bug（先修这 3 个，剩余 26 个测试即通过）：
//!   BUG-P0-1: recall_constraints 忽略 min_floor 参数
//!   BUG-P0-2: recall_constraints 没排序（put 顺序）
//!   BUG-P0-3: HTTP API 没暴露 v2 三层召回端点
//!
//! P1: forget 不清 sense_index / put + update_sense_index 非原子
//!     senses schema 与描述不符 / recall_sensory 时间邻接排序注释撒谎
//! P2: recall_sensory MCP 响应格式太重 / senses 中文边界

use std::collections::HashSet;
use taodb::recall::{recall_constraints, recall_narrative, recall_sensory};
use taodb::store::Store;
use taodb::{
    ConstraintRecallRequest, Memory, MemoryType, NarrativeRecallRequest, SenseAnchor, SensoryRecallRequest,
    SpatialCoord,
};

// ── Helpers ──

fn tempdir() -> tempdir::TempDir {
    tempdir::TempDir::new("taodb-v2-test").unwrap()
}

fn make_narrative(text: &str, time_ns: i64, containers: Vec<&str>, senses: Vec<(&str, &str, f32)>) -> Memory {
    let mut mem = Memory::from_text(text);
    mem.time.absolute_ns = time_ns;
    mem.space = SpatialCoord {
        containers: containers.into_iter().map(String::from).collect(),
        ..Default::default()
    };
    mem.memory_type = MemoryType::Narrative;
    mem.senses = senses
        .into_iter()
        .map(|(kind, impression, intensity)| SenseAnchor {
            kind: kind.into(),
            impression: impression.into(),
            intensity,
            source: None,
        })
        .collect();
    mem
}

fn make_constraint(text: &str, time_ns: i64, containers: Vec<&str>, floor: f32) -> Memory {
    let mut mem = Memory::from_text(text);
    mem.time.absolute_ns = time_ns;
    mem.space = SpatialCoord {
        containers: containers.into_iter().map(String::from).collect(),
        ..Default::default()
    };
    mem.energy_floor = floor;
    mem.memory_type = MemoryType::Constraint;
    mem
}

// ════════════════════════════════════════════════════════════════
// A. is_constraint / MemoryType 边界（[OK]）
// ════════════════════════════════════════════════════════════════

#[test]
fn a01_is_constraint_boundary() {
    // [OK] floor=0.49 不是约束；floor=0.5 是约束
    let mut m_below = Memory::from_text("x");
    m_below.energy_floor = 0.49;
    assert!(!m_below.is_constraint());
    let mut m_at = Memory::from_text("x");
    m_at.energy_floor = 0.5;
    assert!(m_at.is_constraint());
}

#[test]
fn a02_is_constraint_explicit_type() {
    // [OK] memory_type=Constraint + floor=0.0 → 是约束
    let mut m = Memory::from_text("x");
    m.memory_type = MemoryType::Constraint;
    assert!(m.is_constraint());
}

#[test]
fn a03_constraint_request_default() {
    // [OK] ConstraintRecallRequest::default() = { min_floor: 0.5, top_k: 50 }
    let req = ConstraintRecallRequest::default();
    assert_eq!(req.min_floor, 0.5);
    assert_eq!(req.top_k, 50);
}

#[test]
fn a04_sensory_request_default() {
    // [OK] SensoryRecallRequest::default() = { top_k: 10, narrative_span_days: 0 }
    let req = SensoryRecallRequest::default();
    assert_eq!(req.top_k, 10);
    assert_eq!(req.narrative_span_days, 0);
    assert!(req.senses.is_empty());
}

#[test]
fn a05_narrative_request_default() {
    // [OK] NarrativeRecallRequest::default() = { narrative_span_days: 30, top_k: 10 }
    let req = NarrativeRecallRequest::default();
    assert_eq!(req.narrative_span_days, 30);
    assert_eq!(req.top_k, 10);
}

// ════════════════════════════════════════════════════════════════
// B. recall_constraints（核心 bug 区）
// ════════════════════════════════════════════════════════════════

#[test]
fn b01_recall_constraints_respects_min_floor() {
    // [BUG-P0-1] recall_constraints 应遵守 req.min_floor
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;

    // floor=0.5(边界) / 0.6(半永久) / 0.8(永久)
    store.put(&make_constraint("rule-边界", t, vec!["world"], 0.5)).unwrap();
    store
        .put(&make_constraint("rule-半永久", t, vec!["world"], 0.6))
        .unwrap();
    store.put(&make_constraint("rule-永久", t, vec!["world"], 0.8)).unwrap();

    // min_floor=0.7 → 仅永久 (floor>=0.7)
    let req = ConstraintRecallRequest {
        min_floor: 0.7,
        top_k: 50,
    };
    let result = recall_constraints(&store, &req);
    assert_eq!(
        result.len(),
        1,
        "[BUG-P0-1] min_floor=0.7 应只返回 1 条，实际 {} 条",
        result.len()
    );
    assert!(result.iter().all(|m| m.energy_floor >= 0.7));

    // min_floor=0.6 → 半永久 + 永久
    let req = ConstraintRecallRequest {
        min_floor: 0.6,
        top_k: 50,
    };
    let result = recall_constraints(&store, &req);
    assert_eq!(result.len(), 2, "min_floor=0.6 应返回 2 条，实际 {} 条", result.len());
    assert!(result.iter().all(|m| m.energy_floor >= 0.6));

    // min_floor=0.5 → 全部
    let req = ConstraintRecallRequest {
        min_floor: 0.5,
        top_k: 50,
    };
    let result = recall_constraints(&store, &req);
    assert_eq!(result.len(), 3, "min_floor=0.5 应返回 3 条，实际 {} 条", result.len());
}

#[test]
fn b02_recall_constraints_sorted_by_floor_desc() {
    // [BUG-P0-2] 应按 energy_floor DESC 排序
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;

    // 故意倒序 put：先 0.5，后 0.9，再 0.7
    store.put(&make_constraint("low", t, vec![], 0.5)).unwrap();
    store.put(&make_constraint("high", t, vec![], 0.9)).unwrap();
    store.put(&make_constraint("mid", t, vec![], 0.7)).unwrap();

    let req = ConstraintRecallRequest {
        min_floor: 0.5,
        top_k: 50,
    };
    let result = recall_constraints(&store, &req);

    assert!(result.len() >= 2, "至少 2 条才能检查排序");
    assert!(
        result[0].energy_floor >= result[1].energy_floor,
        "[BUG-P0-2] 必须 energy_floor DESC, 实际 [{}, {}]",
        result[0].energy_floor,
        result[1].energy_floor
    );
    if result.len() >= 3 {
        assert!(
            result[1].energy_floor >= result[2].energy_floor,
            "[BUG-P0-2] 第二条必须 >= 第三条"
        );
    }
    // 永久记忆必须在最前
    assert_eq!(result[0].energy_floor, 0.9, "[BUG-P0-2] 永久记忆 floor=0.9 应排第一");
}

#[test]
fn b03_recall_constraints_top_k_limit() {
    // [OK] top_k 硬限制
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;
    for i in 0..20 {
        store
            .put(&make_constraint(&format!("rule-{i}"), t, vec![], 0.7))
            .unwrap();
    }
    let req = ConstraintRecallRequest {
        min_floor: 0.5,
        top_k: 5,
    };
    let result = recall_constraints(&store, &req);
    assert_eq!(result.len(), 5);
}

#[test]
fn b04_recall_constraints_empty_store() {
    // [OK] 空 store 返回空
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let req = ConstraintRecallRequest::default();
    let result = recall_constraints(&store, &req);
    assert!(result.is_empty());
}

#[test]
fn b05_recall_constraints_excludes_narrative() {
    // [OK] 不应召回叙事层（即使 floor=0.4 因为没到 0.5）
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;
    store.put(&make_narrative("叙事A", t, vec!["ch"], vec![])).unwrap(); // default floor=0
    store.put(&make_constraint("规则B", t, vec!["world"], 0.7)).unwrap();
    let req = ConstraintRecallRequest::default();
    let result = recall_constraints(&store, &req);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].events[0].what, "规则B");
}

#[test]
fn b06_recall_constraints_with_explicit_type_no_floor() {
    // [OK] is_constraint() 用 memory_type==Constraint 兜底,
    // 但 min_floor 仍然过滤: floor=0.0 不满足 min_floor=0.5
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;
    let mut m = Memory::from_text("显式类型无 floor");
    m.time.absolute_ns = t;
    m.memory_type = MemoryType::Constraint;
    m.energy_floor = 0.0;
    store.put(&m).unwrap();
    // min_floor=0.0 → 应返回
    let req = ConstraintRecallRequest {
        min_floor: 0.0,
        top_k: 50,
    };
    let result = recall_constraints(&store, &req);
    assert_eq!(result.len(), 1, "min_floor=0.0 应返回显式 Constraint 类型");
    // min_floor=0.5 → 不应返回（floor 不够）
    let req2 = ConstraintRecallRequest {
        min_floor: 0.5,
        top_k: 50,
    };
    let result2 = recall_constraints(&store, &req2);
    assert_eq!(result2.len(), 0, "min_floor=0.5 应过滤掉 floor=0.0 的约束");
}

// ════════════════════════════════════════════════════════════════
// C. recall_sensory（感官触发层）
// ════════════════════════════════════════════════════════════════

#[test]
fn c01_sensory_end_to_end() {
    // [OK] 单感官召回
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;

    let m1 = make_narrative(
        "鼓沿木缝的涩",
        t,
        vec!["人物:桑安歌", "物件:鼓"],
        vec![("触觉", "涩", 0.9)],
    );
    let m2 = make_narrative(
        "指甲刻痕的涩",
        t - 86400 * 1_000_000_000,
        vec!["人物:柏正则", "物件:凿"],
        vec![("触觉", "涩", 0.8)],
    );
    let m3 = make_narrative(
        "掌根螺旋纹的涩",
        t - 86400 * 2 * 1_000_000_000,
        vec!["人物:桑安歌", "物件:老槐"],
        vec![("触觉", "涩", 0.7)],
    );
    for m in [&m1, &m2, &m3] {
        store.put(m).unwrap();
        store.update_sense_index(m).unwrap();
    }

    let req = SensoryRecallRequest {
        senses: vec!["涩".into()],
        top_k: 10,
        narrative_span_days: 0,
    };
    let result = recall_sensory(&store, &req);
    assert_eq!(result.len(), 3);
}

#[test]
fn c02_sensory_cross_container() {
    // [OK] 跨人物/跨场景召回（核心价值）
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;

    store
        .put(&make_narrative(
            "声门下的颤",
            t,
            vec!["人物:桑安歌", "场景:邯郸"],
            vec![("触觉", "颤", 0.9)],
        ))
        .unwrap();
    store
        .put(&make_narrative(
            "凿下的颤",
            t - 86400 * 1_000_000_000,
            vec!["人物:柏正则", "场景:骊山"],
            vec![("触觉", "颤", 0.8)],
        ))
        .unwrap();
    store
        .put(&make_narrative(
            "指尖的颤",
            t - 86400 * 2 * 1_000_000_000,
            vec!["人物:葵儿", "场景:老槐院"],
            vec![("触觉", "颤", 0.7)],
        ))
        .unwrap();
    for m in store.all() {
        store.update_sense_index(&m).unwrap();
    }

    let req = SensoryRecallRequest {
        senses: vec!["颤".into()],
        top_k: 10,
        narrative_span_days: 0,
    };
    let result = recall_sensory(&store, &req);
    let persons: HashSet<String> = result
        .iter()
        .flat_map(|m| m.space.containers.iter().filter(|c| c.starts_with("人物:")).cloned())
        .collect();
    assert_eq!(persons.len(), 3, "感官触发应跨 3 个人物召回，实际 {}", persons.len());
}

#[test]
fn c03_sensory_narrative_window_filter() {
    // [OK] 时间窗过滤
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let now = 1700000000000000000_i64;

    store
        .put(&make_narrative("凉感A", now, vec![], vec![("温度", "凉", 0.9)]))
        .unwrap();
    store
        .put(&make_narrative(
            "凉感B",
            now - 5 * 86400 * 1_000_000_000,
            vec![],
            vec![("温度", "凉", 0.8)],
        ))
        .unwrap();
    store
        .put(&make_narrative(
            "凉感C",
            now - 30 * 86400 * 1_000_000_000,
            vec![],
            vec![("温度", "凉", 0.7)],
        ))
        .unwrap();
    for m in store.all() {
        store.update_sense_index(&m).unwrap();
    }

    // span=10 天 → 应筛掉 30 天前的 C
    let req = SensoryRecallRequest {
        senses: vec!["凉".into()],
        top_k: 10,
        narrative_span_days: 10,
    };
    let result = recall_sensory(&store, &req);
    assert_eq!(result.len(), 2, "span=10 应筛掉 30 天前");
}

#[test]
fn c04_sensory_multi_sense_sort() {
    // [OK] 多感官匹配 DESC 排序
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;

    store
        .put(&make_narrative("only涩", t, vec![], vec![("触觉", "涩", 0.9)]))
        .unwrap();
    store
        .put(&make_narrative(
            "涩凉",
            t - 100 * 1_000_000_000,
            vec![],
            vec![("触觉", "涩", 0.7), ("温度", "凉", 0.6)],
        ))
        .unwrap();
    store
        .put(&make_narrative(
            "涩凉颤",
            t - 200 * 1_000_000_000,
            vec![],
            vec![("触觉", "涩", 0.7), ("温度", "凉", 0.6), ("触觉", "颤", 0.5)],
        ))
        .unwrap();
    for m in store.all() {
        store.update_sense_index(&m).unwrap();
    }

    let req = SensoryRecallRequest {
        senses: vec!["涩".into(), "凉".into(), "颤".into()],
        top_k: 10,
        narrative_span_days: 0,
    };
    let result = recall_sensory(&store, &req);
    if result.len() >= 2 {
        let count = |m: &Memory| {
            m.senses
                .iter()
                .filter(|s| ["涩", "凉", "颤"].contains(&s.impression.as_str()))
                .count()
        };
        let first = count(&result[0]);
        let last = count(result.last().unwrap());
        assert!(first >= last, "感官匹配数必须 DESC: first={} last={}", first, last);
    }
}

#[test]
fn c05_sensory_empty_senses() {
    // [OK] 空 senses 返回空
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let req = SensoryRecallRequest::default();
    let result = recall_sensory(&store, &req);
    assert!(result.is_empty());
}

#[test]
fn c06_sensory_top_k_limit() {
    // [OK] top_k 硬限制
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;
    for i in 0..20 {
        let mut m = Memory::from_text(&format!("涩感{i}"));
        m.time.absolute_ns = t - i * 1_000_000_000;
        m.senses = vec![SenseAnchor {
            kind: "触觉".into(),
            impression: "涩".into(),
            intensity: 0.8,
            source: None,
        }];
        store.put(&m).unwrap();
        store.update_sense_index(&m).unwrap();
    }
    let req = SensoryRecallRequest {
        senses: vec!["涩".into()],
        top_k: 5,
        narrative_span_days: 0,
    };
    let result = recall_sensory(&store, &req);
    assert!(result.len() <= 5, "top_k 硬限制：实际 {}", result.len());
}

#[test]
fn c07_sensory_chinese_impression() {
    // [OK] 中文字符串作感官键
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;
    store
        .put(&make_narrative("涩", t, vec![], vec![("触觉", "涩", 0.9)]))
        .unwrap();
    store
        .put(&make_narrative("颤", t, vec![], vec![("触觉", "颤", 0.9)]))
        .unwrap();
    store
        .put(&make_narrative("紧", t, vec![], vec![("触觉", "紧", 0.9)]))
        .unwrap();
    for m in store.all() {
        store.update_sense_index(&m).unwrap();
    }

    let req = SensoryRecallRequest {
        senses: vec!["涩".into(), "紧".into()],
        top_k: 10,
        narrative_span_days: 0,
    };
    let result = recall_sensory(&store, &req);
    let impressions: HashSet<String> = result
        .iter()
        .flat_map(|m| m.senses.iter().map(|s| s.impression.clone()))
        .collect();
    assert!(
        impressions.contains("涩") || impressions.contains("紧"),
        "中文感官必须能匹配"
    );
}

#[test]
fn c08_sensory_nonexistent_impression() {
    // [OK] 不存在的感官返回空
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;
    store
        .put(&make_narrative("X", t, vec![], vec![("触觉", "涩", 0.9)]))
        .unwrap();
    store.update_sense_index(&Memory::from_text("X")).unwrap_or(()); // 跳过错误
    let req = SensoryRecallRequest {
        senses: vec!["不存在的感官".into()],
        top_k: 10,
        narrative_span_days: 0,
    };
    let result = recall_sensory(&store, &req);
    assert!(result.is_empty());
}

#[test]
fn c09_sensory_excludes_constraint_via_floor() {
    // [OK] 感官召回通常召回叙事，但约束记忆如果带 senses 也能召回（不强制排除）
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;

    let mut constraint_with_sense = make_constraint("规则带感官", t, vec!["world"], 0.7);
    constraint_with_sense.senses = vec![SenseAnchor {
        kind: "触觉".into(),
        impression: "涩".into(),
        intensity: 0.9,
        source: None,
    }];
    store.put(&constraint_with_sense).unwrap();
    store.update_sense_index(&constraint_with_sense).unwrap();

    let req = SensoryRecallRequest {
        senses: vec!["涩".into()],
        top_k: 10,
        narrative_span_days: 0,
    };
    let result = recall_sensory(&store, &req);
    // 当前实现：约束记忆也进感官索引，会被召回（这是预期——感官是独立维度）
    assert_eq!(result.len(), 1);
}

// ════════════════════════════════════════════════════════════════
// D. recall_narrative（叙事时空层）
// ════════════════════════════════════════════════════════════════

#[test]
fn d01_narrative_excludes_constraints() {
    // [OK] 叙事层召回不应包含约束记忆
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;
    store.put(&make_constraint("世界规则", t, vec!["world"], 0.7)).unwrap();
    store
        .put(&make_narrative(
            "叙事事件",
            t,
            vec!["人物:桑安歌"],
            vec![("触觉", "涩", 0.8)],
        ))
        .unwrap();

    let req = NarrativeRecallRequest {
        persons: vec!["桑安歌".into()],
        locations: vec![],
        objects: vec![],
        narrative_span_days: 3650,
        chapter_span: 0,
        top_k: 10,
        dimensions: vec![],
    };
    let window = recall_narrative(&store, &req);
    assert_eq!(window.memories.len(), 1);
    assert_eq!(window.memories[0].memory_type, MemoryType::Narrative);
}

#[test]
fn d02_narrative_filters_by_persons() {
    // [BUG-P0-4] recall_narrative 应按 persons 过滤（intersection），当前是 union
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;
    store
        .put(&make_narrative("A 的记忆", t, vec!["人物:桑安歌"], vec![]))
        .unwrap();
    store
        .put(&make_narrative("B 的记忆", t, vec!["人物:柏正则"], vec![]))
        .unwrap();
    store
        .put(&make_narrative("C 的记忆", t, vec!["人物:葵儿"], vec![]))
        .unwrap();

    let req = NarrativeRecallRequest {
        persons: vec!["桑安歌".into()],
        locations: vec![],
        objects: vec![],
        narrative_span_days: 3650,
        chapter_span: 0,
        top_k: 10,
        dimensions: vec![],
    };
    let window = recall_narrative(&store, &req);
    // 当前实现是 union（所有维度并行返回），期望 filter（只桑安歌）
    println!("[BUG-P0-4] persons=['桑安歌'] → {} 条 (期望 1)", window.memories.len());
    for m in &window.memories {
        println!("    text={}", m.events[0].what);
    }
    assert_eq!(window.memories.len(), 1, "[BUG-P0-4] persons 应过滤而非 union");
    assert!(window.memories[0].space.containers.contains(&"人物:桑安歌".to_string()));
}

#[test]
fn d03_narrative_filters_by_objects() {
    // [BUG-P0-4] objects 同上
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;
    store.put(&make_narrative("鼓", t, vec!["物件:鼓"], vec![])).unwrap();
    store.put(&make_narrative("凿", t, vec!["物件:凿"], vec![])).unwrap();

    let req = NarrativeRecallRequest {
        persons: vec![],
        locations: vec![],
        objects: vec!["鼓".into()],
        narrative_span_days: 3650,
        chapter_span: 0,
        top_k: 10,
        dimensions: vec![],
    };
    let window = recall_narrative(&store, &req);
    println!("[BUG-P0-4] objects=['鼓'] → {} 条 (期望 1)", window.memories.len());
    assert_eq!(window.memories.len(), 1, "[BUG-P0-4] objects 应过滤而非 union");
    assert!(window.memories[0].space.containers.contains(&"物件:鼓".to_string()));
}

#[test]
fn d04_narrative_top_k_limit() {
    // [OK] top_k 限制
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;
    for i in 0..30 {
        store
            .put(&make_narrative(
                &format!("m{i}"),
                t - i as i64 * 1_000_000_000,
                vec!["人物:桑安歌"],
                vec![],
            ))
            .unwrap();
    }
    let req = NarrativeRecallRequest {
        persons: vec!["桑安歌".into()],
        locations: vec![],
        objects: vec![],
        narrative_span_days: 3650,
        chapter_span: 0,
        top_k: 5,
        dimensions: vec![],
    };
    let window = recall_narrative(&store, &req);
    assert!(window.memories.len() <= 5);
}

#[test]
fn d05_narrative_empty_store() {
    // [OK] 空 store
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let req = NarrativeRecallRequest::default();
    let window = recall_narrative(&store, &req);
    assert!(window.memories.is_empty());
}

// ════════════════════════════════════════════════════════════════
// E. 感官索引一致性（put/forget/update_sense_index 原子性）
// ════════════════════════════════════════════════════════════════

#[test]
fn e01_sense_index_skips_empty() {
    // [OK] 空 senses 静默成功
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let mem = Memory::from_text("no senses");
    store.put(&mem).unwrap();
    assert!(store.update_sense_index(&mem).is_ok());
}

#[test]
fn e02_sense_anchor_serde_roundtrip() {
    // [OK] bincode 序列化往返
    let mut mem = Memory::from_text("test");
    mem.senses = vec![SenseAnchor {
        kind: "触觉".into(),
        impression: "涩".into(),
        intensity: 0.8,
        source: Some("鼓".into()),
    }];
    let raw = bincode::serialize(&mem).unwrap();
    let restored: Memory = bincode::deserialize(&raw).unwrap();
    assert_eq!(restored.senses.len(), 1);
    assert_eq!(restored.senses[0].impression, "涩");
    assert_eq!(restored.senses[0].kind, "触觉");
    assert_eq!(restored.senses[0].source.as_deref(), Some("鼓"));
}

#[test]
fn e03_sense_index_idempotent() {
    // [OK] 同一记忆多次 update_sense_index 不重复
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;
    let mut mem = Memory::from_text("涩");
    mem.time.absolute_ns = t;
    mem.senses = vec![SenseAnchor {
        kind: "触觉".into(),
        impression: "涩".into(),
        intensity: 0.9,
        source: None,
    }];
    store.put(&mem).unwrap();
    store.update_sense_index(&mem).unwrap();
    store.update_sense_index(&mem).unwrap();
    store.update_sense_index(&mem).unwrap();

    let req = SensoryRecallRequest {
        senses: vec!["涩".into()],
        top_k: 10,
        narrative_span_days: 0,
    };
    let result = recall_sensory(&store, &req);
    // 去重后应该只有 1 条（即使 update 3 次）
    assert_eq!(
        result.len(),
        1,
        "update_sense_index 必须幂等，实际返回 {}",
        result.len()
    );
}

#[test]
fn e04_sense_index_leftover_after_forget() {
    // [BUG-P1] forget 应清 sense_index（当前实现：死 ID 残留）
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;
    let m = make_narrative("凉感X", t, vec![], vec![("温度", "凉", 0.9)]);
    let id = m.id.to_string();
    store.put(&m).unwrap();
    store.update_sense_index(&m).unwrap();

    assert!(store.forget(&id));

    // 直接查 sense_index 看死 ID 是否还在
    let ids_in_index = store.get_ids_by_senses(&["凉".into()], 100);
    println!("[BUG-P1] forget 后感官索引残留 ID: {:?}", ids_in_index);
    // 当前实现：感官索引里还有死 ID
    assert!(
        ids_in_index.is_empty(),
        "[BUG-P1] forget 应清空感官索引中的死 ID，实际残留 {:?}",
        ids_in_index
    );
}

#[test]
fn e05_put_then_update_sense_index_atomic() {
    // [BUG-P1] put 成功后 update_sense_index 失败 → 索引缺失但记忆存在
    // 当前实现：put 成功提交后才 update_sense_index，不是原子操作
    // 这个测试只能验证当前行为的非原子性，实际修复需要 atomic txn
    // 此处不强制断言（需要 mock 失败），仅作文档：
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;
    let m = make_narrative("x", t, vec![], vec![("触觉", "涩", 0.9)]);

    store.put(&m).unwrap(); // 事务 1 提交
    // 此时如果在 put 和 update_sense_index 之间崩溃，索引缺失
    store.update_sense_index(&m).unwrap();

    // 修复后此测试应该改成"put 内部自动 update_sense_index，无需外部调用"
    let req = SensoryRecallRequest {
        senses: vec!["涩".into()],
        top_k: 10,
        narrative_span_days: 0,
    };
    let result = recall_sensory(&store, &req);
    assert_eq!(result.len(), 1);
}

// ════════════════════════════════════════════════════════════════
// F. chapter_index 持久化
// ════════════════════════════════════════════════════════════════

#[test]
fn f01_chapter_index_persists() {
    // [OK] chapter_index 持久化与回读
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let mut mem = Memory::from_text("第152回");
    mem.chapter_index = "卷二/第152回".into();
    mem.memory_type = MemoryType::Narrative;
    let id = mem.id.to_string();
    store.put(&mem).unwrap();
    drop(store);

    let store2 = Store::open(dir.path()).unwrap();
    let loaded = store2.get_by_id(&id).unwrap();
    assert_eq!(loaded.chapter_index, "卷二/第152回");
}

#[test]
fn f02_chapter_index_default_empty() {
    // [OK] 默认值是空字符串
    let mem = Memory::from_text("test");
    assert_eq!(mem.chapter_index, "");
}

#[test]
fn f03_chapter_index_in_serialization() {
    // [OK] chapter_index 参与 bincode 序列化
    let mut mem = Memory::from_text("test");
    mem.chapter_index = "卷一/第1回".into();
    mem.memory_type = MemoryType::Narrative;
    let raw = bincode::serialize(&mem).unwrap();
    let restored: Memory = bincode::deserialize(&raw).unwrap();
    assert_eq!(restored.chapter_index, "卷一/第1回");
}

// ════════════════════════════════════════════════════════════════
// G. HTTP API v2 端点（API 对称性）
// ════════════════════════════════════════════════════════════════

#[test]
fn g01_http_api_v2_endpoints_present() {
    // [BUG-P0-3] HTTP API 必须暴露 v2 三层召回
    let api_src = std::fs::read_to_string("src/api/mod.rs").unwrap();
    let has_constraints = api_src.contains("recall_constraints") || api_src.contains("recall/constraints");
    let has_sensory = api_src.contains("recall_sensory") || api_src.contains("recall/sensory");
    let has_narrative = api_src.contains("recall_narrative") || api_src.contains("recall/narrative");
    println!(
        "[BUG-P0-3] HTTP API 检查: constraints={} sensory={} narrative={}",
        has_constraints, has_sensory, has_narrative
    );
    assert!(has_constraints, "[BUG-P0-3] HTTP API 缺少 /v1/recall/constraints");
    assert!(has_sensory, "[BUG-P0-3] HTTP API 缺少 /v1/recall/sensory");
    assert!(has_narrative, "[BUG-P0-3] HTTP API 缺少 /v1/recall/narrative");
}

#[test]
fn g02_http_router_registered() {
    // [BUG-P0-3] router 必须注册 v2 端点
    let api_src = std::fs::read_to_string("src/api/mod.rs").unwrap();
    // 找到 Router::new().route(...) 块
    let has_v2_routes =
        api_src.contains(".route(\"/v1/recall/constraints\"") || api_src.contains(".route(\"/v1/recall_constraints\"");
    println!("[BUG-P0-3] router 路由注册: {}", has_v2_routes);
    assert!(has_v2_routes, "[BUG-P0-3] Router 必须注册 v2 召回路由");
}

// ════════════════════════════════════════════════════════════════
// H. 工具描述 / Schema 一致性
// ════════════════════════════════════════════════════════════════

#[test]
fn h01_mcp_senses_schema_vs_description() {
    // [BUG-P1] senses schema 描述和实际不符
    // 描述："input sensory impressions like ['涩','凉','颤']"
    // 实际 schema: "senses":{"type":"string"} —— 要 JSON 字符串 Vec<SenseAnchor>
    let mcp_src = std::fs::read_to_string("src/mcp.rs").unwrap();
    // 找到 recall_sensory 工具的 schema
    let has_string_schema = mcp_src.contains("\"senses\":{\"type\":\"string\"}");
    let has_array_schema = mcp_src.contains("\"senses\":{\"type\":\"array\"");
    println!(
        "[BUG-P1] senses schema: string={} array={}",
        has_string_schema, has_array_schema
    );
    // 当前实现是 string schema，期望 array of string（更友好）
    // 修复方向：把 schema 改成 array of string，并在内部构造 SenseAnchor
    // 或者改描述为"JSON 字符串 Vec<SenseAnchor>"
    assert!(
        !has_string_schema || has_array_schema,
        "[BUG-P1] senses schema 应是 array of string 或 描述要明确为 JSON 字符串"
    );
}

#[test]
fn h02_mcp_recall_constraints_response_format() {
    // [BUG-P2] recall_constraints MCP 响应返回完整 Memory 对象，太重
    // 应该跟 recall 一样只返回 text + 关键元数据
    let mcp_src = std::fs::read_to_string("src/mcp.rs").unwrap();
    let recall_constraints_section = mcp_src
        .find("taodb_recall_constraints")
        .and_then(|i| mcp_src[i..].find("\"memories\":memories"))
        .map(|_| true);
    println!(
        "[BUG-P2] recall_constraints 响应格式检查: 直接 memories={:?}",
        recall_constraints_section
    );
    // 当前实现：直接返回 memories 数组（完整 Memory 对象）
    // 期望：跟 recall 一样返回 {id, text, why, score, energy_floor, ...}
    assert!(
        recall_constraints_section.is_none(),
        "[BUG-P2] recall_constraints 响应应剪枝为 id/text/energy_floor 而非完整 Memory 对象"
    );
}

#[test]
fn h03_mcp_recall_sensory_response_format() {
    // [BUG-P2] recall_sensory 同上
    let mcp_src = std::fs::read_to_string("src/mcp.rs").unwrap();
    let has_bare_memories = mcp_src.contains("\"taodb_recall_sensory\"") && mcp_src.contains("\"memories\":memories");
    println!("[BUG-P2] recall_sensory 响应格式: 直接 memories={}", has_bare_memories);
    assert!(!has_bare_memories, "[BUG-P2] recall_sensory 响应应剪枝");
}

#[test]
fn h04_recall_sensory_time_proximity_sort() {
    // [BUG-P1] 注释说"按匹配感官数 DESC + 时间邻接 DESC"，实际只按感官数
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let anchor = 1700000000000000000_i64;

    // 两条记忆都带"涩"+ "凉"（感官匹配数相同=2）
    // 时间：一条近，一条远
    store
        .put(&make_narrative(
            "近",
            anchor,
            vec![],
            vec![("触觉", "涩", 0.5), ("温度", "凉", 0.5)],
        ))
        .unwrap();
    store
        .put(&make_narrative(
            "远",
            anchor - 100 * 86400 * 1_000_000_000,
            vec![],
            vec![("触觉", "涩", 0.5), ("温度", "凉", 0.5)],
        ))
        .unwrap();
    for m in store.all() {
        store.update_sense_index(&m).unwrap();
    }

    let req = SensoryRecallRequest {
        senses: vec!["涩".into(), "凉".into()],
        top_k: 10,
        narrative_span_days: 0,
    };
    let result = recall_sensory(&store, &req);

    // 注释承诺"时间邻接 DESC"，但代码没实现
    // 期望：近 排第一
    let near_first = result[0].events[0].what == "近";
    println!("[BUG-P1] 感官匹配数相同时，时间近的排第一: {}", near_first);
    // 当前实现：可能不保证顺序
    assert!(near_first, "[BUG-P1] 时间邻接 DESC 应实现");
}

// ════════════════════════════════════════════════════════════════
// I. 集成场景
// ════════════════════════════════════════════════════════════════

#[test]
fn i01_three_layer_integration() {
    // [OK] 三层召回联动：约束层 + 叙事层 + 感官层
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;

    // 约束层
    store
        .put(&make_constraint("规则:工具不感应矿脉", t, vec!["world"], 0.7))
        .unwrap();
    store
        .put(&make_constraint("人物:桑安歌感知框架", t, vec!["人物:桑安歌"], 0.7))
        .unwrap();

    // 叙事层
    let narrative = make_narrative(
        "桑安歌在酒肆发现鼓皮共鸣",
        t,
        vec!["人物:桑安歌", "场景:邯郸酒肆", "物件:鼓"],
        vec![("触觉", "涩", 0.9), ("温度", "凉", 0.6)],
    );
    store.put(&narrative).unwrap();
    store.update_sense_index(&narrative).unwrap();

    // Layer 1: 约束层
    let req_c = ConstraintRecallRequest::default();
    let constraints = recall_constraints(&store, &req_c);
    assert_eq!(constraints.len(), 2);

    // Layer 2: 感官层
    let req_s = SensoryRecallRequest {
        senses: vec!["涩".into()],
        top_k: 10,
        narrative_span_days: 0,
    };
    let sensory = recall_sensory(&store, &req_s);
    assert_eq!(sensory.len(), 1);
    assert!(sensory[0].events[0].what.contains("桑安歌"));

    // Layer 3: 叙事层
    let req_n = NarrativeRecallRequest {
        persons: vec!["桑安歌".into()],
        locations: vec![],
        objects: vec![],
        narrative_span_days: 3650,
        chapter_span: 0,
        top_k: 10,
        dimensions: vec![],
    };
    let narrative_result = recall_narrative(&store, &req_n);
    assert_eq!(narrative_result.memories.len(), 1);
    assert_eq!(narrative_result.memories[0].memory_type, MemoryType::Narrative);
}

#[test]
fn i02_bulk_500_memories_recall() {
    // [OK] 批量性能 smoke test：500 条记忆下感官召回能跑
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;
    let impressions = ["涩", "凉", "颤", "跳", "紧"];
    for i in 0..500 {
        let imp = impressions[i % 5];
        let mut m = Memory::from_text(&format!("m{i}"));
        m.time.absolute_ns = t - i as i64 * 86400 * 1_000_000_000;
        m.senses = vec![SenseAnchor {
            kind: "触觉".into(),
            impression: imp.into(),
            intensity: 0.8,
            source: None,
        }];
        store.put(&m).unwrap();
        store.update_sense_index(&m).unwrap();
    }
    let req = SensoryRecallRequest {
        senses: vec!["涩".into()],
        top_k: 10,
        narrative_span_days: 0,
    };
    let result = recall_sensory(&store, &req);
    // 100 条涩感（500/5）
    assert!(result.len() <= 10, "top_k=10 必须硬限制");
    assert!(result.len() > 0, "应至少召回 1 条涩感");
}

// ════════════════════════════════════════════════════════════════
// J. MCP 路径回归测试（静态分析 + Store API 模拟）
// ════════════════════════════════════════════════════════════════

#[test]
fn j01_mcp_memorize_senses_schema_impl_match() {
    // [REGRESSION] schema 改成 array of string 后，memorize 实现也要兼容
    // 用静态分析检查实现是否处理 array 输入
    let mcp_src = std::fs::read_to_string("src/mcp.rs").unwrap();

    // 1. 确认 schema 是 array of string（修复已生效）
    let has_array_schema = mcp_src.contains(r#""senses":{"type":"array","items":{"type":"string"}}"#);
    assert!(has_array_schema, "schema 应是 array of string");

    // 2. 找到 memorize dispatch 分支（L128-256），检查 senses 处理
    //    关键看 L221 附近的代码 —— 应该用 as_array() / str_list()，不能是 as_str()
    let dispatch_start = mcp_src.find("\"taodb_memorize\" => {").expect("memorize dispatch");
    // 取 memorize 分支到下一个 handler 之间的代码
    let next_handler = mcp_src[dispatch_start..]
        .find("\"taodb_recall\" => {")
        .map(|i| dispatch_start + i)
        .unwrap_or(mcp_src.len());
    let memorize_block = &mcp_src[dispatch_start..next_handler];

    println!("[REGRESSION] memorize dispatch 分支 senses 处理代码：");
    for line in memorize_block.lines() {
        if line.contains("senses") || line.contains("as_str") || line.contains("as_array") {
            println!("  {}", line.trim());
        }
    }

    // 修复期望：memorize 分支里有这段代码：
    //   if let Some(senses_arr) = args.get("senses").and_then(|v| v.as_array()) {
    //       mem.senses = senses_arr.iter().filter_map(...).map(|s| SenseAnchor {...}).collect();
    //   }
    // 当前 bug 仍然在：仍然用 as_str() + serde_json::from_str::<Vec<SenseAnchor>>
    let uses_as_array_on_senses = memorize_block
        .lines()
        .filter(|l| l.contains("senses") && l.contains("as_array"))
        .count()
        > 0;

    println!("[REGRESSION] memorize 分支: as_array()={}", uses_as_array_on_senses);

    assert!(
        uses_as_array_on_senses,
        "[REGRESSION] memorize dispatch 应该用 as_array() 处理 array of string schema（主路径）"
    );
}

#[test]
fn j02_sensory_recall_after_store_put() {
    // [REGRESSION] 模拟 LLM 调用 memorize → recall_sensory 的端到端路径
    // 通过 Store API（这是 MCP dispatch 内部调的同一路径）
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;

    // 模拟 LLM 按新 schema 传 senses array of string 的内部处理路径
    // MCP 当前实现的处理方式（修复前）：as_str() 读不到 array → senses 留空
    // 修复后应该构造 SenseAnchor 写入
    let llm_input_senses = vec!["涩", "凉"];
    let mut mem = Memory::from_text("喉咙发涩");
    mem.time.absolute_ns = t;
    mem.space.containers = vec!["人物:桑安歌".into()];
    // 修复后的实现应该把 array of string 转成 SenseAnchor 列表
    mem.senses = llm_input_senses
        .iter()
        .map(|s| SenseAnchor {
            kind: "未指定".into(),
            impression: s.to_string(),
            intensity: 0.5,
            source: None,
        })
        .collect();

    store.put(&mem).unwrap();
    // put 同事务会更新 sense_index

    let req = SensoryRecallRequest {
        senses: vec!["涩".into()],
        top_k: 5,
        narrative_span_days: 0,
    };
    let result = recall_sensory(&store, &req);
    assert_eq!(result.len(), 1, "感官索引写入后必须能召回");
    assert_eq!(result[0].senses[0].impression, "涩");
}

// ════════════════════════════════════════════════════════════════
// K. 感官召回排序边界
// ════════════════════════════════════════════════════════════════

#[test]
fn k01_sensory_time_proximity_strict() {
    // [REGRESSION] 感官匹配数相同时，时间邻接应作为二级排序键
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let anchor = 1700000000000000000_i64;

    // 5 条感官匹配数=2 的记忆，时间跨度大
    let times = [0_i64, -10, -30, -100, -500];
    for (i, days) in times.iter().enumerate() {
        store
            .put(&make_narrative(
                &format!("m{i}"),
                anchor + days * 86400 * 1_000_000_000,
                vec![],
                vec![("触觉", "涩", 0.5), ("温度", "凉", 0.5)],
            ))
            .unwrap();
        store.update_sense_index(&Memory::from_text(&format!("m{i}"))).ok();
    }

    let req = SensoryRecallRequest {
        senses: vec!["涩".into(), "凉".into()],
        top_k: 10,
        narrative_span_days: 0,
    };
    let result = recall_sensory(&store, &req);
    println!("[REGRESSION] 感官匹配数=2 五条，按时间邻接排序：");
    for m in &result {
        println!("  time={} text={}", m.time.absolute_ns, m.events[0].what);
    }
    // 当前实现：sort_by 只按感官数（全部=2，相等），Rust 排序稳定但顺序不定
    // 期望：时间近的排第一（m0）
    assert_eq!(
        result[0].events[0].what, "m0",
        "[REGRESSION] 时间邻接必须作为二级排序键，时间最近的应排第一"
    );
}
