use crate::db;
use crate::query;
use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    conn: Arc<MutexConnection>,
}

struct MutexConnection(Connection);

impl MutexConnection {
    fn lock(&self) -> &Connection {
        // For simplicity, we use a simple approach — in production, use a connection pool
        // Since SQLite WAL mode allows concurrent reads, this is acceptable for a local tool
        &self.0
    }
}

// We need Mutex for write safety, but for a read-only query server, we can use unsafe
// In production, use r2d2 connection pool
unsafe impl Send for MutexConnection {}
unsafe impl Sync for MutexConnection {}

#[derive(Deserialize)]
struct PathQuery {
    path: String,
    #[serde(default)]
    depth: Option<usize>,
}

#[derive(Deserialize)]
struct RefsQuery {
    path: String,
    #[serde(default = "default_direction")]
    direction: String,
    #[serde(default = "default_filter")]
    filter: String,
}

fn default_direction() -> String {
    "in".to_string()
}
fn default_filter() -> String {
    "ALL".to_string()
}

#[derive(Deserialize)]
struct GlobQuery {
    pattern: String,
    #[serde(default = "default_type")]
    entry_type: String,
}

fn default_type() -> String {
    "ALL".to_string()
}

#[derive(Deserialize)]
struct GrepQuery {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
}

#[derive(Serialize)]
struct StatusResponse {
    file_count: i64,
    asset_count: i64,
    entity_count: i64,
    yaml_obj_count: i64,
    yaml_ref_count: i64,
    vfs_entry_count: i64,
    vfs_edge_count: i64,
}

pub async fn run_server(project: PathBuf, port: u16) -> anyhow::Result<()> {
    let project_root = project.canonicalize()?;
    let db_path = project_root.join(".unityassetdb").join("index.db");

    if !db_path.exists() {
        anyhow::bail!(
            "No index found at {}. Run `unityassetdb index build {}` first.",
            db_path.display(),
            project_root.display()
        );
    }

    let conn = db::open_db(&db_path)?;
    let state = AppState {
        conn: Arc::new(MutexConnection(conn)),
    };

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/status", get(status_handler))
        .route("/api/ls", get(ls_handler))
        .route("/api/refs", get(refs_handler))
        .route("/api/glob", get(glob_handler))
        .route("/api/grep", get(grep_handler))
        .route("/api/read", get(read_handler))
        .route("/api/graph", get(graph_handler))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    println!("🌐 UnityAssetDB Web UI: http://localhost:{}", port);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn index_handler() -> Response {
    let html = include_str!("../web/index.html");
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response()
}

async fn status_handler(
    State(state): State<AppState>,
) -> Result<Json<StatusResponse>, (StatusCode, String)> {
    let conn = state.conn.lock();

    let file_count = conn
        .query_row("SELECT COUNT(*) FROM files WHERE project_id = 1", [], |r| {
            r.get(0)
        })
        .unwrap_or(0);
    let asset_count = conn
        .query_row(
            "SELECT COUNT(*) FROM assets WHERE project_id = 1",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let entity_count = conn
        .query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))
        .unwrap_or(0);
    let yaml_obj_count = conn
        .query_row("SELECT COUNT(*) FROM yaml_objects", [], |r| r.get(0))
        .unwrap_or(0);
    let yaml_ref_count = conn
        .query_row("SELECT COUNT(*) FROM yaml_references", [], |r| r.get(0))
        .unwrap_or(0);
    let vfs_entry_count = conn
        .query_row(
            "SELECT COUNT(*) FROM vfs_entries WHERE project_id = 1",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let vfs_edge_count = conn
        .query_row("SELECT COUNT(*) FROM vfs_edges", [], |r| r.get(0))
        .unwrap_or(0);

    Ok(Json(StatusResponse {
        file_count,
        asset_count,
        entity_count,
        yaml_obj_count,
        yaml_ref_count,
        vfs_entry_count,
        vfs_edge_count,
    }))
}

async fn ls_handler(
    State(state): State<AppState>,
    Query(q): Query<PathQuery>,
) -> Result<Json<Vec<query::LsEntry>>, (StatusCode, String)> {
    let conn = state.conn.lock();
    let depth = q.depth.unwrap_or(2);
    let results = query::query_ls(conn, &q.path, depth)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(results))
}

