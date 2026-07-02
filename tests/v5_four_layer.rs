//! taodb v0.5 专属测试 — 四层记忆模型
//!
//! 覆盖：
//! - boost_energy 持久化 + 多次累加
//! - energy_floor 永久下限
//! - energy_range 高能触发
//! - min_energy 过滤最终 window
//! - top_k 硬限制

use taodb::recall::recall_window_with_options;
use taodb::store::Store;
use taodb::{EmotionalMark, Memory, Query, SpatialCoord, TimeStamp};

fn make_memory_with_emotion(text: &str, time_ns: i64, intensity: f32, floor: f32) -> Memory {
    let mut mem = Memory::from_text(text);
    mem.time.absolute_ns = time_ns;
    mem.emotion = vec![EmotionalMark {
        time_offset_ns: 0,
        label: "情感".into(),
        intensity,
    }];
    mem.energy_floor = floor;
    mem
}

fn tempdir() -> tempdir::TempDir {
    tempdir::TempDir::new("taodb-v5-test").expect("create tempdir")
}

#[test]
fn test_boost_energy_persists_in_cache() {
    // 1) 写一条记忆
    // 2) recall 一次 → boost +0.05
    // 3) recall 第二次 → 看到 energy 累加
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let m = make_memory_with_emotion("boost test", now, 0.5, 0.0);
    store.put(&m).unwrap();
    store.decay_all().unwrap();
    let e0 = store.get_by_id(&m.id.to_string()).unwrap().potential_energy;

    let query = Query {
        text: "x".into(),
        context_time: TimeStamp::now(),
        context_space: SpatialCoord::default(),
        body_state: None,
    };
    let r1 = recall_window_with_options(&store, &query, 5, 30, 0.0);
    let e1 = r1.memories[0].potential_energy;

    let r2 = recall_window_with_options(&store, &query, 5, 30, 0.0);
    let e2 = r2.memories[0].potential_energy;

    let r3 = recall_window_with_options(&store, &query, 5, 30, 0.0);
    let e3 = r3.memories[0].potential_energy;

    // 每次 boost +0.05
    assert!((e1 - e0 - 0.05).abs() < 0.01, "e1={} e0={} delta={}", e1, e0, e1 - e0);
    assert!((e2 - e1 - 0.05).abs() < 0.01, "e2={} e1={} delta={}", e2, e1, e2 - e1);
    assert!((e3 - e2 - 0.05).abs() < 0.01, "e3={} e2={} delta={}", e3, e2, e3 - e2);
}

#[test]
fn test_boost_energy_persists_across_restart() {
    // 1) 写一条记忆
    // 2) recall 一次（boost +0.05，写入 disk）
    // 3) 重启 Store（从 disk reload）
    // 4) 看到 boost 后的 energy
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let m = make_memory_with_emotion("restart test", now, 0.5, 0.0);
    let id = m.id.to_string();
    store.put(&m).unwrap();
    store.decay_all().unwrap();
    let e0 = store.get_by_id(&id).unwrap().potential_energy;

    let query = Query {
        text: "x".into(),
        context_time: TimeStamp::now(),
        context_space: SpatialCoord::default(),
        body_state: None,
    };
    let _r1 = recall_window_with_options(&store, &query, 5, 30, 0.0);
    store.decay_all().unwrap(); // 持久化 boosted energy
    drop(store);

    // 重启 — 从 disk reload
    let store2 = Store::open(dir.path()).unwrap();
    let e1 = store2.get_by_id(&id).unwrap().potential_energy;
    assert!(
        (e1 - e0 - 0.05).abs() < 0.01,
        "boost not persisted: e0={} e1={}",
        e0,
        e1
    );
}

#[test]
fn test_energy_floor_prevents_decay() {
    // floor=0.5 的记忆, 365 天后 energy 应保持 >= 0.5
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let m = make_memory_with_emotion("permanent", now - 365 * 86400 * 1_000_000_000_i64, 0.3, 0.5);
    store.put(&m).unwrap();
    store.decay_all().unwrap();
    let e = store.get_by_id(&m.id.to_string()).unwrap().potential_energy;
    assert!(e >= 0.5, "floor not enforced: e={}", e);
    // 多次 decay 仍保持
    store.decay_all().unwrap();
    let e2 = store.get_by_id(&m.id.to_string()).unwrap().potential_energy;
    assert!(
        (e - e2).abs() < 0.01,
        "floor broken after multiple decay: e={} e2={}",
        e,
        e2
    );
}

