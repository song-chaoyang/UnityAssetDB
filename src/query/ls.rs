use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct LsEntry {
    pub vfs_path: String,
    pub entry_type: String,
    pub entry_kind: String,
    pub display_name: String,
    pub has_children: bool,
}

pub fn query_ls(conn: &Connection, vfs_path: &str, depth: usize) -> rusqlite::Result<Vec<LsEntry>> {
    let max_depth = depth.min(10) as i64;

    let sql = r#"
        WITH RECURSIVE tree(id, vfs_path, entry_type, entry_kind, display_name, parent_path, depth) AS (
            SELECT id, vfs_path, entry_type, entry_kind, display_name, parent_vfs_path, 0
            FROM vfs_entries
            WHERE parent_vfs_path = ?1
            UNION
            SELECT child.id, child.vfs_path, child.entry_type, child.entry_kind,
                   child.display_name, child.parent_vfs_path, tree.depth + 1
            FROM tree
            JOIN vfs_entries child ON child.parent_vfs_path = tree.vfs_path
            WHERE tree.depth < ?2
        )
        SELECT DISTINCT
            t.vfs_path,
            t.entry_type,
            t.entry_kind,
            t.display_name,
            EXISTS(SELECT 1 FROM vfs_entries gc WHERE gc.parent_vfs_path = t.vfs_path) AS has_children
        FROM tree t
        ORDER BY t.vfs_path
    "#;

    let mut stmt = conn.prepare(sql)?;
    let results = stmt
        .query_map(params![vfs_path, max_depth], |row| {
            Ok(LsEntry {
                vfs_path: row.get(0)?,
                entry_type: row.get(1)?,
                entry_kind: row.get(2)?,
                display_name: row.get(3)?,
                has_children: row.get(4)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(results)
}
