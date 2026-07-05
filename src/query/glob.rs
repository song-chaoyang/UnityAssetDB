use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct GlobResult {
    pub vfs_path: String,
    pub entry_type: String,
    pub entry_kind: String,
    pub display_name: String,
}

pub fn query_glob(
    conn: &Connection,
    pattern: &str,
    entry_type_filter: &str,
) -> rusqlite::Result<Vec<GlobResult>> {
    // Convert glob pattern to SQL LIKE pattern
    let like_pattern = glob_to_like(pattern);

    let type_clause = match entry_type_filter.to_lowercase().as_str() {
        "file" => "AND entry_type = 'file'",
        "directory" => "AND entry_type = 'directory'",
        "node" => "AND entry_type = 'node'",
        _ => "",
    };

    let sql = format!(
        r#"SELECT vfs_path, entry_type, entry_kind, display_name
           FROM vfs_entries
           WHERE vfs_path LIKE ?1 ESCAPE '\'
           {}
           ORDER BY vfs_path
           LIMIT 500"#,
        type_clause
    );

    let mut stmt = conn.prepare(&sql)?;
    let results = stmt
        .query_map(rusqlite::params![like_pattern], |row| {
            Ok(GlobResult {
                vfs_path: row.get(0)?,
                entry_type: row.get(1)?,
                entry_kind: row.get(2)?,
                display_name: row.get(3)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(results)
}

fn glob_to_like(pattern: &str) -> String {
    let mut result = String::new();
    let mut chars = pattern.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '*' => {
                if chars.peek() == Some(&'*') {
                    chars.next();
                    result.push('%');
                } else {
                    result.push('%');
                }
            }
            '?' => result.push('_'),
            '%' | '_' | '\\' => {
                result.push('\\');
                result.push(c);
            }
            _ => result.push(c),
        }
    }

    result
}
