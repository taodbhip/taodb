//! TaoDB v2 深度边界测试
//! 覆盖 v2_three_layer 未覆盖的边界场景

use std::collections::HashSet;
use taodb::model::{ConstraintRecallRequest, NarrativeRecallRequest, SensoryRecallRequest};
use taodb::recall::{recall_constraints, recall_narrative, recall_sensory};
use taodb::store::Store;
use taodb::{Memory, MemoryType, SenseAnchor, SpatialCoord};

fn tempdir() -> tempdir::TempDir {
    tempdir::TempDir::new("taodb-deep").unwrap()
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

#[test]
fn deep_01_sense_index_survives_reopen() {
    // 感官索引跨 store 生命周期持久化
    let dir = tempdir();
    let t = 1700000000000000000_i64;
    let id;
    {
        let store = Store::open(dir.path()).unwrap();
        let m = make_narrative("涩感", t, vec![], vec![("触觉", "涩", 0.9)]);
        id = m.id.to_string();
        store.put(&m).unwrap();
    }
    // reopen
    let store = Store::open(dir.path()).unwrap();
    let req = SensoryRecallRequest {
        senses: vec!["涩".into()],
        top_k: 10,
        narrative_span_days: 0,
    };
    let result = recall_sensory(&store, &req);
    assert_eq!(result.len(), 1, "reopen后感官索引应持久化");
    assert_eq!(result[0].id.to_string(), id);
}

#[test]
fn deep_02_narrative_filters_correctly_intersection() {
    // 验证叙事召回是 intersection 不是 union
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;

    store
        .put(&make_narrative(
            "桑安歌在邯郸",
            t,
            vec!["人物:桑安歌", "场景:邯郸"],
            vec![],
        ))
        .unwrap();
    store
        .put(&make_narrative(
            "柏正则在骊山",
            t,
            vec!["人物:柏正则", "场景:骊山"],
            vec![],
        ))
        .unwrap();
    store
        .put(&make_narrative("葵儿在邯郸", t, vec!["人物:葵儿", "场景:邯郸"], vec![]))
        .unwrap();

    // persons=[桑安歌] + locations=[邯郸] → intersection 应只有桑安歌在邯郸 (1条)
    let req = NarrativeRecallRequest {
        persons: vec!["桑安歌".into()],
        locations: vec!["邯郸".into()],
        objects: vec![],
        narrative_span_days: 3650,
        chapter_span: 0,
        top_k: 10,
        dimensions: vec![],
    };
    let window = recall_narrative(&store, &req);
    assert_eq!(window.memories.len(), 1, "persons+locations intersection");
    assert!(window.memories[0].events[0].what.contains("桑安歌"));
}

#[test]
fn deep_03_empty_store_all_layers() {
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();

    let c = recall_constraints(&store, &ConstraintRecallRequest::default());
    assert!(c.is_empty());

    let s = recall_sensory(
        &store,
        &SensoryRecallRequest {
            senses: vec!["涩".into()],
            top_k: 10,
            narrative_span_days: 0,
        },
    );
    assert!(s.is_empty());

    let n = recall_narrative(&store, &NarrativeRecallRequest::default());
    assert!(n.memories.is_empty());
}

#[test]
fn deep_04_duplicate_sense_put_is_idempotent() {
    // 同一记忆多次 put 不会在感官索引里重复
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;
    let m = make_narrative("涩", t, vec![], vec![("触觉", "涩", 0.9)]);

    for _ in 0..5 {
        store.put(&m).unwrap();
    }

    let req = SensoryRecallRequest {
        senses: vec!["涩".into()],
        top_k: 10,
        narrative_span_days: 0,
    };
    let result = recall_sensory(&store, &req);
    assert_eq!(result.len(), 1, "重复 put 应幂等");
}

#[test]
fn deep_05_constraint_mid_values() {
    // energy_floor 边界值: 0.0, 0.49, 0.5, 0.51, 1.0
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;

    for &floor in &[0.0_f32, 0.49, 0.5, 0.51, 1.0] {
        store
            .put(&make_constraint(&format!("f{floor}"), t, vec![], floor))
            .unwrap();
    }

    // min_floor=0.5: 应返回 floor>=0.5 的 (0.5, 0.51, 1.0) = 3条
    let req = ConstraintRecallRequest {
        min_floor: 0.5,
        top_k: 50,
    };
    let result = recall_constraints(&store, &req);
    assert_eq!(result.len(), 3, "min_floor=0.5 → 3条");
    // 排序验证
    assert!(result[0].energy_floor >= result[1].energy_floor);
    assert!(result[1].energy_floor >= result[2].energy_floor);

    // min_floor=0.0: 5条全返回
    let req2 = ConstraintRecallRequest {
        min_floor: 0.0,
        top_k: 50,
    };
    assert_eq!(recall_constraints(&store, &req2).len(), 5);
}

#[test]
fn deep_06_narrative_excludes_constraints_consistently() {
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;

    store.put(&make_constraint("规则", t, vec!["world"], 0.7)).unwrap();
    store
        .put(&make_narrative("叙事", t, vec!["人物:桑安歌"], vec![]))
        .unwrap();

    // 所有 narrative 召回都应排除约束
    for dims in [
        vec![],
        vec!["天".into()],
        vec!["地".into()],
        vec!["天".into(), "地".into()],
    ] {
        let req = NarrativeRecallRequest {
            persons: vec![],
            locations: vec![],
            objects: vec![],
            narrative_span_days: 3650,
            chapter_span: 0,
            top_k: 10,
            dimensions: dims,
        };
        let window = recall_narrative(&store, &req);
        for m in &window.memories {
            assert!(!m.is_constraint(), "维度 {:?} 不应返回约束记忆", &req.dimensions);
        }
    }
}

#[test]
fn deep_07_sensory_cross_impression_multi_match() {
    // 多感官查询按匹配数排序
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;

    store
        .put(&make_narrative("单涩", t, vec![], vec![("触觉", "涩", 0.9)]))
        .unwrap();
    store
        .put(&make_narrative(
            "涩凉颤",
            t,
            vec![],
            vec![("触觉", "涩", 0.9), ("温度", "凉", 0.8), ("触觉", "颤", 0.7)],
        ))
        .unwrap();
    store
        .put(&make_narrative(
            "涩凉",
            t,
            vec![],
            vec![("触觉", "涩", 0.9), ("温度", "凉", 0.8)],
        ))
        .unwrap();

    let req = SensoryRecallRequest {
        senses: vec!["涩".into(), "凉".into(), "颤".into()],
        top_k: 10,
        narrative_span_days: 0,
    };
    let result = recall_sensory(&store, &req);
    assert_eq!(result.len(), 3);
    // 匹配最多的排第一
    let count0 = result[0]
        .senses
        .iter()
        .filter(|s| ["涩", "凉", "颤"].contains(&s.impression.as_str()))
        .count();
    let count2 = result[2]
        .senses
        .iter()
        .filter(|s| ["涩", "凉", "颤"].contains(&s.impression.as_str()))
        .count();
    assert!(count0 >= count2, "多感官匹配数 DESC");
}

#[test]
fn deep_08_forget_cleans_all_traces() {
    // forget 后：memories 表无、timeline 无、sense_index 无、缓存无
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;
    let m = make_narrative("X", t, vec![], vec![("触觉", "涩", 0.9)]);
    let id = m.id.to_string();
    store.put(&m).unwrap();

    assert!(store.forget(&id));

    // 查 memory
    assert!(store.get_by_id(&id).is_none(), "memory 表应删除");
    // 查感官索引（不应返回已删除的记忆）
    let req = SensoryRecallRequest {
        senses: vec!["涩".into()],
        top_k: 10,
        narrative_span_days: 0,
    };
    let result = recall_sensory(&store, &req);
    assert!(result.iter().all(|r| r.id.to_string() != id), "感官索引不应残留死 ID");
}

#[test]
fn deep_09_bincode_roundtrip_all_fields() {
    // 完整 Memory（含所有 v2 字段）序列化往返
    let mut mem = Memory::from_text("test");
    mem.memory_type = MemoryType::Constraint;
    mem.chapter_index = "卷二/第152回".into();
    mem.senses = vec![
        SenseAnchor {
            kind: "触觉".into(),
            impression: "涩".into(),
            intensity: 0.9,
            source: Some("鼓".into()),
        },
        SenseAnchor {
            kind: "温度".into(),
            impression: "凉".into(),
            intensity: 0.6,
            source: None,
        },
    ];
    mem.energy_floor = 0.7;
    mem.time.absolute_ns = 1782532456372428000;
    mem.space.containers = vec!["人物:桑安歌".into(), "场景:邯郸酒肆".into()];

    let raw = bincode::serialize(&mem).unwrap();
    let restored: Memory = bincode::deserialize(&raw).unwrap();

    assert_eq!(restored.memory_type, MemoryType::Constraint);
    assert_eq!(restored.chapter_index, "卷二/第152回");
    assert_eq!(restored.senses.len(), 2);
    assert_eq!(restored.senses[0].impression, "涩");
    assert_eq!(restored.senses[0].source.as_deref(), Some("鼓"));
    assert_eq!(restored.senses[1].impression, "凉");
    assert_eq!(restored.energy_floor, 0.7);
    assert_eq!(restored.time.absolute_ns, 1782532456372428000);
}

#[test]
fn deep_10_large_sense_index_recovery() {
    // 大量感官索引的极限测试：100 条不同感官的记忆
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;
    let impressions: Vec<String> = (0..100).map(|i| format!("感{}", i)).collect();

    for imp in &impressions {
        let m = make_narrative(imp, t, vec![], vec![("触觉", imp, 0.8)]);
        store.put(&m).unwrap();
    }

    // 每条感官只召回自己
    for imp in impressions.iter().take(20) {
        let req = SensoryRecallRequest {
            senses: vec![imp.clone()],
            top_k: 10,
            narrative_span_days: 0,
        };
        let result = recall_sensory(&store, &req);
        assert_eq!(result.len(), 1, "感官'{}'应只召回1条", imp);
    }
}

#[test]
fn deep_11_narrative_time_window_filter() {
    // 时间窗应过滤掉太远的记忆
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let now = 1700000000000000000_i64;

    store
        .put(&make_narrative("现在", now, vec!["人物:桑安歌"], vec![]))
        .unwrap();
    store
        .put(&make_narrative(
            "10天前",
            now - 10 * 86400 * 1_000_000_000,
            vec!["人物:桑安歌"],
            vec![],
        ))
        .unwrap();
    store
        .put(&make_narrative(
            "100天前",
            now - 100 * 86400 * 1_000_000_000,
            vec!["人物:桑安歌"],
            vec![],
        ))
        .unwrap();

    let req = NarrativeRecallRequest {
        persons: vec!["桑安歌".into()],
        locations: vec![],
        objects: vec![],
        narrative_span_days: 20,
        chapter_span: 0,
        top_k: 10,
        dimensions: vec![],
    };
    let window = recall_narrative(&store, &req);
    assert_eq!(window.memories.len(), 2, "span=20应过滤100天前");
}

#[test]
fn deep_12_http_api_response_shapes() {
    // 验证 HTTP API handler 返回的 JSON 形状
    let api_src = std::fs::read_to_string("src/api/mod.rs").unwrap();

    // 验证 v2 路由注册
    assert!(api_src.contains("/v1/recall/constraints"), "缺少 constraints 路由");
    assert!(api_src.contains("/v1/recall/sensory"), "缺少 sensory 路由");
    assert!(api_src.contains("/v1/recall/narrative"), "缺少 narrative 路由");

    // 验证 handler 函数存在
    assert!(api_src.contains("fn handle_recall_constraints"), "缺少 handler");
    assert!(api_src.contains("fn handle_recall_sensory"), "缺少 handler");
    assert!(api_src.contains("fn handle_recall_narrative"), "缺少 handler");

    // 验证 responses are pruned (not raw Memory)
    let has_full_memory_in_response = api_src.contains("\"memories\": memories")  // raw Memory dump
        || api_src.contains("\"memories\":memories"); // variable name dump
    // HTTP handlers should NOT dump raw Memory
    let constraints_handler = api_src.find("fn handle_recall_constraints").unwrap();
    let constraints_end = api_src[constraints_handler..]
        .find("fn handle_recall_sensory")
        .unwrap_or(api_src.len());
    let constraints_body = &api_src[constraints_handler..constraints_handler + constraints_end];
    assert!(
        !constraints_body.contains("\"memories\": memories"),
        "HTTP constraints handler 不应 dump 原始 Memory"
    );
    assert!(
        constraints_body.contains("\"id\":"),
        "HTTP handler 应返回剪枝后的轻量对象"
    );
}

#[test]
fn deep_13_mcp_schema_completeness() {
    // 验证所有 v2 MCP 工具在 tools() 中注册
    let mcp_src = std::fs::read_to_string("src/mcp.rs").unwrap();
    assert!(
        mcp_src.contains("taodb_recall_constraints"),
        "MCP 缺少 recall_constraints 工具注册"
    );
    assert!(
        mcp_src.contains("taodb_recall_sensory"),
        "MCP 缺少 recall_sensory 工具注册"
    );
    // 验证 dispatch 中有 handler
    assert!(
        mcp_src.contains("\"taodb_recall_constraints\" =>"),
        "MCP dispatch 缺少 constraints handler"
    );
    assert!(
        mcp_src.contains("\"taodb_recall_sensory\" =>"),
        "MCP dispatch 缺少 sensory handler"
    );
}

#[test]
fn deep_14_energy_floor_exactly_at_boundary() {
    // energy_floor 精确等于 min_floor 应被包含 (>=)
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;

    store.put(&make_constraint("exact", t, vec![], 0.7)).unwrap();

    let req = ConstraintRecallRequest {
        min_floor: 0.7,
        top_k: 50,
    };
    let result = recall_constraints(&store, &req);
    assert_eq!(result.len(), 1, "floor == min_floor 应被包含");
}

#[test]
fn deep_15_sense_index_with_forget_and_reput() {
    // forget → re-put 同一感官的另一个记忆 → 索引正确
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let t = 1700000000000000000_i64;

    let m1 = make_narrative("旧涩", t, vec![], vec![("触觉", "涩", 0.9)]);
    let id1 = m1.id.to_string();
    store.put(&m1).unwrap();
    store.forget(&id1);

    // re-put 新涩感
    let m2 = make_narrative("新涩", t, vec![], vec![("触觉", "涩", 0.8)]);
    store.put(&m2).unwrap();

    let req = SensoryRecallRequest {
        senses: vec!["涩".into()],
        top_k: 10,
        narrative_span_days: 0,
    };
    let result = recall_sensory(&store, &req);
    assert_eq!(result.len(), 1, "forget后re-put应只返回新记忆");
    assert_eq!(result[0].id.to_string(), m2.id.to_string());
    assert!(result[0].events[0].what.contains("新涩"), "应返回新记忆而非旧记忆");
}
