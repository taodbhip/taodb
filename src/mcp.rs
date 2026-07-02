//! MCP Server — 多维度时空接口
//!
//! memorize: 接受道/天/地/人/物五维锚点
//! recall:   构建 context_space 从 containers，锚点从数据内部推导
//! recent / stats / forget / decay: 不变

use rmcp::{ErrorData as McpError, RoleServer, ServerHandler, ServiceExt, model::*, service::RequestContext};
use serde_json::{Map, Value, json};
use std::path::PathBuf;
use std::sync::Arc;

use crate::model::{BodyState, EmotionalMark, Memory, PotentialField, Query, SpatialCoord, TimeStamp, TopoRel};
use crate::store::Store;

fn to_content(s: impl Into<String>) -> Content {
    Annotated::new(
        RawContent::Text(RawTextContent {
            text: s.into(),
            meta: None,
        }),
        None,
    )
}

fn schema(v: Value) -> Arc<Map<String, Value>> {
    Arc::new(v.as_object().unwrap().clone())
}

#[derive(Clone)]
pub struct TaodbMcp {
    store: Arc<Store>,
    user_id: String,
    project_id: String,
    project_instructions: String,
}

impl TaodbMcp {
    pub fn new(data_dir: PathBuf, user_id: String, project_id: String) -> anyhow::Result<Self> {
        let project_path = data_dir
            .join("users")
            .join(&user_id)
            .join("projects")
            .join(&project_id)
            .join("db");
        std::fs::create_dir_all(&project_path)?;
        let store = Store::open(&project_path)?;

        let project_instructions = Self::load_project_instructions();

        eprintln!(
            "[mcp] taodb ready: user={user_id}, project={project_id}, memories={}",
            store.count()
        );
        Ok(Self {
            store: Arc::new(store),
            user_id,
            project_id,
            project_instructions,
        })
    }

