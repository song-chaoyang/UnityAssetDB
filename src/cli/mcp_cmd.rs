use crate::db;
use crate::query;
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

// ═══════════════════════════════════════════════════════════════════
//  MCP Protocol Types
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

// ═══════════════════════════════════════════════════════════════════
//  MCP Tool Definitions
// ═══════════════════════════════════════════════════════════════════

fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "index_build",
            "description": "Build a fresh SQLite index for a Unity project. Discovers all assets, extracts YAML objects and references, parses C# scripts, builds entity graph and VFS.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_path": {
                        "type": "string",
                        "description": "Absolute path to the Unity project root directory"
                    }
                },
                "required": ["project_path"]
            }
        }),
        json!({
            "name": "index_status",
            "description": "Show index statistics: file count, asset count, entity count, YAML object count, reference count, VFS entry/edge counts.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_path": {
                        "type": "string",
                        "description": "Absolute path to the Unity project root directory"
                    }
                },
                "required": ["project_path"]
            }
        }),
        json!({
            "name": "refs",
            "description": "Query the reference graph for a VFS path. Find who references an asset (incoming) or what an asset references (outgoing). This is the most powerful tool — use it to trace dependency chains like: which scenes use a material, which prefabs reference a script, what does a scene depend on.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_path": {
                        "type": "string",
                        "description": "Absolute path to the Unity project root directory"
                    },
                    "path": {
                        "type": "string",
                        "description": "VFS path to query (e.g. 'Assets/Materials/Enemy.mat')"
                    },
                    "direction": {
                        "type": "string",
                        "enum": ["in", "out"],
                        "description": "'in' = who references this asset; 'out' = what this asset references. Default: 'in'",
                        "default": "in"
                    },
                    "filter": {
                        "type": "string",
                        "enum": ["File", "Component", "GameObject", "ALL"],
                        "description": "Filter results by entry type. Default: 'ALL'",
                        "default": "ALL"
                    }
                },
                "required": ["project_path", "path"]
            }
        }),
        json!({
            "name": "ls",
            "description": "List VFS entries under a path. Use to browse the asset hierarchy: directories, files, and entity nodes (GameObjects, Components, Materials, etc.).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_path": {
                        "type": "string",
                        "description": "Absolute path to the Unity project root directory"
                    },
                    "path": {
                        "type": "string",
                        "description": "VFS path to list children of (e.g. 'Assets', 'Assets/Scenes')"
                    },
                    "depth": {
                        "type": "integer",
                        "description": "How deep to traverse. Default: 1",
                        "default": 1
                    }
                },
                "required": ["project_path", "path"]
            }
        }),
        json!({
            "name": "glob",
            "description": "Find VFS entries matching a glob pattern. Useful for finding assets by name pattern. Supports * and ? wildcards.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_path": {
                        "type": "string",
                        "description": "Absolute path to the Unity project root directory"
                    },
                    "pattern": {
                        "type": "string",
                        "description": "Glob pattern (e.g. '*.prefab', '*Enemy*', '**/Player*')"
                    },
                    "entry_type": {
                        "type": "string",
                        "enum": ["ALL", "file", "directory", "node"],
                        "description": "Filter by entry type. Default: 'ALL'",
                        "default": "ALL"
                    }
                },
                "required": ["project_path", "pattern"]
            }
        }),
        json!({
            "name": "grep",
            "description": "Search content within indexed VFS entries. Finds entries whose content contains the search pattern.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_path": {
                        "type": "string",
                        "description": "Absolute path to the Unity project root directory"
                    },
                    "pattern": {
                        "type": "string",
                        "description": "Text pattern to search for"
                    },
                    "path_prefix": {
                        "type": "string",
                        "description": "Optional: only search within this VFS path prefix"
                    }
                },
                "required": ["project_path", "pattern"]
            }
        }),
        json!({
            "name": "read",
            "description": "Read the content of a VFS entry. Returns entry metadata and content if available.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_path": {
                        "type": "string",
                        "description": "Absolute path to the Unity project root directory"
                    },
                    "path": {
                        "type": "string",
                        "description": "VFS path to read"
                    }
                },
                "required": ["project_path", "path"]
            }
        }),
    ]
}

// ═══════════════════════════════════════════════════════════════════
//  MCP Server
// ═══════════════════════════════════════════════════════════════════

pub fn run_mcp_server() -> anyhow::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let id = request.id.unwrap_or(Value::Null);

        let response = handle_request(&request.method, request.params, &id);
        let response_str = serde_json::to_string(&response)?;
        writeln!(stdout, "{}", response_str)?;
        stdout.flush()?;
    }

    Ok(())
}

