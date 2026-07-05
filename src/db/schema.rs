pub const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS projects (
    id INTEGER PRIMARY KEY,
    project_path TEXT NOT NULL UNIQUE,
    schema_version INTEGER NOT NULL DEFAULT 1,
    indexed_at TEXT
);

CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL,
    project_rel_path TEXT NOT NULL,
    abs_path TEXT NOT NULL,
    kind TEXT NOT NULL,
    guid TEXT,
    meta_file_id INTEGER,
    size_bytes INTEGER NOT NULL,
    mtime_ms INTEGER NOT NULL,
    content_hash TEXT NOT NULL,
    importer_type TEXT,
    UNIQUE (project_id, project_rel_path),
    FOREIGN KEY (project_id) REFERENCES projects (id) ON DELETE CASCADE,
    FOREIGN KEY (meta_file_id) REFERENCES files (id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_files_guid ON files (guid);
CREATE INDEX IF NOT EXISTS idx_files_kind_project ON files (kind, project_id);
CREATE INDEX IF NOT EXISTS idx_files_meta_file_id ON files (meta_file_id);

CREATE TABLE IF NOT EXISTS assemblies (
    id INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    source TEXT NOT NULL,
    root_namespace TEXT,
    is_editor_only INTEGER NOT NULL DEFAULT 0,
    UNIQUE (project_id, name),
    FOREIGN KEY (project_id) REFERENCES projects (id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_assemblies_project_name ON assemblies (project_id, name);

CREATE TABLE IF NOT EXISTS assembly_references (
    from_assembly_id INTEGER NOT NULL,
    to_assembly_name TEXT NOT NULL,
    is_external INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (from_assembly_id) REFERENCES assemblies (id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_assembly_references_from ON assembly_references (from_assembly_id);

CREATE TABLE IF NOT EXISTS yaml_objects (
    id INTEGER PRIMARY KEY,
    file_id INTEGER NOT NULL,
    doc_index INTEGER NOT NULL,
    unity_class_id INTEGER NOT NULL,
    anchor TEXT,
    object_type TEXT NOT NULL,
    local_identifier TEXT NOT NULL,
    game_object_file_id TEXT,
    component_type_name TEXT,
    script_guid TEXT,
    script_file_id TEXT,
    name TEXT,
    line_start INTEGER NOT NULL,
    line_end INTEGER NOT NULL,
    FOREIGN KEY (file_id) REFERENCES files (id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_yaml_objects_file_id ON yaml_objects (file_id);
CREATE INDEX IF NOT EXISTS idx_yaml_objects_local_identifier ON yaml_objects (file_id, local_identifier);
CREATE INDEX IF NOT EXISTS idx_yaml_objects_script_guid ON yaml_objects (script_guid);

CREATE TABLE IF NOT EXISTS yaml_references (
    id INTEGER PRIMARY KEY,
    file_id INTEGER NOT NULL,
    source_yaml_object_id INTEGER NOT NULL,
    field_path TEXT NOT NULL,
    target_guid TEXT,
    target_file_id TEXT,
    target_local_id TEXT,
    ref_kind TEXT NOT NULL,
    FOREIGN KEY (file_id) REFERENCES files (id) ON DELETE CASCADE,
    FOREIGN KEY (source_yaml_object_id) REFERENCES yaml_objects (id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_yaml_references_source ON yaml_references (source_yaml_object_id);
CREATE INDEX IF NOT EXISTS idx_yaml_references_file ON yaml_references (file_id);
CREATE INDEX IF NOT EXISTS idx_yaml_references_target_guid ON yaml_references (target_guid);

CREATE TABLE IF NOT EXISTS cs_declarations (
    id INTEGER PRIMARY KEY,
    file_id INTEGER NOT NULL,
    decl_kind TEXT NOT NULL,
    simple_name TEXT NOT NULL,
    qualified_name TEXT,
    signature TEXT,
    line_start INTEGER NOT NULL,
    line_end INTEGER NOT NULL,
    FOREIGN KEY (file_id) REFERENCES files (id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_cs_declarations_file_id ON cs_declarations (file_id);
CREATE INDEX IF NOT EXISTS idx_cs_declarations_simple_name ON cs_declarations (simple_name);

CREATE TABLE IF NOT EXISTS cs_mentions (
    id INTEGER PRIMARY KEY,
    file_id INTEGER NOT NULL,
    mention_kind TEXT NOT NULL,
    text TEXT NOT NULL,
    receiver_text TEXT,
    containing_declaration_id INTEGER,
    line_start INTEGER NOT NULL,
    line_end INTEGER NOT NULL,
    FOREIGN KEY (file_id) REFERENCES files (id) ON DELETE CASCADE,
    FOREIGN KEY (containing_declaration_id) REFERENCES cs_declarations (id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_cs_mentions_file_id ON cs_mentions (file_id);
CREATE INDEX IF NOT EXISTS idx_cs_mentions_text ON cs_mentions (text);

CREATE TABLE IF NOT EXISTS symbols (
    id INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL,
    file_id INTEGER,
    declaration_id INTEGER,
    symbol_kind TEXT NOT NULL,
    simple_name TEXT NOT NULL,
    qualified_name TEXT NOT NULL,
    display_name TEXT NOT NULL,
    signature TEXT,
    containing_symbol_id INTEGER,
    base_symbol_name TEXT,
    visibility TEXT,
    line_start INTEGER,
    line_end INTEGER,
    FOREIGN KEY (project_id) REFERENCES projects (id) ON DELETE CASCADE,
    FOREIGN KEY (file_id) REFERENCES files (id) ON DELETE SET NULL,
    FOREIGN KEY (containing_symbol_id) REFERENCES symbols (id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_symbols_file_id ON symbols (file_id);
CREATE INDEX IF NOT EXISTS idx_symbols_simple_name ON symbols (simple_name);
CREATE INDEX IF NOT EXISTS idx_symbols_qualified_name ON symbols (qualified_name);

CREATE TABLE IF NOT EXISTS symbol_edges (
    id INTEGER PRIMARY KEY,
    from_symbol_id INTEGER NOT NULL,
    to_symbol_id INTEGER NOT NULL,
    edge_kind TEXT NOT NULL,
    source_file_id INTEGER,
    FOREIGN KEY (from_symbol_id) REFERENCES symbols (id) ON DELETE CASCADE,
    FOREIGN KEY (to_symbol_id) REFERENCES symbols (id) ON DELETE CASCADE,
    FOREIGN KEY (source_file_id) REFERENCES files (id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_symbol_edges_from ON symbol_edges (from_symbol_id);
CREATE INDEX IF NOT EXISTS idx_symbol_edges_to ON symbol_edges (to_symbol_id);

CREATE TABLE IF NOT EXISTS assets (
    id INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL,
    file_id INTEGER NOT NULL,
    asset_kind TEXT NOT NULL,
    guid TEXT NOT NULL,
    name TEXT NOT NULL,
    vfs_root_path TEXT NOT NULL,
    UNIQUE (project_id, guid),
    FOREIGN KEY (project_id) REFERENCES projects (id) ON DELETE CASCADE,
    FOREIGN KEY (file_id) REFERENCES files (id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_assets_file_id ON assets (file_id);
CREATE INDEX IF NOT EXISTS idx_assets_guid ON assets (guid);

CREATE TABLE IF NOT EXISTS entities (
    id INTEGER PRIMARY KEY,
    asset_id INTEGER NOT NULL,
    yaml_object_id INTEGER,
    entity_kind TEXT NOT NULL,
    local_key TEXT NOT NULL,
    name TEXT,
    hierarchy_name TEXT,
    hierarchy_order INTEGER NOT NULL DEFAULT 0,
    type_name TEXT NOT NULL,
    script_symbol_id INTEGER,
    parent_entity_id INTEGER,
    source_entity_id INTEGER,
    line_start INTEGER NOT NULL,
    line_end INTEGER NOT NULL,
    generated_content TEXT,
    FOREIGN KEY (asset_id) REFERENCES assets (id) ON DELETE CASCADE,
    FOREIGN KEY (yaml_object_id) REFERENCES yaml_objects (id) ON DELETE CASCADE,
    FOREIGN KEY (script_symbol_id) REFERENCES symbols (id) ON DELETE SET NULL,
    FOREIGN KEY (parent_entity_id) REFERENCES entities (id) ON DELETE SET NULL,
    FOREIGN KEY (source_entity_id) REFERENCES entities (id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_entities_asset_id ON entities (asset_id);
CREATE INDEX IF NOT EXISTS idx_entities_yaml_object_id ON entities (yaml_object_id);
CREATE INDEX IF NOT EXISTS idx_entities_parent ON entities (parent_entity_id);
CREATE INDEX IF NOT EXISTS idx_entities_script_symbol ON entities (script_symbol_id);

CREATE TABLE IF NOT EXISTS entity_edges (
    id INTEGER PRIMARY KEY,
    from_entity_id INTEGER NOT NULL,
    to_entity_id INTEGER NOT NULL,
    edge_kind TEXT NOT NULL,
    edge_subkind TEXT,
    FOREIGN KEY (from_entity_id) REFERENCES entities (id) ON DELETE CASCADE,
    FOREIGN KEY (to_entity_id) REFERENCES entities (id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_entity_edges_from ON entity_edges (from_entity_id);
CREATE INDEX IF NOT EXISTS idx_entity_edges_to ON entity_edges (to_entity_id);
CREATE UNIQUE INDEX IF NOT EXISTS uq_entity_edges ON entity_edges (from_entity_id, to_entity_id, edge_kind, COALESCE(edge_subkind, ''));

CREATE TABLE IF NOT EXISTS entity_symbol_edges (
    id INTEGER PRIMARY KEY,
    from_entity_id INTEGER NOT NULL,
    to_symbol_id INTEGER NOT NULL,
    edge_kind TEXT NOT NULL,
    edge_subkind TEXT,
    source_field_path TEXT,
    FOREIGN KEY (from_entity_id) REFERENCES entities (id) ON DELETE CASCADE,
    FOREIGN KEY (to_symbol_id) REFERENCES symbols (id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_entity_symbol_edges_from ON entity_symbol_edges (from_entity_id);
CREATE INDEX IF NOT EXISTS idx_entity_symbol_edges_to ON entity_symbol_edges (to_symbol_id);

CREATE TABLE IF NOT EXISTS vfs_entries (
    id INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL,
    entry_type TEXT NOT NULL,
    entry_kind TEXT NOT NULL,
    vfs_path TEXT NOT NULL,
    parent_vfs_path TEXT,
    source_file_id INTEGER,
    source_entity_id INTEGER,
    display_name TEXT NOT NULL,
    child_order INTEGER NOT NULL DEFAULT 2000000000,
    content TEXT,
    meta_content TEXT,
    line_start INTEGER,
    line_end INTEGER,
    target_vfs_path TEXT,
    UNIQUE (project_id, vfs_path),
    FOREIGN KEY (project_id) REFERENCES projects (id) ON DELETE CASCADE,
    FOREIGN KEY (source_file_id) REFERENCES files (id) ON DELETE SET NULL,
    FOREIGN KEY (source_entity_id) REFERENCES entities (id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_vfs_entries_parent ON vfs_entries (project_id, parent_vfs_path);
CREATE INDEX IF NOT EXISTS idx_vfs_entries_kind ON vfs_entries (project_id, entry_kind);
CREATE INDEX IF NOT EXISTS idx_vfs_entries_source_file ON vfs_entries (source_file_id);
CREATE INDEX IF NOT EXISTS idx_vfs_entries_source_entity ON vfs_entries (source_entity_id);

CREATE TABLE IF NOT EXISTS vfs_edges (
    id INTEGER PRIMARY KEY,
    from_entry_id INTEGER NOT NULL,
    to_entry_id INTEGER NOT NULL,
    edge_kind TEXT NOT NULL,
    edge_subkind TEXT,
    FOREIGN KEY (from_entry_id) REFERENCES vfs_entries (id) ON DELETE CASCADE,
    FOREIGN KEY (to_entry_id) REFERENCES vfs_entries (id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_vfs_edges_to_kind ON vfs_edges (to_entry_id, edge_kind);
CREATE INDEX IF NOT EXISTS idx_vfs_edges_from_kind ON vfs_edges (from_entry_id, edge_kind);
CREATE UNIQUE INDEX IF NOT EXISTS uq_vfs_edges ON vfs_edges (from_entry_id, to_entry_id, edge_kind, COALESCE(edge_subkind, ''));

CREATE TABLE IF NOT EXISTS index_diagnostics (
    id INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL,
    severity TEXT NOT NULL,
    category TEXT NOT NULL,
    code TEXT,
    stage TEXT,
    file_path TEXT,
    message TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects (id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS rebuild_summary (
    project_id INTEGER PRIMARY KEY,
    mode TEXT NOT NULL,
    discovered_file_count INTEGER NOT NULL,
    diagnostic_count INTEGER NOT NULL,
    completed_stages_json TEXT NOT NULL,
    published_index_path TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects (id) ON DELETE CASCADE
);
"#;
