#![allow(clippy::explicit_counter_loop)]

use rusqlite::params;
use std::path::Path;
use unityassetdb::db;
use unityassetdb::discovery;
use unityassetdb::extract;
use unityassetdb::materialize::VfsBuilder;
use unityassetdb::model::{AssetKind, FileKind, MetaInfo};
use unityassetdb::query;
use unityassetdb::resolve::{script_binding, EntityGraphBuilder};

const FIXTURE_ROOT: &str = "tests/fixtures/TestProject";

// ═══════════════════════════════════════════════════════════════════
//  Unit Tests: Meta File Parsing
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_meta_parse_guid() {
    let content = "guid: a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4
MonoImporter:
  externalObjects: {}
  serializedVersion: 2
  mainObjectFileID: 2100000
";
    let meta = MetaInfo::parse(content);
    assert_eq!(
        meta.guid.as_deref(),
        Some("a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4")
    );
    assert_eq!(meta.importer_type.as_deref(), Some("MonoImporter"));
    assert_eq!(meta.meta_main_object_file_id, Some(2100000));
}

#[test]
fn test_meta_parse_no_guid() {
    let content = "fileFormatVersion: 2
";
    let meta = MetaInfo::parse(content);
    assert!(meta.guid.is_none());
}

// ═══════════════════════════════════════════════════════════════════
//  Unit Tests: File Kind Classification
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_file_kind_classification() {
    assert_eq!(FileKind::from_extension(".unity"), FileKind::Scene);
    assert_eq!(FileKind::from_extension(".prefab"), FileKind::Prefab);
    assert_eq!(FileKind::from_extension(".cs"), FileKind::CSharp);
    assert_eq!(FileKind::from_extension(".mat"), FileKind::Material);
    assert_eq!(FileKind::from_extension(".asset"), FileKind::Asset);
    assert_eq!(FileKind::from_extension(".meta"), FileKind::Meta);
    assert_eq!(FileKind::from_extension(".shader"), FileKind::Shader);
    assert_eq!(FileKind::from_extension(".png"), FileKind::Binary);
    assert_eq!(FileKind::from_extension(".fbx"), FileKind::Binary);
}

#[test]
fn test_file_kind_is_unity_yaml() {
    assert!(FileKind::Scene.is_unity_yaml());
    assert!(FileKind::Prefab.is_unity_yaml());
    assert!(FileKind::YamlAsset.is_unity_yaml());
    assert!(!FileKind::CSharp.is_unity_yaml());
    assert!(!FileKind::Binary.is_unity_yaml());
}

// ═══════════════════════════════════════════════════════════════════
//  Unit Tests: Unity YAML Extraction
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_yaml_extract_material() {
    let content = r#"%YAML 1.1
%TAG !u! tag:unity3d.com,2011:
--- !u!21 &2100000
Material:
  serializedVersion: 8
  m_ObjectHideFlags: 0
  m_Name: Enemy
  m_Shader: {fileID: 4800000, guid: b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8, type: 3}
"#;
    let result = extract::extract_from_unity_yaml(content).expect("extraction should succeed");
    assert_eq!(result.objects.len(), 1);
    let obj = &result.objects[0];
    assert_eq!(obj.object_type, "Material");
    assert_eq!(obj.local_identifier, "2100000");
    assert_eq!(obj.name.as_deref(), Some("Enemy"));
    assert_eq!(obj.unity_class_id, 21);

    assert_eq!(result.references.len(), 1);
    let r = &result.references[0];
    assert_eq!(r.field_path, "m_Shader");
    assert_eq!(
        r.target_guid.as_deref(),
        Some("b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8")
    );
    assert_eq!(r.ref_kind, "guid-file");
}

