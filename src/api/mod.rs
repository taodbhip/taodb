//! HTTP API
//!
//! POST /v1/memories  — 写入记忆
//! POST /v1/recall    — 召回时空窗口
//! POST /v1/decay     — 衰减
//! GET  /v1/stats     — 统计
//! POST /v1/users     — 创建用户
//! GET  /v1/users     — 列出用户
//! POST /v1/projects  — 创建项目
//! GET  /v1/projects  — 列出项目

use crate::model::{Memory, Query, RecallWindow};
use crate::store::Store;
use crate::tenant::{ProjectConfig, TenantManager, UserConfig};
use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::{delete, get, post},
};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
pub struct ApiState {
    pub tenants: Arc<TenantManager>,
    pub stores: Arc<RwLock<std::collections::HashMap<String, Arc<Store>>>>,
    pub admin_token: String,
}

impl ApiState {
    fn store_key(user_id: &str, project_id: &str) -> String {
        format!("{user_id}/{project_id}")
    }
    fn get_or_open_store(&self, user_id: &str, project_id: &str) -> anyhow::Result<Arc<Store>> {
        let key = Self::store_key(user_id, project_id);
        if let Some(s) = self.stores.read().get(&key) {
            return Ok(s.clone());
        }
        let db_path = self.tenants.project_db_path(user_id, project_id);
        std::fs::create_dir_all(&db_path)?;
        let store = Store::open(&db_path)?;
        let arc = Arc::new(store);
        self.stores.write().insert(key, arc.clone());
        Ok(arc)
    }
}

// ===== Request/Response =====

#[derive(Debug, Deserialize)]
struct IngestRequest {
    text: Option<String>,
    memory: Option<Memory>,
    event_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct IngestResponse {
    memory_id: String,
    ok: bool,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RecallRequest {
    query: String,
    top_k: Option<usize>,
    within_days: Option<i64>,
    min_energy: Option<f32>,
    context_time: Option<crate::model::TimeStamp>,
    context_space: Option<crate::model::SpatialCoord>,
}

#[derive(Debug, Serialize)]
struct StatsResponse {
    memory_count: usize,
    storage_bytes: u64,
}

// ===== Auth =====

fn extract_token(headers: &HeaderMap) -> Option<String> {
    if let Some(v) = headers.get("authorization")
        && let Ok(s) = v.to_str()
        && let Some(stripped) = s.strip_prefix("Bearer ")
    {
        return Some(stripped.to_string());
    }
    headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

fn authenticate(state: &ApiState, headers: &HeaderMap) -> anyhow::Result<UserConfig> {
    let token = extract_token(headers).ok_or_else(|| anyhow::anyhow!("missing api token"))?;
    if token == state.admin_token {
        anyhow::bail!("admin token not allowed for user endpoints");
    }
    state.tenants.find_user_by_token(&token)
}

fn require_user_project(headers: &HeaderMap, state: &ApiState) -> anyhow::Result<(UserConfig, String, String)> {
    let user = authenticate(state, headers)?;
    let project_id = headers
        .get("x-project-id")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            anyhow::anyhow!("missing x-project-id header. Add 'x-project-id' header with your project name.")
        })?;
    // Auto-create project if not found
    if state.tenants.get_project(&user.user_id, project_id).is_err() {
        state.tenants.create_project(&user.user_id, project_id, project_id)?;
    }
    Ok((user.clone(), user.user_id.clone(), project_id.to_string()))
}

fn check_admin(headers: &HeaderMap, state: &ApiState) -> bool {
    extract_token(headers).map(|t| t == state.admin_token).unwrap_or(false)
}

// ===== Router =====

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(handle_health))
        .route("/v1/memories", post(handle_ingest))
        .route("/v1/memories/:memory_id", delete(handle_forget))
        .route("/v1/recall", post(handle_recall))
        .route("/v1/recent", get(handle_recent))
        .route("/v1/decay", post(handle_decay))
        .route("/v1/stats", get(handle_stats))
        .route("/v1/projects", post(handle_create_project).get(handle_list_projects))
        .route("/v1/projects/:project_id", get(handle_get_project))
        .route("/v1/users", post(handle_create_user).get(handle_list_users))
        // v2 三层召回
        .route("/v1/recall/constraints", post(handle_recall_constraints))
        .route("/v1/recall/sensory", post(handle_recall_sensory))
        .route("/v1/recall/narrative", post(handle_recall_narrative))
        .with_state(state)
}

// ===== Handlers =====

async fn handle_health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status":"ok","service":"taodb","version":env!("CARGO_PKG_VERSION")}))
}

