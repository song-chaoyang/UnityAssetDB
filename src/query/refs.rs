use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct RefResult {
    pub from_path: String,
    pub from_kind: String,
    pub from_type: String,
    pub to_path: String,
    pub to_kind: String,
    pub to_type: String,
    pub edge_kind: String,
    pub edge_subkind: Option<String>,
}

#[derive(Clone, Copy)]
pub enum RefDirection {
    In,
    Out,
}

pub fn query_refs(
    conn: &Connection,
    vfs_path: &str,
    direction: RefDirection,
    filter: &str,
) -> rusqlite::Result<Vec<RefResult>> {
    let edge_kinds = "('calls', 'binds_to', 'depends_on', 'instance_of', 'refs')";

    let (direction_clause, join_col) = match direction {
        RefDirection::In => (
            "edge.to_entry_id IN (SELECT entry_id FROM scope)".to_string(),
            "edge.from_entry_id",
        ),
        RefDirection::Out => (
            "edge.from_entry_id IN (SELECT entry_id FROM scope)".to_string(),
            "edge.to_entry_id",
        ),
    };

    let filter_clause = match filter.to_lowercase().as_str() {
        "file" => "AND target.entry_type = 'file'",
        "component" => "AND target.entry_kind = 'component'",
        "gameobject" => "AND target.entry_kind = 'gameobject'",
        _ => "",
    };

    let sql = format!(
        r#"
        WITH RECURSIVE scope(entry_id, depth) AS (
            SELECT id, 0
            FROM vfs_entries
            WHERE lower(vfs_path) = lower(?1)
            UNION
            SELECT child.id, scope.depth + 1
            FROM scope
            JOIN vfs_edges se ON se.to_entry_id = scope.entry_id
               AND se.edge_kind IN ('child_of', 'defined_in')
            JOIN vfs_entries child ON child.id = se.from_entry_id
            WHERE scope.depth < 5
        )
        SELECT DISTINCT
            from_entry.vfs_path,
            from_entry.entry_kind,
            from_entry.entry_type,
            to_entry.vfs_path,
            to_entry.entry_kind,
            to_entry.entry_type,
            edge.edge_kind,
            edge.edge_subkind
        FROM vfs_edges edge
        JOIN vfs_entries from_entry ON from_entry.id = edge.from_entry_id
        JOIN vfs_entries to_entry ON to_entry.id = edge.to_entry_id
        JOIN vfs_entries target ON target.id = {join_col}
        WHERE edge.edge_kind IN {edge_kinds}
          AND {direction_clause}
          {filter_clause}
        ORDER BY from_entry.vfs_path, to_entry.vfs_path
        "#,
        join_col = join_col,
        edge_kinds = edge_kinds,
        direction_clause = direction_clause,
        filter_clause = filter_clause,
    );

    let mut stmt = conn.prepare(&sql)?;
    let results = stmt
        .query_map(params![vfs_path], |row| {
            Ok(RefResult {
                from_path: row.get(0)?,
                from_kind: row.get(1)?,
                from_type: row.get(2)?,
                to_path: row.get(3)?,
                to_kind: row.get(4)?,
                to_type: row.get(5)?,
                edge_kind: row.get(6)?,
                edge_subkind: row.get(7)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(results)
}
