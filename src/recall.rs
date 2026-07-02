//! 召回 — 多维度时空展开
//!
//! 五维检索（道/天/地/人/物），LLM 驱动维度组合。
//! 核心改变：锚点从墙上时钟移到数据内部——从匹配 containers 的最新记忆推导。

use crate::model::{
    ConstraintRecallRequest, Memory, MemoryScore, NarrativeRecallRequest, Query, RecallWindow, SensoryRecallRequest,
    TimeStamp,
};
use crate::store::Store;
use std::collections::HashSet;

const RECONSOLIDATION_BOOST: f32 = 0.05;

// ── 第一层: 约束层召回 (Shadow-Loom: WorldModel constraints) ──
// 会话启动时自动加载。返回所有 energy_floor >= min_floor 的记忆。
// 这些记忆永不衰减，永远在 LLM 上下文里。

pub fn recall_constraints(store: &Store, req: &ConstraintRecallRequest) -> Vec<Memory> {
    let mut result: Vec<Memory> = store
        .all()
        .into_iter()
        .filter(|m| m.is_constraint() && m.energy_floor >= req.min_floor)
        .collect();
    result.sort_by(|a, b| {
        b.energy_floor
            .partial_cmp(&a.energy_floor)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    result.truncate(req.top_k);
    result
}

// ── 第二层: 感官触发召回 (Proust: involuntary memory) ──
// LLM 在写作时发现当前场景出现某个感官状态 → 调用此函数。
// TaoDB 从感官索引中找出所有共享该感官的记忆 —— 不论人物、场景、时间。
// 感官是跨容器、跨时间、跨人物的索引维度。

pub fn recall_sensory(store: &Store, req: &SensoryRecallRequest) -> Vec<Memory> {
    if req.senses.is_empty() {
        return vec![];
    }
    store.sensory_recall(&req.senses, req.top_k, req.narrative_span_days)
}

// ── 第三层: 叙事时空召回 (现有多维召回的重构入口) ──
// 天/地/人/物 四维并行 → 合并去重 → 排序
// 不含约束层记忆 (约束层由 recall_constraints 单独返回)

pub fn recall_narrative(store: &Store, req: &NarrativeRecallRequest) -> RecallWindow {
    let mut containers = vec![];
    for p in &req.persons {
        containers.push(format!("人物:{}", p));
    }
    for l in &req.locations {
        containers.push(format!("场景:{}", l));
    }
    for o in &req.objects {
        containers.push(format!("物件:{}", o));
    }

    let query = Query {
        text: String::new(),
        context_time: TimeStamp::now(),
        context_space: crate::model::SpatialCoord {
            containers,
            ..Default::default()
        },
        body_state: None,
    };

    let dims = if req.dimensions.is_empty() {
        vec![]
    } else {
        req.dimensions.clone()
    };

    let mut window = recall_multidimensional(
        store,
        &query,
        req.top_k,
        req.narrative_span_days,
        0.0, // min_energy: 不召回约束层 (约束层单独调用)
        &dims,
    );

    // 过滤掉约束层记忆 (以防 energy_floor 设得低混进来了)
    window.memories.retain(|m| !m.is_constraint());
    // P0-4 fix: post-filter by requested persons/locations/objects (intersection across types, union within type)
    let has_person = !req.persons.is_empty();
    let has_location = !req.locations.is_empty();
    let has_object = !req.objects.is_empty();
    if has_person || has_location || has_object {
        let person_tags: Vec<String> = req.persons.iter().map(|p| format!("人物:{}", p)).collect();
        let location_tags: Vec<String> = req.locations.iter().map(|l| format!("场景:{}", l)).collect();
        let object_tags: Vec<String> = req.objects.iter().map(|o| format!("物件:{}", o)).collect();

        window.memories.retain(|m| {
            let containers = &m.space.containers;
            let match_person = !has_person
                || person_tags
                    .iter()
                    .any(|t| containers.iter().any(|c| c.contains(t) || t.contains(c.as_str())));
            let match_location = !has_location
                || location_tags
                    .iter()
                    .any(|t| containers.iter().any(|c| c.contains(t) || t.contains(c.as_str())));
            let match_object = !has_object
                || object_tags
                    .iter()
                    .any(|t| containers.iter().any(|c| c.contains(t) || t.contains(c.as_str())));
            match_person && match_location && match_object
        });
    }
    // P0-4 fix: also apply narrative time window in post-filter
    // (recall_multidimensional "地" dimension returns all spatial matches regardless of time)
    if req.narrative_span_days > 0 {
        let anchor_ns = derive_narrative_anchor(store, &[]);
        let span_ns = req.narrative_span_days * 86400 * 1_000_000_000_i64;
        window
            .memories
            .retain(|m| (m.time.absolute_ns - anchor_ns).abs() <= span_ns);
    }
    window
}
const DEFAULT_NARRATIVE_SPAN_DAYS: i64 = 30;

// ── 旧版兼容接口 ──

pub fn recall_window(store: &Store, query: &Query, window_size: usize) -> RecallWindow {
    recall_window_with_options(store, query, window_size, DEFAULT_NARRATIVE_SPAN_DAYS, 0.0)
}

pub fn recall_window_with_days(store: &Store, query: &Query, seed_count: usize, window_days: i64) -> RecallWindow {
    recall_window_with_options(store, query, seed_count, window_days, 0.0)
}

/// 旧版单维接口（保留兼容，内部改为多维展开）
pub fn recall_window_with_options(
    store: &Store,
    query: &Query,
    seed_count: usize,
    window_days: i64,
    min_energy: f32,
) -> RecallWindow {
    recall_multidimensional(
        store,
        query,
        seed_count,
        window_days,
        min_energy,
        &[], // 激活全部维度
    )
}

// ── 多维召回核心 ──

/// 多维度时空展开
///
/// 1. 推导叙事锚点：从匹配 containers 的最新记忆中取 time_ns
/// 2. 沿各维度并行展开：
///    - 天: 叙事时间轴 ± narrative_span_days
///    - 地: 空间 containers 重叠
///    - 道: 高能永久记忆 (min_energy)
///    - 人: 身体/情感同源（同 POV + 同身体部位）
///    - 物: 物件链（同 event.with）
/// 3. 合并、去重、按多维评分排序
pub fn recall_multidimensional(
    store: &Store,
    query: &Query,
    top_k: usize,
    narrative_span_days: i64,
    min_energy: f32,
    active_dimensions: &[String],
) -> RecallWindow {
    let all_dims = active_dimensions.is_empty();
    let dim = |name: &str| all_dims || active_dimensions.iter().any(|d| d == name);

    let mut recall_paths: Vec<String> = vec![];
    let mut all_memories: Vec<Memory> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // ── 锚点推导 ──
    let context_containers: Vec<String> = query.context_space.containers.clone();
    let anchor_ns = derive_narrative_anchor(store, &context_containers);
    recall_paths.push(format!("anchor: {} (containers: {:?})", anchor_ns, context_containers));

    // ── 天: 叙事时间轴展开 ──
    if dim("天") {
        let from = anchor_ns - narrative_span_days * 86400 * 1_000_000_000_i64;
        let to = anchor_ns + narrative_span_days * 86400 * 1_000_000_000_i64;
        let time_ids = store.time_range(from, to, top_k * 3);
        recall_paths.push(format!("天: time_range [{}, {}] → {} hits", from, to, time_ids.len()));
        for id in &time_ids {
            if !seen.contains(id) {
                seen.insert(id.clone());
                if let Some(m) = store.get_by_id(id) {
                    all_memories.push(m);
                }
            }
        }
    }

    // ── 地: 空间 containers 重叠 ──
    if dim("地") && !context_containers.is_empty() {
        let mut spatial: Vec<Memory> = store
            .all()
            .into_iter()
            .filter(|m| m.space.containers.iter().any(|c| context_containers.contains(c)))
            .collect();
        recall_paths.push(format!("地: container_overlap → {} hits", spatial.len()));
        for m in spatial.drain(..) {
            if !seen.contains(&m.id.to_string()) {
                seen.insert(m.id.to_string());
                all_memories.push(m);
            }
        }
    }

    // ── 道: 高能永久记忆 ──
    if dim("道") && min_energy > 0.0 {
        let high_energy = store.energy_range(min_energy, top_k * 2);
        recall_paths.push(format!("道: energy > {} → {} hits", min_energy, high_energy.len()));
        for m in high_energy {
            if !seen.contains(&m.id.to_string()) {
                seen.insert(m.id.to_string());
                all_memories.push(m);
            }
        }
    }

    // ── 人: 身体/情感同源（同 containers 中含 "人物:" 前缀的） ──
    if dim("人") && !context_containers.is_empty() {
        let pov_tags: Vec<&str> = context_containers
            .iter()
            .filter(|c| c.starts_with("人物:") || c.starts_with("关系:"))
            .map(|s| s.as_str())
            .collect();
        if !pov_tags.is_empty() {
            let mut body_memories: Vec<Memory> = store
                .all()
                .into_iter()
                .filter(|m| {
                    m.space.containers.iter().any(|c| pov_tags.contains(&c.as_str()))
                        && (!m.bodies.is_empty() || !m.emotion.is_empty())
                })
                .collect();
            recall_paths.push(format!(
                "人: pov_tags={:?}, has_body/emotion → {} hits",
                pov_tags,
                body_memories.len()
            ));
            for m in body_memories.drain(..) {
                if !seen.contains(&m.id.to_string()) {
                    seen.insert(m.id.to_string());
                    all_memories.push(m);
                }
            }
        }
    }

    // ── 物: 物件链（同 event.with + containers 中包含 "物件:"） ──
    if dim("物") && !context_containers.is_empty() {
        let object_tags: Vec<&str> = context_containers
            .iter()
            .filter(|c| c.starts_with("物件:"))
            .map(|s| s.as_str())
            .collect();
        if !object_tags.is_empty() {
            let mut object_memories: Vec<Memory> = store
                .all()
                .into_iter()
                .filter(|m| {
                    m.events.iter().any(|ev| {
                        ev.with
                            .as_ref()
                            .is_some_and(|w| object_tags.iter().any(|t| t.contains(w.as_str()) || w.contains(t)))
                    }) || m.space.containers.iter().any(|c| object_tags.contains(&c.as_str()))
                })
                .collect();
            recall_paths.push(format!(
                "物: object_tags={:?} → {} hits",
                object_tags,
                object_memories.len()
            ));
            for m in object_memories.drain(..) {
                if !seen.contains(&m.id.to_string()) {
                    seen.insert(m.id.to_string());
                    all_memories.push(m);
                }
            }
        }
    }

    if all_memories.is_empty() {
        recall_paths.push("result: empty".into());
        return RecallWindow {
            memories: vec![],
            time_range: None,
            space_scope: Some(context_containers),
            field_density: 0.0,
            emergent_associations: vec![],
            recall_paths,
            scoring_breakdown: vec![],
        };
    }

    // ── 多维评分排序 ──
    let context_set: HashSet<String> = context_containers.into_iter().collect();

    // 五维权重：dimensions 激活影响权重分配
    let w_time: f32 = if dim("天") { 3.0 } else { 1.0 }; // 天: 时间距离权重
    let w_space: f32 = if dim("地") { 2.0 } else { 0.5 }; // 地: 空间重合权重
    let w_energy: f32 = if dim("道") { 2.0 } else { 1.0 }; // 道: 能量权重
    let w_body: f32 = if dim("人") { 2.0 } else { 0.5 }; // 人: 身体/情感权重
    let w_text: f32 = 0.5; // query 文本匹配权重

    // query 分词（轻量 token 匹配，不做语义搜索）
    let query_tokens: HashSet<String> = query
        .text
        .split(|c: char| c.is_whitespace() || c == '，' || c == '。' || c == '、')
        .filter(|t| t.len() >= 2)
        .map(|t| t.to_lowercase())
        .collect();

    let mut scored: Vec<(&Memory, f32, MemoryScore)> = all_memories
        .iter()
        .map(|m| {
            let mut score = 0.0_f32;

            // 天: 叙事时间距离
            let dt = (m.time.absolute_ns - anchor_ns).abs() as f64;
            let narrative_proximity = 1.0 / (1.0 + dt / (86400.0 * 1e9));
            let time_score = narrative_proximity as f32 * w_time;
            score += time_score;

            // 地: 空间 containers 重合
            let overlap = m.space.containers.iter().filter(|c| context_set.contains(*c)).count();
            let space_score = overlap as f32 * w_space;
            score += space_score;

            // 道: 能量
            let energy_score = m.potential_energy * w_energy;
            score += energy_score;

            // 人: 身体/情感丰富度
            let mut body_emotion_score = 0.0_f32;
            if !m.bodies.is_empty() {
                body_emotion_score += w_body;
            }
            if !m.emotion.is_empty() {
                body_emotion_score += w_body * 0.5;
            }
            score += body_emotion_score;

            // query 文本匹配（轻量 token overlap）
            let memory_text: String = m.events.iter().map(|e| e.what.as_str()).collect::<Vec<_>>().join(" ");
            let text_tokens: HashSet<String> = memory_text
                .split(|c: char| c.is_whitespace() || c == '，' || c == '。')
                .filter(|t| t.len() >= 2)
                .map(|t| t.to_lowercase())
                .collect();
            let token_overlap = query_tokens.intersection(&text_tokens).count();
            let text_score = if !query_tokens.is_empty() {
                token_overlap as f32 / query_tokens.len() as f32 * w_text
            } else {
                0.0
            };
            score += text_score;

            // why: LLM 可读的召回原因
            let mut reasons: Vec<String> = Vec::new();
            if time_score > 0.5 {
                reasons.push(format!("时间距离近(分:{:.1})", time_score));
            }
            if space_score > 0.0 {
                reasons.push(format!("空间重合{}个容器", overlap));
            }
            if energy_score > 0.1 {
                reasons.push(format!("能量较高({:.2})", m.potential_energy));
            }
            if body_emotion_score > 0.0 {
                reasons.push("含身体/情感标记".into());
            }
            if text_score > 0.0 {
                reasons.push(format!("文本匹配{}个词", token_overlap));
            }
            let why = if reasons.is_empty() {
                "无特别匹配".into()
            } else {
                reasons.join("; ")
            };

            let breakdown = MemoryScore {
                memory_id: m.id.to_string(),
                total_score: 0.0, // 后续填入
                narrative_proximity: time_score,
                container_overlap: space_score,
                energy_score,
                body_emotion_bonus: body_emotion_score,
                text_match_score: text_score,
                why,
            };

            (m, score, breakdown)
        })
        .collect();

    scored.sort_by(|(_, a, _), (_, b, _)| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    let top: Vec<(&Memory, &MemoryScore)> = scored.iter().take(top_k).map(|(m, _, s)| (*m, s)).collect();

    let seed_ids: HashSet<String> = top.iter().map(|(m, _)| m.id.to_string()).collect();
    let mut window: Vec<Memory> = if dim("天") {
        store.window_memories(&seed_ids, narrative_span_days, top_k * 2)
    } else {
        top.iter().map(|(m, _)| (*m).clone()).collect()
    };

    // ── 双模式：当同时指定叙事窗口和高能阈值时，分路标注 ──
    let has_time_window = dim("天") && narrative_span_days < 3650;
    let has_energy_filter = dim("道") && min_energy > 0.0;
    let dual_mode = has_time_window && has_energy_filter;

    if dual_mode {
        recall_paths.push(format!(
            "双模式召回: 时间窗±{}天 + 能量>{:.1}",
            narrative_span_days, min_energy
        ));
    }

    // 合并高能记忆（不在窗口中的）
    if dim("道") && min_energy > 0.0 {
        let window_ids: HashSet<String> = window.iter().map(|m| m.id.to_string()).collect();
        for m in store.energy_range(min_energy, top_k) {
            if !window_ids.contains(&m.id.to_string()) {
                if dual_mode {
                    recall_paths.push(format!(
                        "  + 高能补充: {} (energy={:.2}, containers={:?})",
                        m.events
                            .first()
                            .map(|e| e.what.chars().take(40).collect::<String>())
                            .unwrap_or_default(),
                        m.potential_energy,
                        m.space.containers
                    ));
                }
                window.push(m);
                if window.len() >= top_k {
                    break;
                }
            }
        }
    }

    // 去重
    let mut dedup = HashSet::new();
    window.retain(|m| dedup.insert(m.id.to_string()));

    // 能量过滤
    if dim("道") && min_energy > 0.0 {
        window.retain(|m| m.potential_energy > min_energy);
    }

    // top_k 硬限制
    if window.len() > top_k {
        window.truncate(top_k);
    }

    // 重新巩固
    let recalled_ids: Vec<String> = window.iter().map(|m| m.id.to_string()).collect();
    store.boost_energy(&recalled_ids, RECONSOLIDATION_BOOST);

    // 从 cache 重读 boosted values
    let id_set: HashSet<String> = recalled_ids.into_iter().collect();
    window = id_set.iter().filter_map(|id| store.get_cached(id)).collect();

    // ── 构建评分明细 ──
    let window_ids: HashSet<String> = window.iter().map(|m| m.id.to_string()).collect();
    let mut scoring_breakdown: Vec<MemoryScore> = scored
        .iter()
        .filter(|(m, _, _)| window_ids.contains(&m.id.to_string()))
        .map(|(m, _, s)| {
            let mut s = s.clone();
            s.total_score = scored
                .iter()
                .find(|(sm, _, _)| sm.id == m.id)
                .map(|(_, score, _)| *score)
                .unwrap_or(0.0);
            s
        })
        .collect();
    // 补充直接来自 energy_range 的高能记忆（未在 scored 中）
    for m in &window {
        if !scoring_breakdown.iter().any(|s| s.memory_id == m.id.to_string()) {
            scoring_breakdown.push(MemoryScore {
                memory_id: m.id.to_string(),
                total_score: m.potential_energy,
                narrative_proximity: 0.0,
                container_overlap: 0.0,
                energy_score: m.potential_energy,
                body_emotion_bonus: 0.0,
                text_match_score: 0.0,
                why: format!("高能记忆补充(energy={:.2})", m.potential_energy),
            });
        }
    }

    if window.is_empty() {
        recall_paths.push("result: empty after window expansion".into());
        return RecallWindow {
            memories: vec![],
            time_range: None,
            space_scope: Some(context_set.into_iter().collect()),
            field_density: 0.0,
            emergent_associations: vec![],
            recall_paths,
            scoring_breakdown: vec![],
        };
    }

    let time_range = if window.len() > 1 {
        let mut times: Vec<i64> = window.iter().map(|m| m.time.absolute_ns).collect();
        times.sort();
        Some((
            TimeStamp {
                absolute_ns: times[0],
                ..Default::default()
            },
            TimeStamp {
                absolute_ns: *times.last().unwrap(),
                ..Default::default()
            },
        ))
    } else {
        None
    };
    let space_scope: Vec<String> = window
        .iter()
        .flat_map(|m| m.space.containers.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    recall_paths.push(format!("result: {} memories", window.len()));

    RecallWindow {
        memories: window,
        time_range,
        space_scope: Some(space_scope),
        field_density: 1.0,
        emergent_associations: vec![],
        recall_paths,
        scoring_breakdown,
    }
}

// ── 锚点推导 ──

/// 从匹配 containers 的最新记忆中推导叙事锚点时间
///
/// 优先级:
/// 1. 匹配 containers 的记忆中 absolute_ns 最大的（最近的叙事时间）
/// 2. 如果没有匹配，取全部记忆中 absolute_ns 最大的
/// 3. 如果 store 为空，回退到墙上时钟
pub fn derive_narrative_anchor(store: &Store, containers: &[String]) -> i64 {
    if containers.is_empty() {
        // 无空间约束：取全部记忆中最新的叙事时间
        let all = store.all();
        if let Some(latest) = all.iter().map(|m| m.time.absolute_ns).max() {
            return latest;
        }
        return chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    }

    // 有空间约束：取匹配 containers 的最近叙事时间
    let matching: Vec<i64> = store
        .all()
        .into_iter()
        .filter(|m| m.space.containers.iter().any(|c| containers.contains(c)))
        .map(|m| m.time.absolute_ns)
        .collect();

    if let Some(latest) = matching.iter().max() {
        return *latest;
    }

    // 无匹配：回退到全部记忆
    let all = store.all();
    if let Some(latest) = all.iter().map(|m| m.time.absolute_ns).max() {
        return latest;
    }

    chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use tempdir::TempDir;

    fn setup_store() -> (Store, TempDir) {
        let dir = TempDir::new("taodb-recall-test").unwrap();
        let store = Store::open(dir.path()).unwrap();
        (store, dir)
    }

    fn make_memory(id: &str, time_ns: i64, containers: Vec<&str>, text: &str) -> Memory {
        let mut mem = Memory::from_text(text);
        mem.time.absolute_ns = time_ns;
        mem.time.era = "卷二_稷下之崩".into();
        mem.space.containers = containers.into_iter().map(|s| s.to_string()).collect();
        // Use a fixed ULID by embedding it in text; simpler: just use put which assigns real ULID
        mem
    }

    #[test]
    fn anchor_derives_from_latest_matching_memory() {
        let (store, _dir) = setup_store();
        let m1 = make_memory("a", 1701302400000000000, vec!["人物:桑安歌"], "ch141");
        let m2 = make_memory("b", 1701648000000000000, vec!["人物:桑安歌"], "ch146");
        store.put(&m1).unwrap();
        store.put(&m2).unwrap();

        let anchor = derive_narrative_anchor(&store, &["人物:桑安歌".into()]);
        assert_eq!(anchor, 1701648000000000000); // ch146, latest narrative
    }

    #[test]
    fn anchor_falls_back_to_global_max_when_no_match() {
        let (store, _dir) = setup_store();
        let m1 = make_memory("a", 1701302400000000000, vec!["人物:柏正则"], "ch132");
        store.put(&m1).unwrap();

        let anchor = derive_narrative_anchor(&store, &["人物:桑安歌".into()]);
        assert_eq!(anchor, 1701302400000000000); // fallback to global max
    }

    #[test]
    fn multidimensional_recall_finds_by_container() {
        let (store, _dir) = setup_store();
        let m1 = make_memory("a", 1701302400000000000, vec!["人物:桑安歌", "场景:邯郸酒肆"], "ch141");
        let m2 = make_memory("b", 1701388800000000000, vec!["人物:桑安歌", "人物:葵儿"], "ch143");
        let m3 = make_memory("c", 1701648000000000000, vec!["人物:柏正则", "场景:骊山"], "ch146-bz");
        store.put(&m1).unwrap();
        store.put(&m2).unwrap();
        store.put(&m3).unwrap();

        let query = Query {
            text: "桑安歌在酒肆".into(),
            context_time: TimeStamp::now(),
            context_space: SpatialCoord {
                containers: vec!["人物:桑安歌".into()],
                ..Default::default()
            },
            body_state: None,
        };

        // 仅激活"地"维度：只按容器过滤，不走时间展开
        let window = recall_multidimensional(&store, &query, 10, 3650, 0.0, &["地".into()]);
        assert_eq!(window.memories.len(), 2); // ch141 + ch143, not ch146-bz

        // 天地合并：时间窗口会包括全部3条（都在 30 天叙事窗口内）
        let window_all = recall_multidimensional(&store, &query, 10, 3650, 0.0, &["天".into(), "地".into()]);
        assert_eq!(window_all.memories.len(), 3); // union of time window (all 3) + container match (2)
    }

    #[test]
    fn empty_containers_recalls_all_in_time_window() {
        let (store, _dir) = setup_store();
        let m1 = make_memory("a", 1701302400000000000, vec!["人物:桑安歌"], "ch141");
        let m2 = make_memory("b", 1701648000000000000, vec!["人物:柏正则"], "ch146");
        store.put(&m1).unwrap();
        store.put(&m2).unwrap();

        let query = Query {
            text: "any".into(),
            context_time: TimeStamp::now(),
            context_space: SpatialCoord::default(),
            body_state: None,
        };

        let window = recall_multidimensional(&store, &query, 10, 3650, 0.0, &[]);
        assert!(window.memories.len() >= 2);
    }
}