async fn handle_ingest(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(req): Json<IngestRequest>,
) -> (StatusCode, Json<IngestResponse>) {
    let resp = (|| -> anyhow::Result<IngestResponse> {
        let (_, uid, pid) = require_user_project(&headers, &state)?;
        let store = state.get_or_open_store(&uid, &pid)?;
        let memory = if let Some(mut mem) = req.memory {
            if let Some(eid) = req.event_id {
                match ulid::Ulid::from_string(&eid) {
                    Ok(u) => mem.id = u,
                    Err(_) => anyhow::bail!(
                        "invalid event_id '{}': ULID must be 26 chars from Crockford Base32 (0-9, A-H, J-N, P-T, V-Z), e.g. 01ARZ3NDEKTSV4RRFFQ69G5FAV",
                        eid
                    ),
                }
            }
            mem
        } else if let Some(text) = req.text {
            Memory::from_text(&text)
        } else {
            anyhow::bail!("either memory or text is required");
        };
        let id = memory.id.to_string();
        store.put(&memory)?;
        Ok(IngestResponse {
            memory_id: id,
            ok: true,
            error: None,
        })
    })();
    match resp {
        Ok(r) => (StatusCode::OK, Json(r)),
        Err(e) => (
            StatusCode::OK,
            Json(IngestResponse {
                memory_id: String::new(),
                ok: false,
                error: Some(e.to_string()),
            }),
        ),
    }
}

async fn handle_recall(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(req): Json<RecallRequest>,
) -> (StatusCode, Json<RecallWindow>) {
    let resp = (|| -> anyhow::Result<RecallWindow> {
        let (_, uid, pid) = require_user_project(&headers, &state)?;
        let store = state.get_or_open_store(&uid, &pid)?;
        let query = Query {
            text: req.query,
            context_time: req.context_time.unwrap_or_else(crate::model::TimeStamp::now),
            context_space: req.context_space.unwrap_or_default(),
            body_state: None,
        };
        let days = req.within_days.unwrap_or(30);
        let energy = req.min_energy.unwrap_or(0.0);
        Ok(crate::recall::recall_window_with_options(
            &store,
            &query,
            req.top_k.unwrap_or(5),
            days,
            energy,
        ))
    })();
    match resp {
        Ok(w) => (StatusCode::OK, Json(w)),
        Err(_) => (StatusCode::OK, Json(RecallWindow::empty())),
    }
}

#[derive(Debug, Deserialize)]
struct RecentQuery {
    n: Option<usize>,
}

async fn handle_recent(
    State(state): State<ApiState>,
    headers: HeaderMap,
    axum::extract::Query(q): axum::extract::Query<RecentQuery>,
) -> (StatusCode, Json<serde_json::Value>) {
    let result = (|| -> anyhow::Result<Vec<serde_json::Value>> {
        let (_, uid, pid) = require_user_project(&headers, &state)?;
        let store = state.get_or_open_store(&uid, &pid)?;
        let mems = store.indexed_recent(q.n.unwrap_or(10));
        Ok(mems.iter().map(|m| {
            let text: String = m.events.iter().map(|e| e.what.as_str()).collect::<Vec<_>>().join("; ");
            serde_json::json!({"id":m.id.to_string(),"time_ns":m.time.absolute_ns,"space":m.space.containers,"text":text})
        }).collect())
    })();
    match result {
        Ok(mems) => (
            StatusCode::OK,
            Json(serde_json::json!({"memories":mems,"count":mems.len()})),
        ),
        Err(e) => (StatusCode::OK, Json(serde_json::json!({"error":e.to_string()}))),
    }
}

async fn handle_decay(State(state): State<ApiState>, headers: HeaderMap) -> (StatusCode, Json<serde_json::Value>) {
    let result = (|| -> anyhow::Result<usize> {
        let (_, uid, pid) = require_user_project(&headers, &state)?;
        let store = state.get_or_open_store(&uid, &pid)?;
        store.decay_all()?;
        Ok(store.count())
    })();
    match result {
        Ok(n) => (StatusCode::OK, Json(serde_json::json!({"ok":true,"memory_count":n}))),
        Err(e) => (
            StatusCode::OK,
            Json(serde_json::json!({"ok":false,"error":e.to_string()})),
        ),
    }
}