#[test]
fn test_energy_range_returns_high_energy() {
    // 写 3 条：低能（0.1）、中能（0.5）、高能（0.9）
    // energy_range(min_energy=0.4) 应返回中能 + 高能
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);

    let m_low = make_memory_with_emotion("low energy", now, 0.1, 0.0);
    let m_mid = make_memory_with_emotion("mid energy", now, 0.5, 0.0);
    let m_high = make_memory_with_emotion("high energy", now, 0.9, 0.0);
    store.put(&m_low).unwrap();
    store.put(&m_mid).unwrap();
    store.put(&m_high).unwrap();
    store.decay_all().unwrap();

    let high = store.energy_range(0.4, 100);
    let count = high.len();
    let has_low = high.iter().any(|m| m.id == m_low.id);
    let has_mid = high.iter().any(|m| m.id == m_mid.id);
    let has_high = high.iter().any(|m| m.id == m_high.id);
    assert_eq!(count, 2, "expected 2 high-energy, got {}", count);
    assert!(!has_low, "low energy should be filtered");
    assert!(has_mid, "mid energy should be present");
    assert!(has_high, "high energy should be present");
}

#[test]
fn test_min_energy_filters_final_window() {
    // 写 3 条记忆：低能(0.1, 1天前) + 中能(0.5, 1天前) + 高能(0.9, 1天前)
    // recall with min_energy=0.4 应只返回中能+高能
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let day = 86400 * 1_000_000_000_i64;

    let m_low = make_memory_with_emotion("low", now - 1 * day, 0.1, 0.0);
    let m_mid = make_memory_with_emotion("mid", now - 1 * day, 0.5, 0.0);
    let m_high = make_memory_with_emotion("high", now - 1 * day, 0.9, 0.0);
    store.put(&m_low).unwrap();
    store.put(&m_mid).unwrap();
    store.put(&m_high).unwrap();
    store.decay_all().unwrap();

    let query = Query {
        text: "x".into(),
        context_time: TimeStamp::now(),
        context_space: SpatialCoord::default(),
        body_state: None,
    };
    let window = recall_window_with_options(&store, &query, 5, 30, 0.4);
    let has_low = window.memories.iter().any(|m| m.id == m_low.id);
    let has_mid = window.memories.iter().any(|m| m.id == m_mid.id);
    let has_high = window.memories.iter().any(|m| m.id == m_high.id);
    assert!(!has_low, "min_energy=0.4 should filter low (0.1) memory");
    assert!(has_mid, "mid (0.5) should pass min_energy=0.4");
    assert!(has_high, "high (0.9) should pass min_energy=0.4");
}

#[test]
fn test_top_k_hard_limit() {
    // 写 50 条记忆, recall top_k=3/5/10 应分别返回 3/5/10 条
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    for i in 0..50 {
        let m = make_memory_with_emotion(&format!("m{}", i), now - i as i64 * 86400 * 1_000_000_000_i64, 0.5, 0.0);
        store.put(&m).unwrap();
    }

    let query = Query {
        text: "x".into(),
        context_time: TimeStamp::now(),
        context_space: SpatialCoord::default(),
        body_state: None,
    };
    for k in [3, 5, 10, 20] {
        let w = recall_window_with_options(&store, &query, k, 30, 0.0);
        assert_eq!(
            w.memories.len(),
            k,
            "top_k={} expected {} got {}",
            k,
            k,
            w.memories.len()
        );
    }
}

#[test]
fn test_floor_overrides_min_energy() {
    // floor=0.8 的永久记忆, 即使 intensity=0.1 + 1000 天前, 仍 energy >= 0.8
    // recall with min_energy=0.7 应返回它
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let m = make_memory_with_emotion("permanent", now - 1000 * 86400 * 1_000_000_000_i64, 0.1, 0.8);
    store.put(&m).unwrap();
    store.decay_all().unwrap();
    let e = store.get_by_id(&m.id.to_string()).unwrap().potential_energy;
    assert!(e >= 0.8, "floor=0.8 broken: e={}", e);

    let query = Query {
        text: "x".into(),
        context_time: TimeStamp::now(),
        context_space: SpatialCoord::default(),
        body_state: None,
    };
    let w = recall_window_with_options(&store, &query, 5, 30, 0.7);
    let has_perm = w.memories.iter().any(|m_outer| m_outer.id == m.id);
    assert!(has_perm, "floor=0.8 memory should be retrieved with min_energy=0.7");
}

#[test]
fn test_boost_respects_max_one() {
    // boost 不能超过 1.0
    let dir = tempdir();
    let store = Store::open(dir.path()).unwrap();
    let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let m = make_memory_with_emotion("cap test", now, 1.0, 0.0);
    store.put(&m).unwrap();
    store.decay_all().unwrap();

    let query = Query {
        text: "x".into(),
        context_time: TimeStamp::now(),
        context_space: SpatialCoord::default(),
        body_state: None,
    };
    // 多次 boost
    for _ in 0..10 {
        let _ = recall_window_with_options(&store, &query, 5, 30, 0.0);
    }
    let e = store.get_by_id(&m.id.to_string()).unwrap().potential_energy;
    assert!(e <= 1.0, "boost overflow: e={}", e);
}