    fn load_project_instructions() -> String {
        let path = std::env::current_dir()
            .unwrap_or_default()
            .join(".taodb")
            .join("instructions.md");
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let trimmed = content.trim();
                if trimmed.is_empty() {
                    String::new()
                } else {
                    eprintln!("[mcp] loaded project instructions from {}", path.display());
                    format!("\n\n── PROJECT INSTRUCTIONS ({}) ──\n\n{}", path.display(), trimmed)
                }
            }
            Err(_) => String::new(),
        }
    }

    fn tools(&self) -> Vec<Tool> {
        vec![
            Tool::new(
                "taodb_stats",
                "Check memory engine state. Call at session START: if count=0, prompt user to import project content. If count>0, read container_distribution to align with existing schema, then proceed to taodb_recent.",
                schema(json!({"type":"object","properties":{}})),
            ),
            Tool::new(
                "taodb_recent",
                "Return N most recent memories by insertion order. Call after taodb_stats to find last position. No time window expansion — pure chronological tail.",
                schema(json!({"type":"object","properties":{"n":{"type":"integer"}}})),
            ),
            Tool::new(
                "taodb_recall",
                "Multi-dimensional spatiotemporal recall. containers=['<spatial tags>'] for spatial filtering, narrative_span_days=5 for near-current context, min_energy=0.3 for permanent knowledge. dimensions=['天','地','人','物','道'] to activate specific axes with weighted scoring. Anchor time derived from latest matching memory — NOT wall clock. Returns per-memory 'why' and 'score' fields.",
                schema(json!({"type":"object","properties":{
                    "query":{"type":"string"},
                    "containers":{"type":"array","items":{"type":"string"}},
                    "narrative_span_days":{"type":"integer"},
                    "min_energy":{"type":"number"},
                    "top_k":{"type":"integer"},
                    "dimensions":{"type":"array","items":{"type":"string"}}
                },"required":["query"]})),
            ),
            Tool::new(
                "taodb_memorize",
                "Store a memory with multi-dimensional anchors. Use AFTER completing work. time_ns is auto-derived from containers matching time patterns. containers for spatial tags. energy_floor=0.7 for permanent knowledge, 0.5 for semi-permanent, 0.0 for ordinary context. Optional dimensional anchors: era, relative_time, body_state (JSON), emotional_mark (JSON), potential_field (JSON), topology (JSON). Returns warnings and suggestions when key fields are missing.",
                schema(json!({"type":"object","properties":{
                    "text":{"type":"string"},
                    "time_ns":{"type":"integer"},
                    "containers":{"type":"array","items":{"type":"string"}},
                    "energy_floor":{"type":"number"},
                    "era":{"type":"string"},
                    "relative_time":{"type":"string"},
                    "body_state":{"type":"string"},
                    "emotional_mark":{"type":"string"},
                    "potential_field":{"type":"string"},
                    "topology":{"type":"string"}
                },"required":["text"]})),
            ),
            Tool::new(
                "taodb_forget",
                "Delete a memory by its ULID. Use to remove incorrect or duplicate memories.",
                schema(json!({"type":"object","properties":{"memory_id":{"type":"string"}},"required":["memory_id"]})),
            ),
            Tool::new(
                "taodb_decay",
                "Trigger potential energy decay across all memories using narrative time anchor. Memories below their energy_floor are protected. Run periodically.",
                schema(json!({"type":"object","properties":{}})),
            ),
            // ── v2: 三层召回 ──
            Tool::new(
                "taodb_recall_constraints",
                "Recall ALL constraint-layer memories (energy_floor>=0.5). Call at session START to load world rules, character perception frameworks, and object chains into LLM context. These memories never decay — they are the world model. Returns flat list sorted by energy_floor DESC.",
                schema(
                    json!({"type":"object","properties":{"min_floor":{"type":"number"},"top_k":{"type":"integer"}}}),
                ),
            ),
            Tool::new(
                "taodb_recall_sensory",
                "Sensory-triggered recall (Proust: involuntary memory). Input sensory impressions like ['涩','凉','颤'] and TaoDB returns ALL memories sharing those senses — regardless of character, scene, or time. Senses are cross-container, cross-temporal index keys. Use when writing encounters a sensory state and needs related memories to surface naturally.",
                schema(
                    json!({"type":"object","properties":{"senses":{"type":"array","items":{"type":"string"}},"top_k":{"type":"integer"},"narrative_span_days":{"type":"integer"}},"required":["senses"]}),
                ),
            ),
        ]
    }

    fn s<'a>(args: &'a Map<String, Value>, key: &str) -> &'a str {
        args.get(key).and_then(|v| v.as_str()).unwrap_or("")
    }
    fn n(args: &Map<String, Value>, key: &str, d: usize) -> usize {
        args.get(key).and_then(|v| v.as_u64()).unwrap_or(d as u64) as usize
    }
    fn n_i64(args: &Map<String, Value>, key: &str, d: i64) -> i64 {
        args.get(key).and_then(|v| v.as_i64()).unwrap_or(d)
    }
    fn n_f32(args: &Map<String, Value>, key: &str, d: f32) -> f32 {
        args.get(key).and_then(|v| v.as_f64()).unwrap_or(d as f64) as f32
    }
    fn str_list(args: &Map<String, Value>, key: &str) -> Vec<String> {
        args.get(key)
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default()
    }

    fn dispatch(&self, name: &str, args: Option<Map<String, Value>>) -> CallToolResult {
        let args = args.unwrap_or_default();
        let ok =
            |v: serde_json::Value| CallToolResult::success(vec![to_content(serde_json::to_string_pretty(&v).unwrap())]);
        let err = |m: &str| CallToolResult::error(vec![to_content(m)]);
        match name {
            "taodb_memorize" => {
                let mut mem = Memory::from_text(Self::s(&args, "text"));
                let mut warnings: Vec<String> = Vec::new();
                let mut suggestions: Vec<String> = Vec::new();

                // ── 天: 时间维度 ──
                if let Some(t) = args.get("time_ns").and_then(|v| v.as_i64()) {
                    mem.time.absolute_ns = t;
                } else {
                    // 尝试从 containers 自动推导 time_ns
                    if let Some(cs) = args.get("containers").and_then(|v| v.as_array()) {
                        let container_strs: Vec<String> =
                            cs.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
                        if let Some(derived_ns) = self.store.derive_time_ns_from_containers(&container_strs) {
                            mem.time.absolute_ns = derived_ns;
                            suggestions.push(format!("time_ns 已从 containers 自动推导: {}", derived_ns));
                        } else {
                            warnings.push("time_ns 未设置，使用墙上时钟时间戳。如有叙事时间请传入 time_ns。".into());
                        }
                    } else {
                        warnings.push("time_ns 未设置，使用墙上时钟时间戳。如有叙事时间请传入 time_ns。".into());
                    }
                }
                if let Some(era) = args.get("era").and_then(|v| v.as_str()) {
                    mem.time.era = era.to_string();
                }
                if let Some(rel) = args.get("relative_time").and_then(|v| v.as_str()) {
                    mem.time.relative = vec![rel.to_string()];
                }

                // ── 地: 空间维度 ──
                if let Some(cs) = args.get("containers").and_then(|v| v.as_array()) {
                    let raw_containers: Vec<String> =
                        cs.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
                    // 模糊匹配 + 规范化已有 container
                    let mut normalized = Vec::new();
                    for c in &raw_containers {
                        if let Some(matched) = self.store.fuzzy_match_container(c) {
                            if &matched != c {
                                suggestions.push(format!("container '{}' 已规范化为 '{}'", c, matched));
                            }
                            normalized.push(matched);
                        } else {
                            normalized.push(c.clone());
                        }
                    }
                    mem.space.containers = normalized;
                }
                if let Some(topo_json) = args.get("topology").and_then(|v| v.as_str())
                    && let Ok(parsed) = serde_json::from_str::<Vec<TopoRel>>(topo_json)
                {
                    mem.space.topology = parsed;
                }

                // ── 人: 身体/情感维度 ──
                if let Some(body_json) = args.get("body_state").and_then(|v| v.as_str()) {
                    if let Ok(parsed) = serde_json::from_str::<BodyState>(body_json) {
                        mem.bodies = vec![parsed];
                    }
                } else if mem.energy_floor >= 0.3 {
                    // 重要记忆未设 body_state 时提示
                    suggestions.push("建议添加 body_state (JSON) 以激活'人'维度召回".into());
                }
                if let Some(emo_json) = args.get("emotional_mark").and_then(|v| v.as_str())
                    && let Ok(parsed) = serde_json::from_str::<EmotionalMark>(emo_json)
                {
                    mem.emotion = vec![parsed];
                }

                // ── 道: 规则/势场维度 ──
                if let Some(pf_json) = args.get("potential_field").and_then(|v| v.as_str())
                    && let Ok(parsed) = serde_json::from_str::<PotentialField>(pf_json)
                {
                    mem.potential = vec![parsed];
                }

                // ── 能量 ──
                if let Some(f) = args.get("energy_floor").and_then(|v| v.as_f64()) {
                    mem.energy_floor = f as f32;
                }

                // ── v2: 记忆类型 (约束层/叙事层) ──
                if let Some(mt) = args.get("memory_type").and_then(|v| v.as_str()) {
                    mem.memory_type = match mt {
                        "constraint" => crate::model::MemoryType::Constraint,
                        _ => crate::model::MemoryType::Narrative,
                    };
                } else if mem.energy_floor >= 0.5 {
                    mem.memory_type = crate::model::MemoryType::Constraint;
                }
                // ── v2: 叙事呈现顺序 (syuzhet) ──
                if let Some(ci) = args.get("chapter_index").and_then(|v| v.as_str()) {
                    mem.chapter_index = ci.to_string();
                }
                // ── v2: 感官锚点 (Proust: 感官触发召回) ──
                // schema: array of string (LLM 传 ["涩","凉"]), 内部自动构造 SenseAnchor
                if let Some(senses_val) = args.get("senses") {
                    if let Some(arr) = senses_val.as_array() {
                        mem.senses = arr
                            .iter()
                            .filter_map(|v| v.as_str())
                            .map(|s| crate::model::SenseAnchor {
                                kind: "未指定".into(),
                                impression: s.to_string(),
                                intensity: 0.5,
                                source: None,
                            })
                            .collect();
                    } else if let Some(s) = senses_val.as_str() {
                        // 兼容旧格式: JSON 字符串 Vec<SenseAnchor>
                        if let Ok(parsed) = serde_json::from_str::<Vec<crate::model::SenseAnchor>>(s) {
                            mem.senses = parsed;
                        }
                    }
                }

                if mem.potential_energy < mem.energy_floor {
                    mem.potential_energy = mem.energy_floor;
                }

                match self.store.put(&mem) {
                    Ok(()) => {
                        let mut resp = json!({
                            "ok": true,
                            "memory_id": mem.id.to_string(),
                            "energy_floor": mem.energy_floor,
                            "time_ns": mem.time.absolute_ns,
                            "containers": mem.space.containers,
                        });
                        if !warnings.is_empty() {
                            resp["warnings"] = json!(warnings);
                        }
                        if !suggestions.is_empty() {
                            resp["suggestions"] = json!(suggestions);
                        }
                        ok(resp)
                    }
                    Err(e) => err(&e.to_string()),
                }
            }
            "taodb_recall" => {
                let query_text = Self::s(&args, "query").to_string();
                let containers = Self::str_list(&args, "containers");
                let narrative_span_days = Self::n_i64(&args, "narrative_span_days", 3650);
                let min_energy = Self::n_f32(&args, "min_energy", 0.0);
                let top_k = Self::n(&args, "top_k", 10);
                let dimensions = Self::str_list(&args, "dimensions");

                // ── 构建 context_space 从 containers（核心修复） ──
                let context_space = SpatialCoord {
                    containers: containers.clone(),
                    ..Default::default()
                };

                // ── context_time 从数据内部推导（不再用墙上时钟） ──
                let anchor_ns = crate::recall::derive_narrative_anchor(&self.store, &containers);
                let context_time = TimeStamp {
                    absolute_ns: anchor_ns,
                    era: "叙事".into(),
                    precision: "回".into(),
                    ..Default::default()
                };

                let query = Query {
                    text: query_text,
                    context_time,
                    context_space,
                    body_state: None,
                };

                let active_dims: Vec<String> = if dimensions.is_empty() {
                    // 默认：有 containers → 天+地+道；无 containers → 天+道
                    if containers.is_empty() {
                        vec!["天".into(), "道".into()]
                    } else {
                        vec!["天".into(), "地".into(), "道".into()]
                    }
                } else {
                    dimensions
                };

                let window = crate::recall::recall_multidimensional(
                    &self.store,
                    &query,
                    top_k,
                    narrative_span_days,
                    min_energy,
                    &active_dims,
                );

                let memories: Vec<Value> = window
                    .memories
                    .iter()
                    .map(|m| {
                        let txt: String = m.events.iter().map(|e| e.what.as_str()).collect::<Vec<_>>().join("; ");
                        // 查找对应的评分明细
                        let score_info = window
                            .scoring_breakdown
                            .iter()
                            .find(|s| s.memory_id == m.id.to_string());
                        json!({
                            "id": m.id.to_string(),
                            "text": txt,
                            "time_ns": m.time.absolute_ns,
                            "era": m.time.era,
                            "containers": m.space.containers,
                            "energy": m.potential_energy,
                            "energy_floor": m.energy_floor,
                            "has_body": !m.bodies.is_empty(),
                            "has_emotion": !m.emotion.is_empty(),
                            "has_potential": !m.potential.is_empty(),
                            "score": score_info.map(|s| s.total_score).unwrap_or(0.0),
                            "why": score_info.map(|s| s.why.clone()).unwrap_or_default(),
                        })
                    })
                    .collect();

                ok(json!({
                    "memories": memories,
                    "count": memories.len(),
                    "anchor_ns": anchor_ns,
                    "dimensions_used": active_dims,
                    "recall_paths": window.recall_paths,
                    "scoring_breakdown": window.scoring_breakdown.iter().map(|s| json!({
                        "memory_id": s.memory_id,
                        "total": s.total_score,
                        "time": s.narrative_proximity,
                        "space": s.container_overlap,
                        "energy": s.energy_score,
                        "body_emotion": s.body_emotion_bonus,
                        "text_match": s.text_match_score,
                        "why": s.why,
                    })).collect::<Vec<Value>>(),
                }))
            }
            "taodb_recent" => {
                let n = Self::n(&args, "n", 10);
                let mems = self.store.indexed_recent(n);
                let items: Vec<Value> = mems.iter().map(|m| {
                    let txt: String = m.events.iter().map(|e| e.what.as_str()).collect::<Vec<_>>().join("; ");
                    json!({"id":m.id.to_string(),"time_ns":m.time.absolute_ns,"containers":m.space.containers,"text":txt,"energy":m.potential_energy})
                }).collect();
                ok(json!({"memories":items,"count":items.len()}))
            }
            "taodb_forget" => {
                let id = Self::s(&args, "memory_id");
                if id.is_empty() {
                    err("memory_id is required")
                } else if self.store.forget(id) {
                    ok(json!({"ok":true,"deleted":id}))
                } else {
                    err(&format!("memory not found: {id}"))
                }
            }
            "taodb_stats" => {
                let count = self.store.count();
                let container_dist: Vec<Value> = self
                    .store
                    .container_distribution()
                    .iter()
                    .map(|c| {
                        json!({
                            "name": c.name,
                            "count": c.count,
                            "latest_time_ns": c.latest_time_ns,
                        })
                    })
                    .collect();
                let time_span = self.store.time_span();
                let time_range = time_span.map(|(min, max)| json!({"min_ns": min, "max_ns": max}));
                let ef_dist: Vec<Value> = self
                    .store
                    .energy_floor_distribution()
                    .iter()
                    .map(|b| {
                        json!({
                            "floor": b.floor,
                            "count": b.count,
                        })
                    })
                    .collect();
                let recent_containers = self.store.recent_containers(10);
                ok(json!({
                    "memory_count": count,
                    "user_id": self.user_id,
                    "project_id": self.project_id,
                    "container_distribution": container_dist,
                    "time_range": time_range,
                    "energy_floor_distribution": ef_dist,
                    "recent_containers": recent_containers,
                }))
            }
            "taodb_decay" => {
                let anchor_ns = crate::recall::derive_narrative_anchor(&self.store, &[]);
                match self.store.decay_all_narrative(anchor_ns) {
                    Ok(()) => ok(json!({"ok":true,"memory_count":self.store.count(),"anchor_ns":anchor_ns})),
                    Err(e) => err(&e.to_string()),
                }
            }
            // ── v2: 三层召回 ──
            "taodb_recall_constraints" => {
                let req = crate::model::ConstraintRecallRequest {
                    min_floor: Self::n_f32(&args, "min_floor", 0.5),
                    top_k: Self::n(&args, "top_k", 50),
                };
                let memories = crate::recall::recall_constraints(&self.store, &req);
                let count = memories.len();
                let subset: Vec<serde_json::Value> = memories
                    .into_iter()
                    .map(|m| {
                        json!({
                            "id": m.id.to_string(),
                            "text": m.events.first().map(|e| e.what.as_str()).unwrap_or(""),
                            "energy_floor": m.energy_floor,
                            "memory_type": m.memory_type,
                            "containers": m.space.containers,
                        })
                    })
                    .collect();
                ok(json!({"count":count,"memories":subset}))
            }
            "taodb_recall_sensory" => {
                let senses: Vec<String> = Self::str_list(&args, "senses");
                if senses.is_empty() {
                    return err("senses required (e.g. [\"涩\",\"凉\",\"颤\"])");
                }
                let req = crate::model::SensoryRecallRequest {
                    senses,
                    top_k: Self::n(&args, "top_k", 10),
                    narrative_span_days: Self::n_i64(&args, "narrative_span_days", 0),
                };
                let memories = crate::recall::recall_sensory(&self.store, &req);
                let count = memories.len();
                let subset: Vec<serde_json::Value> = memories
                    .into_iter()
                    .map(|m| {
                        json!({
                            "id": m.id.to_string(),
                            "text": m.events.first().map(|e| e.what.as_str()).unwrap_or(""),
                            "senses": m.senses.iter().map(|s| s.impression.as_str()).collect::<Vec<_>>(),
                            "containers": m.space.containers,
                        })
                    })
                    .collect();
                ok(json!({"count":count,"memories":subset}))
            }
            _ => err(&format!("unknown tool: {name}")),
        }
    }
}

