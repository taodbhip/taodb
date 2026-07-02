//! taodb 道家版核心数据模型
//!
//! 5 层结构（道/天/地/人/物）
//! 道生天，天生地，地生人，人生物。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: ulid::Ulid,
    pub time: TimeStamp,
    pub space: SpatialCoord,
    pub events: Vec<Event>,
    pub bodies: Vec<BodyState>,
    pub emotion: Vec<EmotionalMark>,
    pub potential: Vec<PotentialField>,
    pub potential_energy: f32,
    /// 永久记忆下限 (0.0=正常衰减, 0.5=永久不会降到0.5以下)
    pub energy_floor: f32,
    /// 记忆类型: Constraint(约束/规则) | Narrative(叙事事件)
    #[serde(default)]
    pub memory_type: MemoryType,
    /// 叙事呈现顺序索引 (syuzhet): "卷二/第152回" (仅 Narrative)
    #[serde(default)]
    pub chapter_index: String,
    /// 从叙事中提取的感官锚点 (Proust: 感官触发召回)
    #[serde(default)]
    pub senses: Vec<SenseAnchor>,
}

/// 记忆类型：约束层 vs 叙事层 (Shadow-Loom fabula/syuzhet 分离)
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub enum MemoryType {
    /// 约束层: 世界规则、角色感知框架、物件链 —— 永不衰减，永远在上下文
    #[serde(rename = "constraint")]
    Constraint,
    /// 叙事层: 章回事件、感官描写 —— 随时间/叙事距离衰减
    #[serde(rename = "narrative")]
    #[default]
    Narrative,
}