#[test]
fn test_yaml_extract_scene_with_gameobjects() {
    let content = r#"%YAML 1.1
%TAG !u! tag:unity3d.com,2011:
--- !u!1 &100000
GameObject:
  m_ObjectHideFlags: 0
  m_Name: Player
  m_Component:
  - component: {fileID: 400000}
  - component: {fileID: 11400000}
--- !u!4 &400000
Transform:
  m_GameObject: {fileID: 100000}
  m_LocalPosition: {x: 0, y: 0, z: 0}
  m_Children: []
--- !u!114 &11400000
MonoBehaviour:
  m_GameObject: {fileID: 100000}
  m_Enabled: 1
  m_Script: {fileID: 11500000, guid: d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0, type: 3}
"#;
    let result = extract::extract_from_unity_yaml(content).expect("extraction should succeed");
    assert_eq!(result.objects.len(), 3);

    let go = result
        .objects
        .iter()
        .find(|o| o.object_type == "GameObject")
        .unwrap();
    assert_eq!(go.name.as_deref(), Some("Player"));

    let mb = result
        .objects
        .iter()
        .find(|o| o.object_type == "MonoBehaviour")
        .unwrap();
    assert_eq!(
        mb.script_guid.as_deref(),
        Some("d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0")
    );

    // Should have: m_Component[0].component, m_Component[1].component, m_Script, m_GameObject
    assert!(result.references.len() >= 3);
}

#[test]
fn test_yaml_extract_prefab_instance() {
    let content = r#"%YAML 1.1
%TAG !u! tag:unity3d.com,2011:
--- !u!1001 &100100000
PrefabInstance:
  m_ObjectHideFlags: 0
  m_SourcePrefab: {fileID: 100100000, guid: e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1, type: 3}
"#;
    let result = extract::extract_from_unity_yaml(content).expect("extraction should succeed");
    assert_eq!(result.objects.len(), 1);
    assert_eq!(result.objects[0].object_type, "PrefabInstance");

    let prefab_ref = result
        .references
        .iter()
        .find(|r| r.field_path == "m_SourcePrefab")
        .expect("should have m_SourcePrefab reference");
    assert_eq!(
        prefab_ref.target_guid.as_deref(),
        Some("e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1")
    );
}

#[test]
fn test_yaml_non_unity_content() {
    assert!(extract::extract_from_unity_yaml("hello world").is_none());
    assert!(extract::extract_from_unity_yaml("name: test").is_none());
}

#[test]
fn test_yaml_local_file_reference() {
    let content = r#"%YAML 1.1
%TAG !u! tag:unity3d.com,2011:
--- !u!4 &400000
Transform:
  m_GameObject: {fileID: 100000}
  m_Children: []
"#;
    let result = extract::extract_from_unity_yaml(content).unwrap();
    let local_ref = result
        .references
        .iter()
        .find(|r| r.ref_kind == "local-file")
        .expect("should have a local-file reference");
    assert_eq!(local_ref.field_path, "m_GameObject");
    assert_eq!(local_ref.target_file_id.as_deref(), Some("100000"));
    assert!(local_ref.target_guid.is_none());
}

// ═══════════════════════════════════════════════════════════════════
//  Unit Tests: C# Tree-Sitter Extraction
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_csharp_extract_class_and_methods() {
    let source = r#"using UnityEngine;

public class PlayerController : MonoBehaviour
{
    public float speed = 5.0f;

    void Start()
    {
        Debug.Log("Hello");
    }

    void Update()
    {
        float h = Input.GetAxis("Horizontal");
    }
}
"#;
    let result = extract::extract_from_csharp(source).expect("C# extraction should succeed");

    let class_decl = result
        .declarations
        .iter()
        .find(|d| d.decl_kind == "class")
        .expect("should find class declaration");
    assert_eq!(class_decl.simple_name, "PlayerController");
    assert!(class_decl.qualified_name.contains("PlayerController"));

    let methods: Vec<_> = result
        .declarations
        .iter()
        .filter(|d| d.decl_kind == "method")
        .collect();
    assert!(
        methods.len() >= 2,
        "should find at least 2 methods, got {}",
        methods.len()
    );

    let method_names: Vec<&str> = methods.iter().map(|m| m.simple_name.as_str()).collect();
    assert!(method_names.contains(&"Start"));
    assert!(method_names.contains(&"Update"));
}

#[test]
fn test_csharp_extract_struct_and_interface() {
    let source = r#"
public struct Vector3Data
{
    public float x;
    public float y;
}

public interface IDamageable
{
    void TakeDamage(float amount);
}
"#;
    let result = extract::extract_from_csharp(source).expect("C# extraction should succeed");

    assert!(result
        .declarations
        .iter()
        .any(|d| d.decl_kind == "struct" && d.simple_name == "Vector3Data"));
    assert!(result
        .declarations
        .iter()
        .any(|d| d.decl_kind == "interface" && d.simple_name == "IDamageable"));
    assert!(result
        .declarations
        .iter()
        .any(|d| d.decl_kind == "method" && d.simple_name == "TakeDamage"));
}