fn handle_request(method: &str, params: Option<Value>, id: &Value) -> Value {
    match method {
        "initialize" => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "unityassetdb",
                    "version": "0.1.0"
                }
            }
        }),

        "tools/list" => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "tools": tool_definitions()
            }
        }),

        "tools/call" => {
            let params = match params {
                Some(p) => p,
                None => return error_response(id, -32602, "Missing params".into()),
            };

            let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");

            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

            match call_tool(tool_name, &arguments) {
                Ok(result) => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [
                            {
                                "type": "text",
                                "text": result
                            }
                        ]
                    }
                }),
                Err(e) => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "isError": true,
                        "content": [
                            {
                                "type": "text",
                                "text": format!("Error: {}", e)
                            }
                        ]
                    }
                }),
            }
        }

        _ => error_response(id, -32601, format!("Unknown method: {}", method)),
    }
}

fn error_response(id: &Value, code: i32, message: String) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

// ═══════════════════════════════════════════════════════════════════
//  Tool Handlers
// ═══════════════════════════════════════════════════════════════════

fn call_tool(name: &str, args: &Value) -> Result<String, String> {
    let project_path = args
        .get("project_path")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: project_path")?;

    let project_root = std::path::Path::new(project_path)
        .canonicalize()
        .map_err(|e| format!("Invalid project path: {}", e))?;

    match name {
        "index_build" => tool_index_build(&project_root),
        "index_status" => tool_index_status(&project_root),
        "refs" => tool_refs(&project_root, args),
        "ls" => tool_ls(&project_root, args),
        "glob" => tool_glob(&project_root, args),
        "grep" => tool_grep(&project_root, args),
        "read" => tool_read(&project_root, args),
        _ => Err(format!("Unknown tool: {}", name)),
    }
}

fn open_db(project_root: &std::path::Path) -> Result<rusqlite::Connection, String> {
    let db_path = project_root.join(".unityassetdb").join("index.db");
    if !db_path.exists() {
        return Err(format!(
            "No index found at {}. Run index_build first.",
            db_path.display()
        ));
    }
    db::open_db(&db_path).map_err(|e| format!("Failed to open database: {}", e))
}

fn tool_index_build(project_root: &std::path::Path) -> Result<String, String> {
    // Delegate to the existing build pipeline
    super::index_cmd::run_build_internal(project_root)
        .map_err(|e| format!("Index build failed: {}", e))?;
    Ok(format!(
        "Index built successfully for project: {}",
        project_root.display()
    ))
}

fn tool_index_status(project_root: &std::path::Path) -> Result<String, String> {
    let conn = open_db(project_root)?;

    let file_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM files WHERE project_id = 1", [], |r| {
            r.get(0)
        })
        .unwrap_or(0);
    let asset_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM assets WHERE project_id = 1",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let entity_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))
        .unwrap_or(0);
    let yaml_obj_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM yaml_objects", [], |r| r.get(0))
        .unwrap_or(0);
    let yaml_ref_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM yaml_references", [], |r| r.get(0))
        .unwrap_or(0);
    let cs_decl_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM cs_declarations", [], |r| r.get(0))
        .unwrap_or(0);
    let vfs_entry_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM vfs_entries WHERE project_id = 1",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let vfs_edge_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM vfs_edges", [], |r| r.get(0))
        .unwrap_or(0);

    let result = json!({
        "project_path": project_root.to_string_lossy(),
        "files": file_count,
        "assets": asset_count,
        "entities": entity_count,
        "yaml_objects": yaml_obj_count,
        "yaml_references": yaml_ref_count,
        "cs_declarations": cs_decl_count,
        "vfs_entries": vfs_entry_count,
        "vfs_edges": vfs_edge_count
    });

    Ok(serde_json::to_string_pretty(&result).unwrap())
}