async fn refs_handler(
    State(state): State<AppState>,
    Query(q): Query<RefsQuery>,
) -> Result<Json<Vec<query::RefResult>>, (StatusCode, String)> {
    let conn = state.conn.lock();
    let dir = match q.direction.as_str() {
        "out" => query::RefDirection::Out,
        _ => query::RefDirection::In,
    };
    let results = query::query_refs(conn, &q.path, dir, &q.filter)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(results))
}

async fn glob_handler(
    State(state): State<AppState>,
    Query(q): Query<GlobQuery>,
) -> Result<Json<Vec<query::GlobResult>>, (StatusCode, String)> {
    let conn = state.conn.lock();
    let results = query::query_glob(conn, &q.pattern, &q.entry_type)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(results))
}

async fn grep_handler(
    State(state): State<AppState>,
    Query(q): Query<GrepQuery>,
) -> Result<Json<Vec<query::GrepResult>>, (StatusCode, String)> {
    let conn = state.conn.lock();
    let results = query::query_grep(conn, &q.pattern, q.path.as_deref())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(results))
}

async fn read_handler(
    State(state): State<AppState>,
    Query(q): Query<PathQuery>,
) -> Result<Json<Option<query::ReadResult>>, (StatusCode, String)> {
    let conn = state.conn.lock();
    let result = query::query_read(conn, &q.path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(result))
}

#[derive(Serialize)]
struct GraphNode {
    id: String,
    label: String,
    group: String,
}

#[derive(Serialize)]
struct GraphEdge {
    from: String,
    to: String,
    label: String,
}

#[derive(Serialize)]
struct GraphResponse {
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
}

async fn graph_handler(
    State(state): State<AppState>,
    Query(q): Query<PathQuery>,
) -> Result<Json<GraphResponse>, (StatusCode, String)> {
    let conn = state.conn.lock();
    let path = &q.path;

    // Get the root entry
    let root_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM vfs_entries WHERE lower(vfs_path) = lower(?1) LIMIT 1",
            rusqlite::params![path],
            |row| row.get(0),
        )
        .ok();

    let root_id = match root_id {
        Some(id) => id,
        None => {
            return Ok(Json(GraphResponse {
                nodes: vec![],
                edges: vec![],
            }))
        }
    };

    // Get incoming and outgoing references
    let in_refs = query::query_refs(conn, path, query::RefDirection::In, "ALL")
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let out_refs = query::query_refs(conn, path, query::RefDirection::Out, "ALL")
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Add root node
    let root_label: String = conn
        .query_row(
            "SELECT display_name FROM vfs_entries WHERE id = ?1",
            rusqlite::params![root_id],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| path.to_string());

    let root_kind: String = conn
        .query_row(
            "SELECT entry_kind FROM vfs_entries WHERE id = ?1",
            rusqlite::params![root_id],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "unknown".to_string());

    nodes.push(GraphNode {
        id: path.to_string(),
        label: root_label,
        group: root_kind,
    });
    seen.insert(path.to_string());

    for r in &in_refs {
        if seen.insert(r.from_path.clone()) {
            nodes.push(GraphNode {
                id: r.from_path.clone(),
                label: r
                    .from_path
                    .rsplit('/')
                    .next()
                    .unwrap_or(&r.from_path)
                    .to_string(),
                group: r.from_kind.clone(),
            });
        }
        edges.push(GraphEdge {
            from: r.from_path.clone(),
            to: path.to_string(),
            label: r.edge_kind.clone(),
        });
    }

    for r in &out_refs {
        if seen.insert(r.to_path.clone()) {
            nodes.push(GraphNode {
                id: r.to_path.clone(),
                label: r
                    .to_path
                    .rsplit('/')
                    .next()
                    .unwrap_or(&r.to_path)
                    .to_string(),
                group: r.to_kind.clone(),
            });
        }
        edges.push(GraphEdge {
            from: path.to_string(),
            to: r.to_path.clone(),
            label: r.edge_kind.clone(),
        });
    }

    Ok(Json(GraphResponse { nodes, edges }))
}
