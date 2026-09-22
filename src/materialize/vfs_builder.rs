use rusqlite::Connection;

/// File bodies above this size are not stored in the index
/// (grep/read fall back to "no indexed content" for them).
/// 32MB comfortably covers real-world prefab/scene sizes (largest observed
/// in practice so far: ~24MB) while still bounding worst-case DB bloat from
/// a pathologically huge scene file.
const MAX_INDEXED_CONTENT_BYTES: i64 = 32 * 1024 * 1024;

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
        self.build_with_progress(&mut |_: &str| {})
    }

    /// `on_unit` fires once per unit of work (file entry, node entry, or
    /// set-based edge step), labelled with the item being processed.
    /// Without it the whole materialize phase is a progress-bar black box.
    pub fn build_with_progress(&mut self, on_unit: &mut dyn FnMut(&str)) -> rusqlite::Result<()> {
        self.build_directory_tree()?;
        on_unit("directory tree");
        self.build_file_entries(on_unit)?;
        self.build_node_entries(on_unit)?;
        self.build_vfs_edges(on_unit)?;
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

    fn build_file_entries(&mut self, on_unit: &mut dyn FnMut(&str)) -> rusqlite::Result<()> {
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
            on_unit(&normalized);
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

    fn build_node_entries(&mut self, on_unit: &mut dyn FnMut(&str)) -> rusqlite::Result<()> {
        // Create node entries for entities (GameObjects, Components, Materials, etc.).
        //
        // parent_vfs_path resolution:
        // - Component (including Transform/RectTransform itself): its owning
        //   GameObject, via entities.parent_entity_id directly (`direct_parent`).
        // - GameObject: its owning GameObject is NOT on entities.parent_entity_id
        //   (that column only ever points Component -> GameObject). It has to be
        //   walked: this GameObject's own Transform/RectTransform component
        //   (`own_transform`, found via parent_entity_id = this GameObject) ->
        //   the 'parent_of' edge from that Transform (`pe` / `parent_transform`,
        //   entity_edges is built per-Transform, not per-GameObject) -> that
        //   parent Transform's owning GameObject (`go_parent`). Each hop has
        //   1:0/1:1 cardinality (one GameObject has at most one Transform, one
        //   Transform has at most one parent_of edge), so this is a plain LEFT
        //   JOIN chain, not a recursive query, and never multiplies rows.
        // - Everything else (material/scriptable_object/subasset/shader*/vfx*):
        //   unchanged, flat under the file (no Transform-style hierarchy exists
        //   for these kinds).
        //
        // Transform/RectTransform components are excluded from the result entirely
        // (WHERE clause below) — they're pure hierarchy plumbing with no business
        // value of their own; hiding them means `ls` shows real GameObjects/other
        // components directly nested, matching what you'd see in the Inspector.
        let mut stmt = self.conn.prepare(
            "SELECT e.id, e.asset_id, e.entity_kind, e.local_key, e.name, e.type_name,
                    a.vfs_root_path, e.generated_content,
                    direct_parent.local_key AS direct_parent_local_key,
                    go_parent.local_key     AS go_parent_local_key
             FROM entities e
             JOIN assets a ON e.asset_id = a.id
             LEFT JOIN entities direct_parent
                    ON direct_parent.id = e.parent_entity_id
             LEFT JOIN entities own_transform
                    ON own_transform.parent_entity_id = e.id
                   AND own_transform.entity_kind = 'component'
                   AND own_transform.type_name IN ('Transform', 'RectTransform')
             LEFT JOIN entity_edges pe
                    ON pe.from_entity_id = own_transform.id AND pe.edge_kind = 'parent_of'
             LEFT JOIN entities parent_transform
                    ON parent_transform.id = pe.to_entity_id
             LEFT JOIN entities go_parent
                    ON go_parent.id = parent_transform.parent_entity_id
             WHERE a.project_id = ?1
               AND NOT (e.entity_kind = 'component' AND e.type_name IN ('Transform', 'RectTransform'))",
        )?;

        let entities: Vec<(
            i64,
            i64,
            String,
            String,
            Option<String>,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
        )> = stmt
            .query_map(rusqlite::params![self.project_id], |row| {
                Ok((
                    row.get(0)?,                       // entity_id
                    row.get(1)?,                       // asset_id
                    row.get(2)?,                       // entity_kind
                    row.get(3)?,                       // local_key
                    row.get(4)?,                       // name
                    row.get(5)?,                       // type_name
                    row.get(6)?,                       // vfs_root_path
                    row.get::<_, Option<String>>(7)?,  // generated_content
                    row.get::<_, Option<String>>(8)?,  // direct_parent_local_key
                    row.get::<_, Option<String>>(9)?,  // go_parent_local_key
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
            generated_content,
            direct_parent_local_key,
            go_parent_local_key,
        ) in &entities
        {
            let vfs_path = format!("{}:/{}", vfs_root_path, local_key);
            on_unit(&vfs_path);
            let display_name = name.clone().unwrap_or_else(|| type_name.clone());

            let parent_vfs_path = match entity_kind.as_str() {
                "component" => direct_parent_local_key
                    .as_ref()
                    .map(|k| format!("{}:/{}", vfs_root_path, k))
                    .unwrap_or_else(|| vfs_root_path.clone()),
                "gameobject" => go_parent_local_key
                    .as_ref()
                    .map(|k| format!("{}:/{}", vfs_root_path, k))
                    .unwrap_or_else(|| vfs_root_path.clone()), // root GameObject / no Transform found
                _ => vfs_root_path.clone(),
            };

            self.conn.execute(
                "INSERT OR IGNORE INTO vfs_entries
                 (id, project_id, entry_type, entry_kind, vfs_path, parent_vfs_path,
                  source_entity_id, display_name, content)
                 VALUES (NULL, ?1, 'node', ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    self.project_id,
                    entity_kind,
                    &vfs_path,
                    &parent_vfs_path,
                    entity_id,
                    display_name,
                    generated_content,
                ],
            )?;
        }

        Ok(())
    }


    fn build_vfs_edges(&mut self, on_unit: &mut dyn FnMut(&str)) -> rusqlite::Result<()> {
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
        on_unit("vfs edges: child_of");

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
        on_unit("vfs edges: defined_in");

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
        on_unit("resolving guid refs");

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
        on_unit("vfs edges: depends_on");

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
        on_unit("vfs edges: binds_to");

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
        on_unit("vfs edges: instance_of");

        Ok(())
    }
}
