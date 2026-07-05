use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct GrepResult {
    pub vfs_path: String,
    pub entry_kind: String,
    pub line_number: i64,
    pub matched_line: String,
}

pub fn query_grep(
    conn: &Connection,
    pattern: &str,
    path_prefix: Option<&str>,
) -> rusqlite::Result<Vec<GrepResult>> {
    let path_clause = match path_prefix {
        Some(prefix) if !prefix.is_empty() => "AND vfs_path LIKE ?2 || '%' ESCAPE '\\'",
        _ => "",
    };

    let sql = format!(
        r#"SELECT vfs_path, entry_kind, line_start, content
           FROM vfs_entries
           WHERE content IS NOT NULL
             AND content LIKE '%' || ?1 || '%'
             {}
           ORDER BY vfs_path
           LIMIT 200"#,
        path_clause
    );

    let mut stmt = conn.prepare(&sql)?;
    let results = if path_clause.is_empty() {
        stmt.query_map(params![pattern], |row| {
            let content: String = row.get(3)?;
            let line_start: i64 = row.get(2).unwrap_or(0);
            let matched = content
                .lines()
                .enumerate()
                .find(|(_, l)| l.contains(pattern))
                .map(|(i, l)| (i as i64, l.to_string()))
                .unwrap_or((0, content.chars().take(200).collect()));

            Ok(GrepResult {
                vfs_path: row.get(0)?,
                entry_kind: row.get(1)?,
                line_number: line_start + matched.0,
                matched_line: matched.1,
            })
        })?
        .filter_map(|r| r.ok())
        .collect()
    } else {
        stmt.query_map(params![pattern, path_prefix.unwrap_or("")], |row| {
            let content: String = row.get(3)?;
            let line_start: i64 = row.get(2).unwrap_or(0);
            let matched = content
                .lines()
                .enumerate()
                .find(|(_, l)| l.contains(pattern))
                .map(|(i, l)| (i as i64, l.to_string()))
                .unwrap_or((0, content.chars().take(200).collect()));

            Ok(GrepResult {
                vfs_path: row.get(0)?,
                entry_kind: row.get(1)?,
                line_number: line_start + matched.0,
                matched_line: matched.1,
            })
        })?
        .filter_map(|r| r.ok())
        .collect()
    };

    Ok(results)
}