impl ServerHandler for TaodbMcp {
    fn get_info(&self) -> ServerInfo {
        let caps = ServerCapabilities::builder().enable_tools().build();
        let mut info = InitializeResult::new(caps);
        let mut imp = Implementation::from_build_env();
        imp.name = "taodb".into();
        imp.version = env!("CARGO_PKG_VERSION").into();
        info.server_info = imp;
        let base_instructions = "\
taodb — LLM memory engine. Multi-dimensional spatiotemporal indexing (道/天/地/人/物).\n\
\n\
SESSION STARTUP (every session, before any other tool):\n\
  1. taodb_stats — check memory count AND container_distribution to align with existing schema.\n\
     Read recent_containers to see the project's naming conventions.\n\
  2. If count=0: tell user 'taodb memory is empty. Import project content?'\n\
     → Import priority: permanent rules/specs (energy_floor=0.7) → plans/architecture (0.5) → timeline content (0.0 + time_ns).\n\
  3. If count>0: taodb_recent(n=1) to find last position,\n\
     then taodb_recall(containers=['<current context tags>'], narrative_span_days=5) for recent context,\n\
     then taodb_recall(min_energy=0.3) for permanent knowledge (SEPARATE call).\n\
\n\
BEFORE WORK — two separate recalls:\n\
  taodb_recall(containers=['<spatial tags>'], narrative_span_days=<project_window>, dimensions=['天','地','人'])\n\
  taodb_recall(query='<knowledge topic>', min_energy=0.3, dimensions=['道'])\n\
\n\
AFTER WORK:\n\
  taodb_memorize(text='<key outcome>', containers=['<spatial tags>'], energy_floor=<0.0-0.7>)\n\
  time_ns is AUTO-DERIVED from containers matching time patterns (e.g. '第N回', 'sprint-N', 'day-N').\n\
  For permanent facts (rules, architecture decisions, world-building): energy_floor=0.7.\n\
  For semi-permanent (plans, profiles): energy_floor=0.5.\n\
  For important events: energy_floor=0.3.\n\
  For ordinary context: energy_floor=0.0 (default).\n\
  Add body_state, emotional_mark as JSON to activate '人' dimension for richer recall.\n\
\n\
CONTAINERS: your project's spatial anchors. Use consistent prefixes (e.g. 'feature:', 'module:', '人物:', '场景:').\n\
  The engine fuzzy-matches containers — 'feature:Auth' auto-corrects to 'feature:auth' if that exists.\n\
  Read taodb_stats.recent_containers to stay aligned with the project's naming conventions.\n\
\n\
RECALL returns per-memory 'why' and 'score' fields — read them to understand engine decisions.\n\
MEMORIZE returns 'warnings' and 'suggestions' when key fields are missing — use them to improve.\n\
\n\
DIMENSIONS (activate with dimensions=[...] to weight the scoring):\n\
  道 — permanent knowledge, rules, principles (weights energy)\n\
  天 — temporal proximity (weights time distance)\n\
  地 — spatial/container overlap (weights container matching)\n\
  人 — body_state + emotional_mark (weights bodily/emotional richness)\n\
  物 — object chains, dependencies, artifacts\n\
\n\
ENERGY FLOOR: 0.0=normal | 0.3=important | 0.5=semi-permanent | 0.7=permanent (never fades)\n\
\n\
Your project's .taodb/instructions.md customizes the WHEN and WHAT — follow its domain-specific patterns.";

        info.instructions = Some(format!("{}{}", base_instructions, self.project_instructions));
        info
    }
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        Ok(self.dispatch(&request.name, request.arguments))
    }
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            tools: self.tools(),
            next_cursor: None,
            meta: None,
        })
    }
}

pub async fn run(data_dir: PathBuf) -> anyhow::Result<()> {
    let user_id = std::env::var("TAODB_USER").unwrap_or_else(|_| "default".to_string());
    let project_id = std::env::var("TAODB_PROJECT").unwrap_or_else(|_| "default".to_string());
    let server = TaodbMcp::new(data_dir, user_id, project_id)?;
    let running = server
        .serve(rmcp::transport::io::stdio())
        .await
        .map_err(|e| anyhow::anyhow!("mcp: {e}"))?;
    running.waiting().await.map_err(|e| anyhow::anyhow!("mcp: {e}"))?;
    Ok(())
}