async fn handle_stats(State(state): State<ApiState>, headers: HeaderMap) -> (StatusCode, Json<StatsResponse>) {
    let result = (|| -> anyhow::Result<StatsResponse> {
        let (_, uid, pid) = require_user_project(&headers, &state)?;
        let store = state.get_or_open_store(&uid, &pid)?;
        let db_path = state.tenants.project_db_path(&uid, &pid);
        let storage_bytes = dir_size(&db_path).unwrap_or(0);
        Ok(StatsResponse {
            memory_count: store.count(),
            storage_bytes,
        })
    })();
    match result {
        Ok(r) => (StatusCode::OK, Json(r)),
        Err(_) => (
            StatusCode::OK,
            Json(StatsResponse {
                memory_count: 0,
                storage_bytes: 0,
            }),
        ),
    }
}

// ── v2 三层召回 HTTP handlers ──

#[derive(Debug, Deserialize)]
struct ConstraintRecallHttpRequest {
    min_floor: Option<f32>,
    top_k: Option<usize>,
}

async fn handle_recall_constraints(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(req): Json<ConstraintRecallHttpRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let result = (|| -> anyhow::Result<serde_json::Value> {
        let (_, uid, pid) = require_user_project(&headers, &state)?;
        let store = state.get_or_open_store(&uid, &pid)?;
        let creq = crate::model::ConstraintRecallRequest {
            min_floor: req.min_floor.unwrap_or(0.5),
            top_k: req.top_k.unwrap_or(50),
        };
        let memories = crate::recall::recall_constraints(&store, &creq);
        let count = memories.len();
        let subset: Vec<serde_json::Value> = memories
            .into_iter()
            .map(|m| {
                serde_json::json!({
                    "id": m.id.to_string(),
                    "text": m.events.first().map(|e| e.what.as_str()).unwrap_or(""),
                    "energy_floor": m.energy_floor,
                    "memory_type": m.memory_type,
                    "containers": m.space.containers,
                })
            })
            .collect();
        Ok(serde_json::json!({"count": count, "memories": subset}))
    })();
    match result {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (
            StatusCode::OK,
            Json(serde_json::json!({"count":0,"memories":[],"error":e.to_string()})),
        ),
    }
}

#[derive(Debug, Deserialize)]
struct SensoryRecallHttpRequest {
    senses: Vec<String>,
    top_k: Option<usize>,
    narrative_span_days: Option<i64>,
}

async fn handle_recall_sensory(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(req): Json<SensoryRecallHttpRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let result = (|| -> anyhow::Result<serde_json::Value> {
        let (_, uid, pid) = require_user_project(&headers, &state)?;
        let store = state.get_or_open_store(&uid, &pid)?;
        let sreq = crate::model::SensoryRecallRequest {
            senses: req.senses,
            top_k: req.top_k.unwrap_or(10),
            narrative_span_days: req.narrative_span_days.unwrap_or(0),
        };
        let memories = crate::recall::recall_sensory(&store, &sreq);
        let count = memories.len();
        let subset: Vec<serde_json::Value> = memories
            .into_iter()
            .map(|m| {
                serde_json::json!({
                    "id": m.id.to_string(),
                    "text": m.events.first().map(|e| e.what.as_str()).unwrap_or(""),
                    "senses": m.senses.iter().map(|s| s.impression.as_str()).collect::<Vec<_>>(),
                    "containers": m.space.containers,
                })
            })
            .collect();
        Ok(serde_json::json!({"count": count, "memories": subset}))
    })();
    match result {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (
            StatusCode::OK,
            Json(serde_json::json!({"count":0,"memories":[],"error":e.to_string()})),
        ),
    }
}

#[derive(Debug, Deserialize)]
struct NarrativeRecallHttpRequest {
    persons: Option<Vec<String>>,
    locations: Option<Vec<String>>,
    objects: Option<Vec<String>>,
    narrative_span_days: Option<i64>,
    chapter_span: Option<usize>,
    top_k: Option<usize>,
    dimensions: Option<Vec<String>>,
}

