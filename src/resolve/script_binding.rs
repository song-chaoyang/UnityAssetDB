use rusqlite::Connection;

/// Links MonoBehaviour components to C# symbols via script GUID.
/// For each entity with a script_guid, find the C# file with matching GUID,
/// then find the class symbol in that file.
pub fn bind_scripts_to_symbols(conn: &Connection) -> rusqlite::Result<usize> {
    // Find all entities that have a script_guid but no script_symbol_id
    let mut stmt = conn.prepare(
        "SELECT e.id, yo.script_guid
         FROM entities e
         JOIN yaml_objects yo ON e.yaml_object_id = yo.id
         WHERE yo.script_guid IS NOT NULL
           AND e.script_symbol_id IS NULL",
    )?;

    let entities_to_update: Vec<(i64, String)> = stmt
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .filter_map(|r| r.ok())
        .collect();

    drop(stmt);

    let mut updated = 0;
    for (entity_id, script_guid) in entities_to_update {
        // Find the file with this GUID
        let file_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM files WHERE lower(guid) = lower(?1)",
                rusqlite::params![&script_guid],
                |row| row.get(0),
            )
            .ok();

        if let Some(file_id) = file_id {
            // Find the first class or struct symbol in that file
            let symbol_id: Option<i64> = conn
                .query_row(
                    "SELECT id FROM symbols
                     WHERE file_id = ?1
                       AND symbol_kind IN ('class', 'struct')
                     ORDER BY line_start
                     LIMIT 1",
                    rusqlite::params![file_id],
                    |row| row.get(0),
                )
                .ok();

            if let Some(symbol_id) = symbol_id {
                conn.execute(
                    "UPDATE entities SET script_symbol_id = ?1 WHERE id = ?2",
                    rusqlite::params![symbol_id, entity_id],
                )?;
                updated += 1;
            }
        }
    }

    Ok(updated)
}