fn tool_refs(project_root: &std::path::Path, args: &Value) -> Result<String, String> {
    let conn = open_db(project_root)?;
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: path")?;

    let direction_str = args
        .get("direction")
        .and_then(|v| v.as_str())
        .unwrap_or("in");

    let filter = args.get("filter").and_then(|v| v.as_str()).unwrap_or("ALL");

    let direction = match direction_str {
        "out" => query::RefDirection::Out,
        _ => query::RefDirection::In,
    };

    let results = query::query_refs(&conn, path, direction, filter)
        .map_err(|e| format!("Query failed: {}", e))?;

    if results.is_empty() {
        return Ok(format!("No references found for: {}", path));
    }

    let dir_label = match direction {
        query::RefDirection::In => "referenced by",
        query::RefDirection::Out => "references",
    };

    let mut output = format!("{} ({} {})\n", path, results.len(), dir_label);
    output.push_str(&"─".repeat(80));
    output.push('\n');

    for r in &results {
        match direction {
            query::RefDirection::In => {
                output.push_str(&format!(
                    "  ← {} [{}] via {}{}\n",
                    r.from_path,
                    r.from_kind,
                    r.edge_kind,
                    r.edge_subkind
                        .as_ref()
                        .map(|s| format!(" ({})", s))
                        .unwrap_or_default()
                ));
            }
            query::RefDirection::Out => {
                output.push_str(&format!(
                    "  → {} [{}] via {}{}\n",
                    r.to_path,
                    r.to_kind,
                    r.edge_kind,
                    r.edge_subkind
                        .as_ref()
                        .map(|s| format!(" ({})", s))
                        .unwrap_or_default()
                ));
            }
        }
    }

    output.push_str(&format!("\n{} reference(s) total", results.len()));
    Ok(output)
}

fn tool_ls(project_root: &std::path::Path, args: &Value) -> Result<String, String> {
    let conn = open_db(project_root)?;
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: path")?;

    let depth = args
        .get("depth")
        .and_then(|v| v.as_u64())
        .map(|d| d as usize)
        .unwrap_or(1);

    let entries =
        query::query_ls(&conn, path, depth).map_err(|e| format!("Query failed: {}", e))?;

    if entries.is_empty() {
        return Ok(format!("No entries found under: {}", path));
    }

    let mut output = format!("{} ({} entries)\n", path, entries.len());
    output.push_str(&"─".repeat(80));
    output.push('\n');

    for entry in &entries {
        let icon = match entry.entry_type.as_str() {
            "directory" => "📁",
            "file" => "📄",
            "node" => "🔷",
            "link" => "🔗",
            _ => "?",
        };
        let children = if entry.has_children { " +" } else { "" };
        output.push_str(&format!(
            "  {} {} [{}]{}\n",
            icon, entry.display_name, entry.entry_kind, children
        ));
    }

    Ok(output)
}

fn tool_glob(project_root: &std::path::Path, args: &Value) -> Result<String, String> {
    let conn = open_db(project_root)?;
    let pattern = args
        .get("pattern")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: pattern")?;

    let entry_type = args
        .get("entry_type")
        .and_then(|v| v.as_str())
        .unwrap_or("ALL");

    let results = query::query_glob(&conn, pattern, entry_type)
        .map_err(|e| format!("Query failed: {}", e))?;

    if results.is_empty() {
        return Ok(format!("No entries matching: {}", pattern));
    }

    let mut output = format!("{} ({} matches)\n", pattern, results.len());
    output.push_str(&"─".repeat(80));
    output.push('\n');

    for r in &results {
        output.push_str(&format!(
            "  {} [{}] {}\n",
            r.vfs_path, r.entry_type, r.display_name
        ));
    }

    Ok(output)
}

fn tool_grep(project_root: &std::path::Path, args: &Value) -> Result<String, String> {
    let conn = open_db(project_root)?;
    let pattern = args
        .get("pattern")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: pattern")?;

    let path_prefix = args.get("path_prefix").and_then(|v| v.as_str());

    let results = query::query_grep(&conn, pattern, path_prefix)
        .map_err(|e| format!("Query failed: {}", e))?;

    if results.is_empty() {
        return Ok(format!("No matches for: {}", pattern));
    }

    let mut output = format!("{} ({} matches)\n", pattern, results.len());
    output.push_str(&"─".repeat(80));
    output.push('\n');

    for r in &results {
        output.push_str(&format!(
            "  {}:{} [{}]\n    {}\n",
            r.vfs_path,
            r.line_number,
            r.entry_kind,
            r.matched_line.trim()
        ));
    }

    Ok(output)
}

fn tool_read(project_root: &std::path::Path, args: &Value) -> Result<String, String> {
    let conn = open_db(project_root)?;
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: path")?;

    let result = query::query_read(&conn, path).map_err(|e| format!("Query failed: {}", e))?;

    match result {
        Some(r) => {
            let mut output = format!("{} [{} / {}]\n", r.vfs_path, r.entry_type, r.entry_kind);
            output.push_str(&"─".repeat(80));
            output.push('\n');
            if let Some(content) = r.content {
                output.push_str(&content);
            } else {
                output.push_str("(no indexed content)");
            }
            if let Some(target) = r.target_vfs_path {
                output.push_str(&format!("\n\n→ Link target: {}", target));
            }
            Ok(output)
        }
        None => Ok(format!("Not found: {}", path)),
    }
}
