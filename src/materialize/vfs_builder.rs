use rusqlite::Connection;

/// File bodies above this size are not stored in the index
/// (grep/read fall back to "no indexed content" for them).
const MAX_INDEXED_CONTENT_BYTES: i64 = 256 * 1024;

/// File kinds whose body text is worth indexing for grep/read.
fn is_indexable_text_kind(kind: &str) -> bool {
    matches!(
        kind,
        "scene"
            | "prefab"
            | "csharp"
            | "material"
            | "asset"
            | "yaml-asset"
            | "shader"
            | "shader-include"
            | "asmdef"
            | "asmref"
    )
}

pub struct VfsBuilder<'a> {
    conn: &'a Connection,
    project_id: i64,
}

impl<'a> VfsBuilder<'a> {
    pub fn new(conn: &'a Connection, project_id: i64) -> Self {
        VfsBuilder { conn, project_id }
    }

    pub fn build(&mut self) -> rusqlite::Result<()> {
        self.build_directory_tree()?;
        self.build_file_entries()?;
        self.build_node_entries()?;
        self.build_vfs_edges()?;
        Ok(())
    }

    fn build_directory_tree(&mut self) -> rusqlite::Result<()> {
        // Build directory entries from file paths
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT project_rel_path FROM files WHERE project_id = ?1")?;

        let paths: Vec<String> = stmt
            .query_map(rusqlite::params![self.project_id], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        drop(stmt);

        let mut seen_dirs: std::collections::HashSet<String> = std::collections::HashSet::new();
        seen_dirs.insert("".to_string());

        for path in &paths {
            let normalized = path.replace('\\', "/");
            let parts: Vec<&str> = normalized.split('/').collect();
            let mut current = String::new();

            for (i, part) in parts.iter().enumerate() {
                if part.is_empty() {
                    continue;
                }
                let parent = current.clone();
                if current.is_empty() {
                    current = part.to_string();
                } else {
                    current = format!("{}/{}", current, part);
                }

                // Only create directory entries for intermediate paths (not the file itself)
                if (i < parts.len() - 1 || path.ends_with('/')) && seen_dirs.insert(current.clone())
                {
                    self.conn.execute(
                            "INSERT OR IGNORE INTO vfs_entries
                             (id, project_id, entry_type, entry_kind, vfs_path, parent_vfs_path, display_name)
                             VALUES (NULL, ?1, 'directory', 'directory', ?2, ?3, ?4)",
                            rusqlite::params![
                                self.project_id,
                                &current,
                                if parent.is_empty() { None } else { Some(&parent) },
                                part,
                            ],
                        )?;
                }
            }
        }

        Ok(())
    }

    fn build_file_entries(&mut self) -> rusqlite::Result<()> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project_rel_path, kind, abs_path, size_bytes
             FROM files
             WHERE project_id = ?1 AND kind != 'meta'",
        )?;

        let files: Vec<(i64, String, String, String, i64)> = stmt
            .query_map(rusqlite::params![self.project_id], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        drop(stmt);

        for (file_id, rel_path, kind, abs_path, size_bytes) in files {
            let normalized = rel_path.replace('\\', "/");
            let parent = normalized.rsplit_once('/').map(|(p, _)| p.to_string());

            // Index the body of small text files so `grep` and `read`
            // have something to search; binaries and large files are skipped.
            let content: Option<String> =
                if is_indexable_text_kind(&kind) && size_bytes < MAX_INDEXED_CONTENT_BYTES {
                    std::fs::read_to_string(&abs_path).ok()
                } else {
                    None
                };

            self.conn.execute(
                "INSERT OR IGNORE INTO vfs_entries
                 (id, project_id, entry_type, entry_kind, vfs_path, parent_vfs_path, source_file_id, display_name, content)
                 VALUES (NULL, ?1, 'file', ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    self.project_id,
                    &kind,
                    &normalized,
                    parent.as_deref(),
                    file_id,
                    normalized.rsplit('/').next().unwrap_or(&normalized),
                    content,
                ],
            )?;
        }

        Ok(())
    }