async fn handle_recall_narrative(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(req): Json<NarrativeRecallHttpRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let result = (|| -> anyhow::Result<serde_json::Value> {
        let (_, uid, pid) = require_user_project(&headers, &state)?;
        let store = state.get_or_open_store(&uid, &pid)?;
        let nreq = crate::model::NarrativeRecallRequest {
            persons: req.persons.unwrap_or_default(),
            locations: req.locations.unwrap_or_default(),
            objects: req.objects.unwrap_or_default(),
            narrative_span_days: req.narrative_span_days.unwrap_or(30),
            chapter_span: req.chapter_span.unwrap_or(0),
            top_k: req.top_k.unwrap_or(10),
            dimensions: req.dimensions.unwrap_or_default(),
        };
        let window = crate::recall::recall_narrative(&store, &nreq);
        let subset: Vec<serde_json::Value> = window
            .memories
            .into_iter()
            .map(|m| {
                serde_json::json!({
                    "id": m.id.to_string(),
                    "text": m.events.first().map(|e| e.what.as_str()).unwrap_or(""),
                    "containers": m.space.containers,
                    "senses": m.senses.iter().map(|s| s.impression.as_str()).collect::<Vec<_>>(),
                })
            })
            .collect();
        Ok(serde_json::json!({"count": subset.len(), "memories": subset, "recall_paths": window.recall_paths}))
    })();
    match result {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (
            StatusCode::OK,
            Json(serde_json::json!({"count":0,"memories":[],"error":e.to_string()})),
        ),
    }
}

async fn handle_forget(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(memory_id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let result = (|| -> anyhow::Result<bool> {
        let (_, uid, pid) = require_user_project(&headers, &state)?;
        let store = state.get_or_open_store(&uid, &pid)?;
        Ok(store.forget(&memory_id))
    })();
    match result {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({"ok":true,"deleted":memory_id}))),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"ok":false,"error":"memory not found"})),
        ),
        Err(e) => (
            StatusCode::OK,
            Json(serde_json::json!({"ok":false,"error":e.to_string()})),
        ),
    }
}

fn dir_size(path: &std::path::Path) -> std::io::Result<u64> {
    let mut total = 0;
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let m = entry.metadata()?;
            if m.is_dir() {
                total += dir_size(&entry.path())?;
            } else {
                total += m.len();
            }
        }
    }
    Ok(total)
}

#[derive(Debug, Deserialize)]
struct CreateUserRequest {
    user_id: String,
    email: String,
}
#[derive(Debug, Deserialize)]
struct CreateProjectRequest {
    project_id: String,
    name: String,
}

async fn handle_create_user(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(req): Json<CreateUserRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    if !check_admin(&headers, &state) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error":"admin only"})),
        );
    }
    match state.tenants.create_user(&req.user_id, &req.email) {
        Ok(u) => (StatusCode::CREATED, Json(serde_json::to_value(&u).unwrap())),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":e.to_string()})),
        ),
    }
}

async fn handle_list_users(State(state): State<ApiState>, headers: HeaderMap) -> (StatusCode, Json<serde_json::Value>) {
    if !check_admin(&headers, &state) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error":"admin only"})),
        );
    }
    match state.tenants.list_users() {
        Ok(users) => {
            let n = users.len();
            (StatusCode::OK, Json(serde_json::json!({"users":users,"count":n})))
        }
        Err(e) => (StatusCode::OK, Json(serde_json::json!({"error":e.to_string()}))),
    }
}

async fn handle_create_project(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(req): Json<CreateProjectRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let result = (|| -> anyhow::Result<ProjectConfig> {
        let user = authenticate(&state, &headers)?;
        state.tenants.create_project(&user.user_id, &req.project_id, &req.name)
    })();
    match result {
        Ok(p) => (StatusCode::CREATED, Json(serde_json::to_value(&p).unwrap())),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":e.to_string()})),
        ),
    }
}

async fn handle_list_projects(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> (StatusCode, Json<serde_json::Value>) {
    let result = (|| -> anyhow::Result<Vec<ProjectConfig>> {
        let user = authenticate(&state, &headers)?;
        state.tenants.list_projects(&user.user_id)
    })();
    match result {
        Ok(ps) => {
            let n = ps.len();
            (StatusCode::OK, Json(serde_json::json!({"projects":ps,"count":n})))
        }
        Err(e) => (StatusCode::OK, Json(serde_json::json!({"error":e.to_string()}))),
    }
}

async fn handle_get_project(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let result = (|| -> anyhow::Result<ProjectConfig> {
        let user = authenticate(&state, &headers)?;
        state.tenants.get_project(&user.user_id, &project_id)
    })();
    match result {
        Ok(p) => (StatusCode::OK, Json(serde_json::to_value(&p).unwrap())),
        Err(e) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":e.to_string()}))),
    }
}

pub async fn run(addr: &str, data_dir: std::path::PathBuf, admin_token: String) -> anyhow::Result<()> {
    let tenants = Arc::new(TenantManager::new(&data_dir));
    std::fs::create_dir_all(&data_dir)?;
    let state = ApiState {
        tenants,
        stores: Arc::new(RwLock::new(std::collections::HashMap::new())),
        admin_token,
    };
    let app = router(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    eprintln!("taodb listening on {addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
