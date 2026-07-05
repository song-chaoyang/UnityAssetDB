use unityassetdb::extract;

#[test]
fn test_unity_yaml_extraction() {
    let content = r#"%YAML 1.1
%TAG !u! tag:unity3d.com,2011:
--- !u!21 &2100000
Material:
  serializedVersion: 8
  m_ObjectHideFlags: 0
  m_Name: Enemy
  m_Shader: {fileID: 4800000, guid: b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8, type: 3}
  m_Texture: {fileID: 2800000, guid: c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0, type: 3}
"#;

    let result = extract::extract_from_unity_yaml(content);
    assert!(result.is_some(), "YAML extraction should succeed");
    let result = result.unwrap();
    println!("Objects: {}", result.objects.len());
    println!("References: {}", result.references.len());
    for obj in &result.objects {
        println!(
            "  Object: type={} local_id={} name={:?} script_guid={:?}",
            obj.object_type, obj.local_identifier, obj.name, obj.script_guid
        );
    }
    for r in &result.references {
        println!(
            "  Ref: field={} guid={:?} fileID={:?}",
            r.field_path, r.target_guid, r.target_file_id
        );
    }
    assert!(
        !result.objects.is_empty(),
        "Should have at least one object"
    );
    assert_eq!(result.objects[0].object_type, "Material");
    assert!(!result.references.is_empty(), "Should have references");
}