    fn build_node_entries(&mut self) -> rusqlite::Result<()> {
        // Create node entries for entities (GameObjects, Components, Materials, etc.)
        let mut stmt = self.conn.prepare(
            "SELECT e.id, e.asset_id, e.entity_kind, e.local_key, e.name, e.type_name,
                    a.vfs_root_path, e.parent_entity_id
             FROM entities e
             JOIN assets a ON e.asset_id = a.id
             WHERE a.project_id = ?1",
        )?;

        let entities: Vec<(
            i64,
            i64,
            String,
            String,
            Option<String>,
            String,
            String,
            Option<i64>,
        )> = stmt
            .query_map(rusqlite::params![self.project_id], |row| {
                Ok((
                    row.get(0)?,                   // entity_id
                    row.get(1)?,                   // asset_id
                    row.get(2)?,                   // entity_kind
                    row.get(3)?,                   // local_key
                    row.get(4)?,                   // name
                    row.get(5)?,                   // type_name
                    row.get(6)?,                   // vfs_root_path
                    row.get::<_, Option<i64>>(7)?, // parent_entity_id
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        drop(stmt);

        for (
            entity_id,
            _asset_id,
            entity_kind,
            local_key,
            name,
            type_name,
            vfs_root_path,
            _parent_entity_id,
        ) in &entities
        {
            // VFS path: <vfs_root_path>:/<entity_kind>/<local_key>
            let vfs_path = format!("{}:/{}", vfs_root_path, local_key);
            let display_name = name.clone().unwrap_or_else(|| type_name.clone());

            self.conn.execute(
                "INSERT OR IGNORE INTO vfs_entries
                 (id, project_id, entry_type, entry_kind, vfs_path, parent_vfs_path,
                  source_entity_id, display_name)
                 VALUES (NULL, ?1, 'node', ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    self.project_id,
                    entity_kind,
                    &vfs_path,
                    vfs_root_path,
                    entity_id,
                    display_name,
                ],
            )?;
        }

        Ok(())
    }

    fn build_vfs_edges(&mut self) -> rusqlite::Result<()> {
        // All edge inserts let SQLite auto-assign ids (NULL → max rowid + 1).
        // Manually allocating ids via `next_id + ROW_NUMBER()` while
        // `INSERT OR IGNORE` skips duplicate rows let later queries reuse
        // ids that were already taken — silently dropping edges on PK
        // collision. NULL ids make that class of bug impossible.

        // 1. child_of edges: directory → file
        self.conn.execute(
            "INSERT OR IGNORE INTO vfs_edges (id, from_entry_id, to_entry_id, edge_kind)
             SELECT NULL, e.id, d.id, 'child_of'
             FROM vfs_entries e
             JOIN vfs_entries d ON e.parent_vfs_path = d.vfs_path
             WHERE e.project_id = ?1 AND d.project_id = ?1",
            rusqlite::params![self.project_id],
        )?;

        // 2. defined_in edges: node → file
        self.conn.execute(
            "INSERT OR IGNORE INTO vfs_edges (id, from_entry_id, to_entry_id, edge_kind)
             SELECT NULL, n.id, f.id, 'defined_in'
             FROM vfs_entries n
             JOIN vfs_entries f ON n.parent_vfs_path = f.vfs_path
             WHERE n.project_id = ?1 AND f.project_id = ?1
               AND n.entry_type = 'node' AND f.entry_type = 'file'",
            rusqlite::params![self.project_id],
        )?;

        // Resolve GUID references ONCE into an indexed temp table.
        // Joining `lower(guid) = lower(?)` inline lets the planner pick a
        // files × files nested loop (13k × 13k on real projects); a
        // materialized resolution table keeps every later query on
        // index paths.
        self.conn.execute_batch(
            "DROP TABLE IF EXISTS temp.guid_map;
             DROP TABLE IF EXISTS temp.resolved_refs;
             CREATE TEMP TABLE guid_map (file_id INTEGER PRIMARY KEY, lguid TEXT);
             INSERT INTO guid_map SELECT id, lower(guid) FROM files WHERE guid IS NOT NULL;
             CREATE INDEX temp.idx_guid_map_lguid ON guid_map (lguid);
             CREATE TEMP TABLE resolved_refs AS
               SELECT DISTINCT
                      yr.file_id AS from_file_id,
                      gm.file_id AS to_file_id,
                      from_file.kind AS from_kind,
                      yr.ref_kind AS ref_kind
               FROM yaml_references yr
               JOIN files from_file ON from_file.id = yr.file_id
               JOIN guid_map gm ON gm.lguid = lower(yr.target_guid)
               WHERE yr.target_guid IS NOT NULL;
             CREATE INDEX temp.idx_rr_from ON resolved_refs (from_file_id);
             CREATE INDEX temp.idx_rr_to ON resolved_refs (to_file_id);",
        )?;

        // 3. depends_on edges: file → file (from yaml_references via guid)
        self.conn.execute(
            "INSERT OR IGNORE INTO vfs_edges (id, from_entry_id, to_entry_id, edge_kind, edge_subkind)
             SELECT DISTINCT
                NULL, from_entry.id, to_entry.id, 'depends_on', rr.ref_kind
             FROM resolved_refs rr
             JOIN vfs_entries from_entry ON from_entry.source_file_id = rr.from_file_id
                  AND from_entry.entry_type = 'file' AND from_entry.project_id = ?1
             JOIN vfs_entries to_entry ON to_entry.source_file_id = rr.to_file_id
                  AND to_entry.entry_type = 'file' AND to_entry.project_id = ?1",
            rusqlite::params![self.project_id],
        )?;

        // 4. binds_to edges: component node → script class node
        self.conn.execute(
            "INSERT OR IGNORE INTO vfs_edges (id, from_entry_id, to_entry_id, edge_kind, edge_subkind)
             SELECT NULL,
                    comp_entry.id, script_entry.id, 'binds_to', 'component_script'
             FROM entities comp_entity
             JOIN entities script_symbol ON comp_entity.script_symbol_id = script_symbol.id
             JOIN vfs_entries comp_entry ON comp_entry.source_entity_id = comp_entity.id
             JOIN vfs_entries script_entry ON script_entry.source_entity_id = script_symbol.id
             WHERE comp_entity.entity_kind = 'component'",
            rusqlite::params![],
        )?;

        // 5. instance_of edges: prefab instance → source prefab
        self.conn.execute(
            "INSERT OR IGNORE INTO vfs_edges (id, from_entry_id, to_entry_id, edge_kind)
             SELECT DISTINCT
                NULL, from_entry.id, to_entry.id, 'instance_of'
             FROM resolved_refs rr
             JOIN vfs_entries from_entry ON from_entry.source_file_id = rr.from_file_id
                  AND from_entry.entry_type = 'file' AND from_entry.project_id = ?1
             JOIN vfs_entries to_entry ON to_entry.source_file_id = rr.to_file_id
                  AND to_entry.entry_type = 'file' AND to_entry.project_id = ?1
             WHERE rr.from_kind IN ('scene', 'prefab')",
            rusqlite::params![self.project_id],
        )?;

        Ok(())
    }
}
