use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ReadResult {
    pub vfs_path: String,
    pub entry_type: String,
    pub entry_kind: String,
    pub content: Option<String>,
    pub meta_content: Option<String>,
    pub line_start: Option<i64>,
    pub line_end: Option<i64>,
    pub target_vfs_path: Option<String>,
}

pub fn query_read(conn: &Connection, vfs_path: &str) -> rusqlite::Result<Option<ReadResult>> {
    let mut stmt = conn.prepare(
        "SELECT vfs_path, entry_type, entry_kind, content, meta_content,
                line_start, line_end, target_vfs_path
         FROM vfs_entries
         WHERE lower(vfs_path) = lower(?1)
         LIMIT 1",
    )?;

    let result = stmt
        .query_map(params![vfs_path], |row| {
            Ok(ReadResult {
                vfs_path: row.get(0)?,
                entry_type: row.get(1)?,
                entry_kind: row.get(2)?,
                content: row.get(3)?,
                meta_content: row.get(4)?,
                line_start: row.get(5)?,
                line_end: row.get(6)?,
                target_vfs_path: row.get(7)?,
            })
        })?
        .filter_map(|r| r.ok())
        .next();

    Ok(result)
}
