use crate::extract::YamlObject;
use serde_yaml::Value;

pub struct HierarchyInfo {
    pub game_object_file_id: Option<String>,
    pub parent_transform_local_id: Option<String>,
    pub child_transform_local_ids: Vec<String>,
    pub prefab_instance_local_id: Option<String>,
    pub prefab_source_guid: Option<String>,
    pub prefab_transform_parent_local_id: Option<String>,
    pub component_local_ids: Vec<String>,
}

pub fn extract_hierarchy(obj: &YamlObject) -> HierarchyInfo {
    let payload = &obj.payload;
    let go = get_str_field(payload, "m_GameObject");
    let parent = get_ref_field(payload, "m_TransformParent", "fileID");

    let mut children = Vec::new();
    if let Some(arr) = get_array(payload, "m_Children") {
        for child in arr {
            if let Some(fid) = get_ref_field_value(child, "fileID") {
                if fid != "0" && !fid.is_empty() {
                    children.push(fid);
                }
            }
        }
    }

    let mut component_ids = Vec::new();
    if let Some(arr) = get_array(payload, "m_Component") {
        for comp in arr {
            if let Some(component) = get_field_value(comp, "component") {
                if let Some(fid) = get_ref_field_value(component, "fileID") {
                    if fid != "0" && !fid.is_empty() {
                        component_ids.push(fid);
                    }
                }
            }
        }
    }

    let prefab_instance = get_ref_field(payload, "m_PrefabInstance", "fileID");
    let prefab_source = get_prefab_source_guid(payload);
    let prefab_parent = get_prefab_transform_parent(payload);

    HierarchyInfo {
        game_object_file_id: go,
        parent_transform_local_id: parent,
        child_transform_local_ids: children,
        prefab_instance_local_id: prefab_instance,
        prefab_source_guid: prefab_source,
        prefab_transform_parent_local_id: prefab_parent,
        component_local_ids: component_ids,
    }
}

fn get_str_field(yaml: &Value, field: &str) -> Option<String> {
    if let Value::Mapping(m) = yaml {
        match m.get(Value::String(field.into())) {
            Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
            Some(Value::Number(n)) => Some(n.to_string()),
            _ => None,
        }
    } else {
        None
    }
}

fn get_ref_field(yaml: &Value, field: &str, sub_field: &str) -> Option<String> {
    if let Value::Mapping(m) = yaml {
        if let Some(ref_yaml) = m.get(Value::String(field.into())) {
            return get_ref_field_value(ref_yaml, sub_field);
        }
    }
    None
}

fn get_ref_field_value(yaml: &Value, field: &str) -> Option<String> {
    if let Value::Mapping(m) = yaml {
        match m.get(Value::String(field.into())) {
            Some(Value::String(s)) if !s.is_empty() && s != "0" => Some(s.clone()),
            Some(Value::Number(n)) if n.as_i64().map(|i| i != 0).unwrap_or(false) => {
                Some(n.to_string())
            }
            _ => None,
        }
    } else {
        None
    }
}

fn get_field_value<'a>(yaml: &'a Value, field: &str) -> Option<&'a Value> {
    if let Value::Mapping(m) = yaml {
        m.get(Value::String(field.into()))
    } else {
        None
    }
}

fn get_array<'a>(yaml: &'a Value, field: &str) -> Option<&'a Vec<Value>> {
    if let Value::Mapping(m) = yaml {
        match m.get(Value::String(field.into())) {
            Some(Value::Sequence(arr)) => Some(arr),
            _ => None,
        }
    } else {
        None
    }
}

fn get_prefab_source_guid(yaml: &Value) -> Option<String> {
    if let Value::Mapping(m) = yaml {
        if let Some(Value::Mapping(ph)) = m.get(Value::String("m_SourcePrefab".into())) {
            if let Some(Value::String(g)) = ph.get(Value::String("guid".into())) {
                if !g.is_empty() {
                    return Some(g.clone());
                }
            }
        }
    }
    None
}

fn get_prefab_transform_parent(yaml: &Value) -> Option<String> {
    get_ref_field(yaml, "m_TransformParent", "fileID")
}
