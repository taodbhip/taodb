//! taodb 端到端测试 (v0.3 — 纯时空召回)

use taodb::recall::recall_window;
use taodb::store::Store;
use taodb::{EmotionalMark, Memory, Query, SpatialCoord, TimeStamp};

fn make_memory(text: &str, time_ns: i64, containers: Vec<&str>) -> Memory {
    let mut mem = Memory::from_text(text);
    mem.time.absolute_ns = time_ns;
    mem.space = SpatialCoord {
        containers: containers.into_iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    };
    mem
}

#[test]
fn test_store_write_and_load() {
    let dir = tempdir();
    let store = Store::open(dir.path()).expect("open store");
    let m1 = make_memory("苹果记忆", -365 * 86400 * 1_000_000_000_i64, vec!["厨房", "家"]);
    let m2 = make_memory("战斗记忆", -730 * 86400 * 1_000_000_000_i64, vec!["长平", "战场"]);
    let id1 = m1.id.to_string();
    let id2 = m2.id.to_string();
    store.put(&m1).expect("put 1");
    store.put(&m2).expect("put 2");
    assert_eq!(store.count(), 2);
    drop(store);
    let store2 = Store::open(dir.path()).expect("reopen");
    assert_eq!(store2.count(), 2);
    let all = store2.all();
    let ids: Vec<String> = all.iter().map(|m| m.id.to_string()).collect();
    assert!(ids.contains(&id1));
    assert!(ids.contains(&id2));
}

#[test]
fn test_recall_time_proximity() {
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    // 写三条: 今天, 一年前, 十年前
    let m1 = make_memory(
        "今天的记忆",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
        vec!["家"],
    );
    let m2 = make_memory(
        "一年前的记忆",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0) - 365 * 86400 * 1_000_000_000_i64,
        vec!["家"],
    );
    let m3 = make_memory(
        "十年前的记忆",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0) - 3650 * 86400 * 1_000_000_000_i64,
        vec!["远方"],
    );
    store.put(&m1).unwrap();
    store.put(&m2).unwrap();
    store.put(&m3).unwrap();

    // Query 今天附近
    let query = Query {
        text: "test".into(),
        context_time: TimeStamp::now(),
        context_space: SpatialCoord::default(),
        body_state: None,
    };
    let window = recall_window(&store, &query, 3);
    assert!(!window.memories.is_empty());
    // 今天的记忆应该第一
    assert_eq!(window.memories[0].id, m1.id);
}

#[test]
fn test_recall_space_proximity() {
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    // 不同时间戳，避免 1 年窗口误捕
    let m1 = make_memory("厨房的故事", now, vec!["厨房", "家"]);
    let m2 = make_memory(
        "战场的回忆",
        now - 800 * 86400 * 1_000_000_000_i64,
        vec!["战场", "长平"],
    );
    let m3 = make_memory(
        "灶台边的事",
        now - 100 * 86400 * 1_000_000_000_i64,
        vec!["厨房", "灶台"],
    );
    store.put(&m1).unwrap();
    store.put(&m2).unwrap();
    store.put(&m3).unwrap();

    // Query 厨房，看空间优先
    let query = Query {
        text: "test".into(),
        context_time: TimeStamp {
            absolute_ns: now,
            ..Default::default()
        },
        context_space: SpatialCoord {
            containers: vec!["厨房".to_string()],
            ..Default::default()
        },
        body_state: None,
    };
    let window = recall_window(&store, &query, 3);
    assert!(window.memories.len() >= 1);
    let has_kitchen = window
        .memories
        .iter()
        .any(|m| m.space.containers.contains(&"厨房".to_string()));
    assert!(has_kitchen);
}

#[test]
fn test_potential_energy_natural_decay() {
    let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let e_fresh = taodb::potential_energy(now, 1.0, 1.0);
    let e_month = taodb::potential_energy(now - 30 * 86400 * 1_000_000_000_i64, 1.0, 1.0);
    let e_year = taodb::potential_energy(now - 365 * 86400 * 1_000_000_000_i64, 1.0, 1.0);
    assert!(e_fresh > e_month);
    assert!(e_month > e_year);
}

#[test]
fn test_decay_function_on_memory() {
    let mut mem = make_memory("test", 0, vec![]);
    mem.emotion = vec![EmotionalMark {
        time_offset_ns: 0,
        label: "温暖".into(),
        intensity: 0.9,
    }];
    mem.potential_energy = 0.0;
    taodb::decay(&mut mem);
    assert!(mem.potential_energy > 0.0);
}

#[test]
fn test_forget_memory() {
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let m = make_memory("to be forgotten", 0, vec![]);
    let id = m.id.to_string();
    store.put(&m).unwrap();
    assert_eq!(store.count(), 1);
    assert!(store.forget(&id));
    assert_eq!(store.count(), 0);
    assert!(store.get_by_id(&id).is_none());
}

fn tempdir() -> tempdir::TempDir {
    tempdir::TempDir::new("taodb-test").expect("create tempdir")
}
