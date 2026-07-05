use regex::Regex;

#[derive(Debug, Clone)]
pub struct YamlObject {
    pub doc_index: usize,
    pub unity_class_id: i64,
    pub anchor: Option<String>,
    pub object_type: String,
    pub local_identifier: String,
    pub game_object_file_id: Option<String>,
    pub component_type_name: Option<String>,
    pub script_guid: Option<String>,
    pub script_file_id: Option<String>,
    pub name: Option<String>,
    pub line_start: i64,
    pub line_end: i64,
    pub payload: serde_yaml::Value,
}

#[derive(Debug, Clone)]
pub struct YamlReference {
    pub source_local_identifier: String,
    pub field_path: String,
    pub target_guid: Option<String>,
    pub target_file_id: Option<String>,
    pub target_local_id: Option<String>,
    pub ref_kind: String,
}

pub struct YamlExtractionResult {
    pub objects: Vec<YamlObject>,
    pub references: Vec<YamlReference>,
}

/// Parse Unity YAML content. Unity YAML files have this structure:
/// ```text
/// %YAML 1.1
/// %TAG !u! tag:unity3d.com,2011:
/// --- !u!21 &2100000
/// Material:
///   m_Name: Enemy
///   m_Shader: {fileID: 4800000, guid: abc123, type: 3}
/// --- !u!4 &400000
/// Transform:
///   ...
/// ```
/// Note the object type (`Material:`, `Transform:`, ...) is on its OWN line, directly below the
/// `--- !u!<classId> &<anchor>` marker — Unity's Force Text serializer never puts it on the same
/// line as the marker. A two-line header regex is required to split documents correctly.
pub fn extract_from_unity_yaml(content: &str) -> Option<YamlExtractionResult> {
    if !content.starts_with("%YAML") && !content.contains("--- !u!") {
        return None;
    }

    // Split into documents by the two-line "--- !u!<classId> &<anchor>" + "<TypeName>:" header.
    let doc_pattern = Regex::new(r"(?m)^--- !u!(\d+) &(\d+)\r?\n(\w+):[ \t]*\r?\n").ok()?;
    let mut objects = Vec::new();
    let mut references = Vec::new();

    // Find all document starts
    let matches: Vec<_> = doc_pattern.find_iter(content).collect();
    if matches.is_empty() {
        return None;
    }

    for (doc_index, m) in matches.iter().enumerate() {
        let caps = doc_pattern.captures(m.as_str()).unwrap();
        let class_id: i64 = caps
            .get(1)
            .and_then(|c| c.as_str().parse().ok())
            .unwrap_or(0);
        let anchor = caps.get(2).map(|a| a.as_str().to_string());
        let object_type = caps
            .get(3)
            .map(|t| t.as_str().to_string())
            .unwrap_or_default();

        let doc_start = m.end();
        let doc_end = if doc_index + 1 < matches.len() {
            matches[doc_index + 1].start()
        } else {
            content.len()
        };

        let yaml_body = &content[doc_start..doc_end];
        let line_start = content[..m.start()].lines().count() as i64 + 1;

        // Parse the YAML body
        let parsed: serde_yaml::Value = match serde_yaml::from_str(yaml_body) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let (go_id, script_guid, script_file_id, name) = extract_object_metadata(&parsed);

        let local_identifier = anchor.clone().unwrap_or_default();

        let obj = YamlObject {
            doc_index,
            unity_class_id: class_id,
            anchor: anchor.clone(),
            object_type: object_type.clone(),
            local_identifier: local_identifier.clone(),
            game_object_file_id: go_id,
            component_type_name: if object_type == "MonoBehaviour" {
                Some("MonoBehaviour".to_string())
            } else {
                None
            },
            script_guid: script_guid.clone(),
            script_file_id,
            name,
            line_start,
            line_end: line_start + yaml_body.lines().count() as i64,
            payload: parsed.clone(),
        };

        collect_references(&parsed, &local_identifier, "", &mut references);
        objects.push(obj);
    }

    Some(YamlExtractionResult {
        objects,
        references,
    })
}

fn extract_object_metadata(
    value: &serde_yaml::Value,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    let map = match value {
        serde_yaml::Value::Mapping(m) => m,
        _ => return (None, None, None, None),
    };

    let go_id = map
        .get(serde_yaml::Value::String("m_GameObject".into()))
        .and_then(|v| get_field(v, "fileID"));

    let script = map.get(serde_yaml::Value::String("m_Script".into()));
    let script_guid = script.and_then(|v| get_field(v, "guid"));
    let script_file_id = script.and_then(|v| get_field(v, "fileID"));

    let name = map
        .get(serde_yaml::Value::String("m_Name".into()))
        .and_then(|v| {
            if let serde_yaml::Value::String(s) = v {
                if !s.is_empty() {
                    Some(s.clone())
                } else {
                    None
                }
            } else {
                None
            }
        });

    (go_id, script_guid, script_file_id, name)
}

fn get_field(value: &serde_yaml::Value, field: &str) -> Option<String> {
    if let serde_yaml::Value::Mapping(m) = value {
        m.get(serde_yaml::Value::String(field.into()))
            .and_then(|v| match v {
                serde_yaml::Value::String(s) if !s.is_empty() => Some(s.clone()),
                serde_yaml::Value::Number(n) => Some(n.to_string()),
                _ => None,
            })
    } else {
        None
    }
}

fn collect_references(
    value: &serde_yaml::Value,
    local_id: &str,
    path: &str,
    refs: &mut Vec<YamlReference>,
) {
    match value {
        serde_yaml::Value::Mapping(m) => {
            if is_reference_node(value) {
                let guid = get_field(value, "guid");
                let file_id = get_field(value, "fileID");
                let local_identifier = get_field(value, "localIdentifierInFile");

                if guid.is_some() || (file_id.is_some() && file_id.as_deref() != Some("0")) {
                    let ref_kind = if guid.is_some() {
                        "guid-file"
                    } else {
                        "local-file"
                    };
                    refs.push(YamlReference {
                        source_local_identifier: local_id.to_string(),
                        field_path: path.to_string(),
                        target_guid: guid,
                        target_file_id: file_id,
                        target_local_id: local_identifier,
                        ref_kind: ref_kind.to_string(),
                    });
                    return;
                }
            }

            for (k, v) in m {
                let key_str = match k {
                    serde_yaml::Value::String(s) => s.clone(),
                    serde_yaml::Value::Number(n) => n.to_string(),
                    _ => continue,
                };
                let new_path = if path.is_empty() {
                    key_str
                } else {
                    format!("{}.{}", path, key_str)
                };
                collect_references(v, local_id, &new_path, refs);
            }
        }
        serde_yaml::Value::Sequence(arr) => {
            for (i, v) in arr.iter().enumerate() {
                let new_path = format!("{}[{}]", path, i);
                collect_references(v, local_id, &new_path, refs);
            }
        }
        _ => {}
    }
}

fn is_reference_node(value: &serde_yaml::Value) -> bool {
    if let serde_yaml::Value::Mapping(m) = value {
        m.contains_key(serde_yaml::Value::String("fileID".into()))
            || m.contains_key(serde_yaml::Value::String("guid".into()))
            || m.contains_key(serde_yaml::Value::String("localIdentifierInFile".into()))
    } else {
        false
    }
}