#[test]
fn test_csharp_empty_source() {
    let result = extract::extract_from_csharp("// just a comment");
    assert!(result.is_some());
    let result = result.unwrap();
    assert!(result.declarations.is_empty());
}

// ═══════════════════════════════════════════════════════════════════
//  Integration Tests: Full Indexing Pipeline
// ═══════════════════════════════════════════════════════════════════

fn build_test_index() -> rusqlite::Connection {
    let conn = db::open_in_memory().unwrap();
    db::init_schema(&conn).unwrap();

    let project_root = Path::new(FIXTURE_ROOT);
    let discovery_result = discovery::discover_files(project_root, true);

    // Insert project
    conn.execute(
        "INSERT INTO projects (id, project_path, schema_version) VALUES (1, ?1, 1)",
        params![project_root.to_string_lossy()],
    )
    .unwrap();

    // Insert files
    for file in &discovery_result.files {
        conn.execute(
            "INSERT INTO files (id, project_id, project_rel_path, abs_path, kind, guid,
                                size_bytes, mtime_ms, content_hash, importer_type)
             VALUES (NULL, 1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                file.project_rel_path,
                file.abs_path,
                file.kind.as_str(),
                file.guid,
                file.size_bytes as i64,
                file.mtime_ms,
                file.content_hash,
                file.importer_type,
            ],
        )
        .unwrap();
    }

    // Extract and insert YAML objects + references
    for file in &discovery_result.files {
        if !file.kind.is_unity_yaml() {
            continue;
        }
        let content = std::fs::read_to_string(&file.abs_path).unwrap();
        let file_id: i64 = conn
            .query_row(
                "SELECT id FROM files WHERE project_rel_path = ?1",
                params![&file.project_rel_path],
                |row| row.get(0),
            )
            .unwrap();

        if let Some(result) = extract::extract_from_unity_yaml(&content) {
            let mut next_obj_id = conn
                .query_row(
                    "SELECT COALESCE(MAX(id), 0) + 1 FROM yaml_objects",
                    [],
                    |r| r.get::<_, i64>(0),
                )
                .unwrap();

            for obj in &result.objects {
                conn.execute(
                    "INSERT INTO yaml_objects
                     (id, file_id, doc_index, unity_class_id, anchor, object_type,
                      local_identifier, game_object_file_id, component_type_name,
                      script_guid, script_file_id, name, line_start, line_end)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                    params![
                        next_obj_id,
                        file_id,
                        obj.doc_index as i64,
                        obj.unity_class_id,
                        obj.anchor,
                        obj.object_type,
                        obj.local_identifier,
                        obj.game_object_file_id,
                        obj.component_type_name,
                        obj.script_guid,
                        obj.script_file_id,
                        obj.name,
                        obj.line_start,
                        obj.line_end,
                    ],
                )
                .unwrap();
                next_obj_id += 1;
            }

            let mut next_ref_id = conn
                .query_row(
                    "SELECT COALESCE(MAX(id), 0) + 1 FROM yaml_references",
                    [],
                    |r| r.get::<_, i64>(0),
                )
                .unwrap();

            for r in &result.references {
                conn.execute(
                    "INSERT INTO yaml_references
                     (id, file_id, source_yaml_object_id, field_path,
                      target_guid, target_file_id, target_local_id, ref_kind)
                     VALUES (?1, ?2,
                        (SELECT id FROM yaml_objects WHERE file_id = ?2 AND local_identifier = ?3 LIMIT 1),
                        ?4, ?5, ?6, ?7, ?8)",
                    params![
                        next_ref_id, file_id, r.source_local_identifier,
                        r.field_path, r.target_guid, r.target_file_id,
                        r.target_local_id, r.ref_kind,
                    ],
                ).unwrap();
                next_ref_id += 1;
            }
        }
    }

    // Extract and insert C# declarations
    for file in &discovery_result.files {
        if file.kind != FileKind::CSharp {
            continue;
        }
        let content = std::fs::read_to_string(&file.abs_path).unwrap();
        let file_id: i64 = conn
            .query_row(
                "SELECT id FROM files WHERE project_rel_path = ?1",
                params![&file.project_rel_path],
                |row| row.get(0),
            )
            .unwrap();

        if let Some(result) = extract::extract_from_csharp(&content) {
            let mut next_decl_id = conn
                .query_row(
                    "SELECT COALESCE(MAX(id), 0) + 1 FROM cs_declarations",
                    [],
                    |r| r.get::<_, i64>(0),
                )
                .unwrap();

            for decl in &result.declarations {
                conn.execute(
                    "INSERT INTO cs_declarations
                     (id, file_id, decl_kind, simple_name, qualified_name, signature, line_start, line_end)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        next_decl_id, file_id, decl.decl_kind, decl.simple_name,
                        decl.qualified_name, decl.signature,
                        decl.line_start as i64, decl.line_end as i64,
                    ],
                ).unwrap();
                next_decl_id += 1;
            }
        }
    }

    // Build assets
    let mut next_asset_id = conn
        .query_row("SELECT COALESCE(MAX(id), 0) + 1 FROM assets", [], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap();

    for file in &discovery_result.files {
        if let Some(guid) = &file.guid {
            if file.kind == FileKind::Meta {
                continue;
            }
            let file_id: i64 = conn
                .query_row(
                    "SELECT id FROM files WHERE project_rel_path = ?1",
                    params![&file.project_rel_path],
                    |row| row.get(0),
                )
                .unwrap();

            let asset_kind = AssetKind::from_file_kind(&file.kind).unwrap_or(AssetKind::YamlAsset);
            let name = Path::new(&file.project_rel_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            conn.execute(
                "INSERT OR IGNORE INTO assets (id, project_id, file_id, asset_kind, guid, name, vfs_root_path)
                 VALUES (?1, 1, ?2, ?3, ?4, ?5, ?6)",
                params![next_asset_id, file_id, asset_kind.as_str(), guid, name, file.project_rel_path],
            ).unwrap();
            next_asset_id += 1;
        }
    }

    // Build symbols
    let mut next_symbol_id = conn
        .query_row("SELECT COALESCE(MAX(id), 0) + 1 FROM symbols", [], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap();

    let decls: Vec<(i64, i64, String, String, String, i64, i64)> =
        {
            let mut stmt = conn.prepare(
            "SELECT id, file_id, decl_kind, simple_name, qualified_name, line_start, line_end
             FROM cs_declarations ORDER BY file_id, id"
        ).unwrap();
            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                })
                .unwrap();
            let mut result = Vec::new();
            for v in rows.flatten() {
                result.push(v);
            }
            result
        };

    for (decl_id, file_id, decl_kind, simple_name, qualified_name, line_start, line_end) in &decls {
        conn.execute(
            "INSERT INTO symbols
             (id, project_id, file_id, declaration_id, symbol_kind, simple_name,
              qualified_name, display_name, line_start, line_end)
             VALUES (?1, 1, ?2, ?3, ?4, ?5, ?6, ?5, ?7, ?8)",
            params![
                next_symbol_id,
                file_id,
                decl_id,
                decl_kind,
                simple_name,
                qualified_name,
                line_start,
                line_end
            ],
        )
        .unwrap();
        next_symbol_id += 1;
    }

    // Build entities
    let mut next_entity_id = conn
        .query_row("SELECT COALESCE(MAX(id), 0) + 1 FROM entities", [], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap();

    let assets: Vec<(i64, i64, String)> = {
        let mut stmt = conn
            .prepare("SELECT id, file_id, asset_kind FROM assets WHERE project_id = 1")
            .unwrap();
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap();
        let mut result = Vec::new();
        for v in rows.flatten() {
            result.push(v);
        }
        result
    };

    for (asset_id, file_id, asset_kind_str) in &assets {
        let asset_kind = match asset_kind_str.as_str() {
            "scene" => AssetKind::Scene,
            "prefab" => AssetKind::Prefab,
            "material" => AssetKind::Material,
            "script" => AssetKind::Script,
            "yaml-asset" => AssetKind::YamlAsset,
            _ => AssetKind::YamlAsset,
        };

        // Load YAML objects
        let yaml_objects: Vec<extract::YamlObject> = {
            let mut stmt = conn
                .prepare(
                    "SELECT doc_index, unity_class_id, anchor, object_type, local_identifier,
                        game_object_file_id, component_type_name, script_guid, script_file_id,
                        name, line_start, line_end
                 FROM yaml_objects WHERE file_id = ?1",
                )
                .unwrap();
            let rows = stmt
                .query_map(params![file_id], |row| {
                    Ok(extract::YamlObject {
                        doc_index: row.get::<_, i64>(0)? as usize,
                        unity_class_id: row.get(1)?,
                        anchor: row.get(2)?,
                        object_type: row.get(3)?,
                        local_identifier: row.get(4)?,
                        game_object_file_id: row.get(5)?,
                        component_type_name: row.get(6)?,
                        script_guid: row.get(7)?,
                        script_file_id: row.get(8)?,
                        name: row.get(9)?,
                        line_start: row.get(10)?,
                        line_end: row.get(11)?,
                        payload: serde_yaml::Value::Null,
                    })
                })
                .unwrap();
            let mut result = Vec::new();
            for v in rows.flatten() {
                result.push(v);
            }
            result
        };

        let mut builder = EntityGraphBuilder::new();
        builder.build_for_asset(*asset_id, &asset_kind, &yaml_objects);

        for entity in &builder.entities {
            let yaml_obj_id: Option<i64> = conn.query_row(
                "SELECT id FROM yaml_objects WHERE file_id = ?1 AND local_identifier = ?2 LIMIT 1",
                params![file_id, entity.local_key],
                |row| row.get(0),
            ).ok();

            conn.execute(
                "INSERT INTO entities
                 (id, asset_id, yaml_object_id, entity_kind, local_key, name, type_name, line_start, line_end)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    next_entity_id, asset_id, yaml_obj_id,
                    entity.entity_kind.as_str(), entity.local_key,
                    entity.name, entity.type_name,
                    entity.line_start, entity.line_end,
                ],
            ).unwrap();
            next_entity_id += 1;
        }
    }

    // Script binding
    let _ = script_binding::bind_scripts_to_symbols(&conn);

    // VFS materialization
    let mut vfs_builder = VfsBuilder::new(&conn, 1);
    vfs_builder.build().unwrap();
    conn
}