/// 感官锚点 (Proust 无意记忆: 感官触发比主动检索更强)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SenseAnchor {
    /// 感官类型: 触觉/视觉/听觉/嗅觉/味觉/温度/动作
    pub kind: String,
    /// 具体感觉: 涩/凉/重/颤/跳/酸/紧/滑/糙
    pub impression: String,
    /// 强度 0.0-1.0
    pub intensity: f32,
    /// 感觉来源的物件或身体部位
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TimeStamp {
    pub absolute_ns: i64,
    pub era: String,
    pub relative: Vec<String>,
    pub cycle: Option<String>,
    pub subjective: Option<String>,
    pub precision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SpatialCoord {
    pub containers: Vec<String>,
    pub topology: Vec<TopoRel>,
    pub shape: Option<String>,
    pub orientation: Option<String>,
    pub geo: Option<(f64, f64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopoRel {
    pub direction: String,
    pub target: String,
    pub distance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub time_offset_ns: i64,
    pub what: String,
    pub who: Option<String>,
    pub to: Option<String>,
    pub with: Option<String>,
    pub senses: Vec<SenseFrame>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SenseFrame {
    pub kind: String,
    pub impression: String,
    pub intensity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodyState {
    pub time_offset_ns: i64,
    pub posture: Option<String>,
    pub sensations: Vec<String>,
    pub actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmotionalMark {
    pub time_offset_ns: i64,
    pub label: String,
    pub intensity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PotentialField {
    pub kind: String,
    pub direction: [f32; 3],
    pub strength: f32,
    pub scope: String,
}

/// 衰减计算：使用叙事锚点时间而非墙上时钟
///
/// `anchor_ns` — 叙事上下文的"现在"（最新匹配记忆的时间）
/// `time_ns` — 被评估记忆的时间
/// 半衰期 30 叙事日，衰减到 energy_floor 为止
pub fn potential_energy_narrative(anchor_ns: i64, time_ns: i64, intensity: f32, association: f32) -> f32 {
    let dt_seconds = (anchor_ns - time_ns).abs() as f64 / 1e9;
    let half_life = 30.0 * 24.0 * 3600.0; // 30 叙事日半衰
    let decay = 1.0 / (1.0 + dt_seconds / half_life);
    intensity * association * decay as f32
}

/// 旧版墙上时钟衰减（保留兼容）
pub fn potential_energy(time_ns: i64, intensity: f32, association: f32) -> f32 {
    let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    potential_energy_narrative(now, time_ns, intensity, association)
}

/// 以叙事锚点执行衰减
pub fn decay_narrative(memory: &mut Memory, anchor_ns: i64) {
    memory.potential_energy = potential_energy_narrative(
        anchor_ns,
        memory.time.absolute_ns,
        memory.emotion.iter().map(|e| e.intensity).fold(0.0_f32, f32::max),
        1.0,
    );
    // floor 保护
    if memory.potential_energy < memory.energy_floor {
        memory.potential_energy = memory.energy_floor;
    }
}

pub fn decay(memory: &mut Memory) {
    memory.potential_energy = potential_energy(
        memory.time.absolute_ns,
        memory.emotion.iter().map(|e| e.intensity).fold(0.0_f32, f32::max),
        1.0,
    );
}

/// 召回请求 — LLM 驱动（旧版兼容，内部路由到三层召回）
///
/// `containers` — 空间过滤锚点 (人物:桑安歌, 场景:邯郸酒肆, …)
/// `narrative_span_days` — 沿叙事时间轴前后展开的天数
/// `dimensions` — 激活的展开维度 (天/地/人/物/道)，空 = 全部
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallRequest {
    pub query_text: String,       // LLM 意图描述（用于返回后的语义理解）
    pub containers: Vec<String>,  // 空间过滤
    pub narrative_span_days: i64, // 叙事时间窗口（天）
    pub min_energy: f32,          // 高能记忆阈值
    pub top_k: usize,             // 返回上限
    pub dimensions: Vec<String>,  // 激活维度: "天"/"地"/"人"/"物"/"道"
}

impl Default for RecallRequest {
    fn default() -> Self {
        Self {
            query_text: String::new(),
            containers: vec![],
            narrative_span_days: 30,
            min_energy: 0.0,
            top_k: 10,
            dimensions: vec![],
        }
    }
}

/// 约束层召回 — 返回所有 energy_floor >= min_floor 的记忆
/// 用于会话启动时自动注入 LLM 系统提示词 (Shadow-Loom: WorldModel constraints)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintRecallRequest {
    pub min_floor: f32, // 能量下限，默认 0.5
    pub top_k: usize,   // 返回上限
}

impl Default for ConstraintRecallRequest {
    fn default() -> Self {
        Self {
            min_floor: 0.5,
            top_k: 50,
        }
    }
}

/// 感官触发召回 — 用感官锚点触发共享记忆 (Proust: involuntary memory)
/// 不依赖关键词，不依赖人物/场景标签
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensoryRecallRequest {
    pub senses: Vec<String>,      // 感官锚点: ["涩","凉","颤","跳"]
    pub top_k: usize,             // 返回上限
    pub narrative_span_days: i64, // 时间窗 (0 = 不限)
}

impl Default for SensoryRecallRequest {
    fn default() -> Self {
        Self {
            senses: vec![],
            top_k: 10,
            narrative_span_days: 0,
        }
    }
}

/// 叙事时空召回 — 多维并行 (天/地/人/物)
/// 不含约束层记忆 (约束层由 recall_constraints 单独返回)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeRecallRequest {
    pub persons: Vec<String>,     // 人物过滤
    pub locations: Vec<String>,   // 空间过滤
    pub objects: Vec<String>,     // 物件链
    pub narrative_span_days: i64, // fabula 时间窗
    pub chapter_span: usize,      // syuzhet 叙事窗 (前N回, 0=不限)
    pub top_k: usize,             // 返回上限
    pub dimensions: Vec<String>,  // 激活维度: "天"/"地"/"人"/"物"
}

impl Default for NarrativeRecallRequest {
    fn default() -> Self {
        Self {
            persons: vec![],
            locations: vec![],
            objects: vec![],
            narrative_span_days: 30,
            chapter_span: 0,
            top_k: 10,
            dimensions: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Query {
    pub text: String,
    pub context_time: TimeStamp,
    pub context_space: SpatialCoord,
    pub body_state: Option<BodyState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallWindow {
    pub memories: Vec<Memory>,
    pub time_range: Option<(TimeStamp, TimeStamp)>,
    pub space_scope: Option<Vec<String>>,
    pub field_density: f32,
    pub emergent_associations: Vec<String>,
    /// 召回路径说明（LLM 可视化）
    pub recall_paths: Vec<String>,
    /// 每条记忆的评分明细（LLM 理解为何返回这些）
    pub scoring_breakdown: Vec<MemoryScore>,
}

/// 单条记忆的多维评分明细（LLM 可视化召回原因）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryScore {
    pub memory_id: String,
    pub total_score: f32,
    pub narrative_proximity: f32, // 天: 时间距离分
    pub container_overlap: f32,   // 地: 空间重合分
    pub energy_score: f32,        // 道: 能量分
    pub body_emotion_bonus: f32,  // 人: 身体/情感丰富度
    pub text_match_score: f32,    // query 文本匹配分
    pub why: String,              // LLM 可读的召回原因
}

/// 容器统计（LLM schema 感知）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerStats {
    pub name: String,
    pub count: usize,
    pub latest_time_ns: i64,
}

/// 扩展统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendedStats {
    pub memory_count: usize,
    pub user_id: String,
    pub project_id: String,
    pub container_distribution: Vec<ContainerStats>,
    pub time_range_ns: Option<(i64, i64)>,
    pub energy_floor_distribution: Vec<EnergyFloorBucket>,
    pub recent_containers: Vec<String>, // 最近 10 条记忆使用的 containers
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyFloorBucket {
    pub floor: f32,
    pub count: usize,
}

impl Memory {
    /// 从一段纯文本快速构造 Memory（用于离线 / 不调 LLM 的场景）
    pub fn from_text(text: &str) -> Self {
        let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        Self {
            id: ulid::Ulid::new(),
            time: TimeStamp {
                absolute_ns: now,
                era: "present".into(),
                relative: vec![],
                cycle: None,
                subjective: None,
                precision: "second".into(),
            },
            space: SpatialCoord::default(),
            events: vec![Event {
                time_offset_ns: 0,
                what: text.to_string(),
                who: None,
                to: None,
                with: None,
                senses: vec![],
            }],
            bodies: vec![],
            emotion: vec![],
            potential: vec![],
            potential_energy: 0.0,
            energy_floor: 0.0,
            memory_type: MemoryType::Narrative,
            chapter_index: String::new(),
            senses: vec![],
        }
    }

    /// 是否为约束层记忆
    pub fn is_constraint(&self) -> bool {
        self.memory_type == MemoryType::Constraint || self.energy_floor >= 0.5
    }
}

impl TimeStamp {
    /// 当前墙上时间
    pub fn now() -> Self {
        let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        Self {
            absolute_ns: now,
            era: "现代".into(),
            relative: vec![],
            cycle: None,
            subjective: None,
            precision: "秒".into(),
        }
    }
}

impl RecallWindow {
    pub fn empty() -> Self {
        Self {
            memories: vec![],
            time_range: None,
            space_scope: None,
            field_density: 0.0,
            emergent_associations: vec![],
            recall_paths: vec![],
            scoring_breakdown: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn potential_energy_decay_test() {
        let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let e1 = potential_energy(now, 1.0, 1.0);
        let e2 = potential_energy(now - 30_i64 * 24 * 3600 * 1_000_000_000, 1.0, 1.0);
        let e3 = potential_energy(now - 365_i64 * 24 * 3600 * 1_000_000_000, 1.0, 1.0);
        assert!(e1 > e2);
        assert!(e2 > e3);
    }

    #[test]
    fn narrative_decay_uses_anchor() {
        let anchor = 1701302400000000000;
        // 同一叙事时刻的记忆应该能量最高
        let e_same = potential_energy_narrative(anchor, anchor, 1.0, 1.0);
        // 5 叙事日前的记忆
        let e_5d = potential_energy_narrative(anchor, anchor - 5 * 86400 * 1_000_000_000_i64, 1.0, 1.0);
        assert!(e_same > e_5d);
    }

    #[test]
    fn floor_protection() {
        let anchor = 1701302400000000000;
        // 很远的时间，但 floor=0.5 保护
        let far = anchor - 365 * 86400 * 1_000_000_000_i64;
        let e = potential_energy_narrative(anchor, far, 1.0, 1.0);
        assert!(e < 0.5); // 低于 floor
    }

    #[test]
    fn memory_construction() {
        let mem = Memory {
            id: ulid::Ulid::new(),
            time: TimeStamp {
                absolute_ns: -8_000_000_000_000_000_000,
                era: "present".into(),
                relative: vec!["3 岁那年".into()],
                cycle: None,
                subjective: Some("遥远的童年".into()),
                precision: "年".into(),
            },
            space: SpatialCoord {
                containers: vec!["厨房".into(), "家".into()],
                topology: vec![],
                shape: Some("长方形".into()),
                orientation: None,
                geo: None,
            },
            events: vec![Event {
                time_offset_ns: 0,
                what: "妈妈把苹果递给我".into(),
                who: Some("妈妈".into()),
                to: Some("我".into()),
                with: Some("苹果".into()),
                senses: vec![],
            }],
            bodies: vec![],
            emotion: vec![EmotionalMark {
                time_offset_ns: 0,
                label: "温暖".into(),
                intensity: 0.9,
            }],
            potential: vec![],
            potential_energy: 1.0,
            energy_floor: 0.0,
            memory_type: MemoryType::Narrative,
            chapter_index: String::new(),
            senses: vec![],
        };
        assert_eq!(mem.events.len(), 1);
    }
}