// ═══════════════════════════════════════════════════════════════════
//  Integration Tests: Verify Index Content
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_index_files_discovered() {
    let conn = build_test_index();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM files WHERE kind != 'meta'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(
        count >= 5,
        "should discover at least 5 non-meta files, got {}",
        count
    );
}

#[test]
fn test_index_yaml_objects_extracted() {
    let conn = build_test_index();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM yaml_objects", [], |r| r.get(0))
        .unwrap();
    assert!(
        count >= 5,
        "should extract at least 5 YAML objects, got {}",
        count
    );

    // Check specific objects
    let has_material = conn
        .query_row(
            "SELECT COUNT(*) FROM yaml_objects WHERE object_type = 'Material'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
        > 0;
    assert!(has_material, "should have Material objects");

    let has_gameobject = conn
        .query_row(
            "SELECT COUNT(*) FROM yaml_objects WHERE object_type = 'GameObject'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
        > 0;
    assert!(has_gameobject, "should have GameObject objects");

    let has_transform = conn
        .query_row(
            "SELECT COUNT(*) FROM yaml_objects WHERE object_type = 'Transform'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
        > 0;
    assert!(has_transform, "should have Transform objects");

    let has_monobehaviour = conn
        .query_row(
            "SELECT COUNT(*) FROM yaml_objects WHERE object_type = 'MonoBehaviour'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
        > 0;
    assert!(has_monobehaviour, "should have MonoBehaviour objects");
}

#[test]
fn test_index_yaml_references_extracted() {
    let conn = build_test_index();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM yaml_references", [], |r| r.get(0))
        .unwrap();
    assert!(
        count >= 5,
        "should extract at least 5 YAML references, got {}",
        count
    );

    // Check guid-file references
    let guid_refs: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM yaml_references WHERE ref_kind = 'guid-file'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(guid_refs > 0, "should have guid-file references");

    // Check local-file references
    let local_refs: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM yaml_references WHERE ref_kind = 'local-file'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(local_refs > 0, "should have local-file references");
}

#[test]
fn test_index_cs_declarations_extracted() {
    let conn = build_test_index();
    let class_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cs_declarations WHERE decl_kind = 'class'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(class_count > 0, "should find at least 1 class declaration");

    let has_player_controller = conn
        .query_row(
            "SELECT COUNT(*) FROM cs_declarations WHERE simple_name = 'PlayerController'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
        > 0;
    assert!(has_player_controller, "should find PlayerController class");
}

#[test]
fn test_index_assets_built() {
    let conn = build_test_index();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM assets", [], |r| r.get(0))
        .unwrap();
    assert!(count >= 4, "should build at least 4 assets, got {}", count);

    // Check specific assets
    let has_material = conn
        .query_row(
            "SELECT COUNT(*) FROM assets WHERE asset_kind = 'material'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
        > 0;
    assert!(has_material);

    let has_scene = conn
        .query_row(
            "SELECT COUNT(*) FROM assets WHERE asset_kind = 'scene'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
        > 0;
    assert!(has_scene);

    let has_prefab = conn
        .query_row(
            "SELECT COUNT(*) FROM assets WHERE asset_kind = 'prefab'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
        > 0;
    assert!(has_prefab);
}

#[test]
fn test_index_entities_built() {
    let conn = build_test_index();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))
        .unwrap();
    assert!(
        count >= 5,
        "should build at least 5 entities, got {}",
        count
    );

    let has_gameobject = conn
        .query_row(
            "SELECT COUNT(*) FROM entities WHERE entity_kind = 'gameobject'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
        > 0;
    assert!(has_gameobject, "should have GameObject entities");

    let has_material = conn
        .query_row(
            "SELECT COUNT(*) FROM entities WHERE entity_kind = 'material'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
        > 0;
    assert!(has_material, "should have Material entities");
}

#[test]
fn test_index_vfs_entries_built() {
    let conn = build_test_index();
    let file_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM vfs_entries WHERE entry_type = 'file'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        file_count >= 4,
        "should have at least 4 file VFS entries, got {}",
        file_count
    );

    let dir_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM vfs_entries WHERE entry_type = 'directory'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        dir_count >= 3,
        "should have at least 3 directory entries, got {}",
        dir_count
    );

    let node_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM vfs_entries WHERE entry_type = 'node'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        node_count >= 3,
        "should have at least 3 node entries, got {}",
        node_count
    );
}

#[test]
fn test_index_vfs_edges_built() {
    let conn = build_test_index();
    let child_of: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM vfs_edges WHERE edge_kind = 'child_of'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(child_of > 0, "should have child_of edges");

    let depends_on: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM vfs_edges WHERE edge_kind = 'depends_on'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(depends_on > 0, "should have depends_on edges");
}

// ═══════════════════════════════════════════════════════════════════
//  Integration Tests: Query Layer
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_query_refs_incoming_for_material() {
    let conn = build_test_index();
    let results = query::query_refs(
        &conn,
        "Assets/Materials/Enemy.mat",
        query::RefDirection::In,
        "ALL",
    )
    .unwrap();

    // The scene Game.unity references Enemy.mat via MeshRenderer.m_Materials
    assert!(
        !results.is_empty(),
        "should find incoming references for Enemy.mat"
    );

    let has_scene_ref = results
        .iter()
        .any(|r| r.to_path.contains("Enemy.mat") && r.edge_kind == "depends_on");
    assert!(has_scene_ref, "should have depends_on edge to Enemy.mat");
}

#[test]
fn test_query_refs_outgoing_for_scene() {
    let conn = build_test_index();
    let results = query::query_refs(
        &conn,
        "Assets/Scenes/Game.unity",
        query::RefDirection::Out,
        "ALL",
    )
    .unwrap();

    assert!(
        !results.is_empty(),
        "should find outgoing references from Game.unity"
    );

    // Scene should reference the material
    let refs_material = results.iter().any(|r| r.to_path.contains("Enemy.mat"));
    assert!(refs_material, "Game.unity should reference Enemy.mat");
}

#[test]
fn test_query_refs_with_file_filter() {
    let conn = build_test_index();
    let results = query::query_refs(
        &conn,
        "Assets/Materials/Enemy.mat",
        query::RefDirection::In,
        "File",
    )
    .unwrap();

    for r in &results {
        assert_eq!(
            r.from_type, "file",
            "all results should be file type when filter=File"
        );
    }
}

#[test]
fn test_query_ls_root() {
    let conn = build_test_index();
    let entries = query::query_ls(&conn, "Assets", 1).unwrap();
    assert!(!entries.is_empty(), "should list entries under Assets/");

    let has_materials = entries.iter().any(|e| e.display_name == "Materials");
    let has_scripts = entries.iter().any(|e| e.display_name == "Scripts");
    let has_scenes = entries.iter().any(|e| e.display_name == "Scenes");
    assert!(
        has_materials || has_scripts || has_scenes,
        "should find known directories"
    );
}

#[test]
fn test_query_ls_deep() {
    let conn = build_test_index();
    let entries = query::query_ls(&conn, "Assets", 3).unwrap();
    let has_mat_file = entries.iter().any(|e| e.vfs_path.contains("Enemy.mat"));
    assert!(has_mat_file, "should find Enemy.mat with depth=3");
}

#[test]
fn test_query_glob_materials() {
    let conn = build_test_index();
    let results = query::query_glob(&conn, "*Enemy*", "ALL").unwrap();
    assert!(!results.is_empty(), "should find entries matching *Enemy*");

    let has_mat = results.iter().any(|r| r.vfs_path.contains("Enemy.mat"));
    assert!(has_mat, "should find Enemy.mat in glob results");
}

#[test]
fn test_query_glob_prefabs() {
    let conn = build_test_index();
    let results = query::query_glob(&conn, "*.prefab", "file").unwrap();
    assert!(!results.is_empty(), "should find .prefab files");

    let has_player = results.iter().any(|r| r.vfs_path.contains("Player.prefab"));
    assert!(has_player, "should find Player.prefab");
}

#[test]
fn test_query_read_file() {
    let conn = build_test_index();
    let result = query::query_read(&conn, "Assets/Materials/Enemy.mat").unwrap();
    assert!(
        result.is_some(),
        "should be able to read Enemy.mat VFS entry"
    );

    let entry = result.unwrap();
    assert_eq!(entry.entry_type, "file");
}

#[test]
fn test_query_read_nonexistent() {
    let conn = build_test_index();
    let result = query::query_read(&conn, "Assets/Nonexistent.foo").unwrap();
    assert!(result.is_none(), "should return None for nonexistent path");
}

// ═══════════════════════════════════════════════════════════════════
//  Integration Tests: Cross-Asset Reference Chain
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_reference_chain_scene_to_material() {
    let conn = build_test_index();

    // Scene references Material
    let out_refs = query::query_refs(
        &conn,
        "Assets/Scenes/Game.unity",
        query::RefDirection::Out,
        "ALL",
    )
    .unwrap();

    let mat_ref = out_refs.iter().find(|r| r.to_path.contains("Enemy.mat"));
    assert!(mat_ref.is_some(), "Scene should reference Enemy.mat");

    // Material is referenced by Scene
    let in_refs = query::query_refs(
        &conn,
        "Assets/Materials/Enemy.mat",
        query::RefDirection::In,
        "ALL",
    )
    .unwrap();

    let scene_ref = in_refs.iter().find(|r| r.from_path.contains("Game.unity"));
    assert!(
        scene_ref.is_some(),
        "Enemy.mat should be referenced by Game.unity"
    );
}

#[test]
fn test_prefab_references_script() {
    let conn = build_test_index();

    // Player.prefab has a MonoBehaviour with script_guid pointing to PlayerController.cs
    let script_guid: Option<String> = conn
        .query_row(
            "SELECT script_guid FROM yaml_objects
         WHERE object_type = 'MonoBehaviour' AND script_guid IS NOT NULL
         LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();

    assert!(
        script_guid.is_some(),
        "should find MonoBehaviour with script_guid"
    );

    // The script file should exist with that GUID
    let guid = script_guid.unwrap();
    let file_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM files WHERE lower(guid) = lower(?1)",
            params![&guid],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        file_exists, 1,
        "script file with GUID {} should exist",
        guid
    );
}
