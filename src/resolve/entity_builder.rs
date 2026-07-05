use crate::extract::YamlObject;
use crate::model::{AssetKind, EntityKind};
use std::collections::HashMap;

pub struct BuiltEntity {
    pub asset_id: i64,
    pub yaml_object_id: Option<i64>,
    pub entity_kind: EntityKind,
    pub local_key: String,
    pub name: Option<String>,
    pub type_name: String,
    pub parent_local_key: Option<String>,
    pub line_start: i64,
    pub line_end: i64,
}

pub struct BuiltEdge {
    pub from_local_key: String,
    pub to_local_key: String,
    pub edge_kind: String,
    pub edge_subkind: Option<String>,
}

pub struct EntityGraphBuilder {
    pub entities: Vec<BuiltEntity>,
    pub edges: Vec<BuiltEdge>,
    pub local_key_to_entity_index: HashMap<(i64, String), usize>,
}

impl Default for EntityGraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl EntityGraphBuilder {
    pub fn new() -> Self {
        EntityGraphBuilder {
            entities: Vec::new(),
            edges: Vec::new(),
            local_key_to_entity_index: HashMap::new(),
        }
    }

    pub fn build_for_asset(
        &mut self,
        asset_id: i64,
        asset_kind: &AssetKind,
        yaml_objects: &[YamlObject],
    ) {
        match asset_kind {
            AssetKind::Scene | AssetKind::Prefab => {
                self.build_scene_or_prefab(asset_id, yaml_objects);
            }
            AssetKind::Material => {
                self.build_material(asset_id, yaml_objects);
            }
            AssetKind::ScriptableObject => {
                self.build_scriptable_object(asset_id, yaml_objects);
            }
            _ => {
                // For other asset types, create a single subasset entity
                for obj in yaml_objects {
                    let local_key = obj.local_identifier.clone();
                    let entity = BuiltEntity {
                        asset_id,
                        yaml_object_id: None,
                        entity_kind: EntityKind::SubAsset,
                        local_key: local_key.clone(),
                        name: obj.name.clone(),
                        type_name: obj.object_type.clone(),
                        parent_local_key: None,
                        line_start: obj.line_start,
                        line_end: obj.line_end,
                    };
                    let idx = self.entities.len();
                    self.local_key_to_entity_index
                        .insert((asset_id, local_key), idx);
                    self.entities.push(entity);
                }
            }
        }
    }

    fn build_scene_or_prefab(&mut self, asset_id: i64, yaml_objects: &[YamlObject]) {
        // Build a lookup: local_id -> yaml_object
        let _obj_by_local_id: HashMap<&str, &YamlObject> = yaml_objects
            .iter()
            .map(|o| (o.local_identifier.as_str(), o))
            .collect();

        // 1. Create GameObject entities
        for obj in yaml_objects {
            if obj.object_type == "GameObject" {
                let name = obj.name.clone();
                let entity = BuiltEntity {
                    asset_id,
                    yaml_object_id: None,
                    entity_kind: EntityKind::GameObject,
                    local_key: obj.local_identifier.clone(),
                    name,
                    type_name: "GameObject".to_string(),
                    parent_local_key: None,
                    line_start: obj.line_start,
                    line_end: obj.line_end,
                };
                let idx = self.entities.len();
                self.local_key_to_entity_index
                    .insert((asset_id, obj.local_identifier.clone()), idx);
                self.entities.push(entity);
            }
        }

        // 2. Create Component entities and link to GameObjects
        for obj in yaml_objects {
            // Components have m_GameObject field pointing to their parent
            let go_id = &obj.game_object_file_id;
            let _is_component = obj.object_type != "GameObject"
                && obj.object_type != "Transform"
                && obj.object_type != "RectTransform"
                || (go_id.is_some()
                    && obj.object_type != "Transform"
                    && obj.object_type != "RectTransform");

            // Actually, in Unity YAML, components reference their parent GameObject via m_GameObject
            if go_id.is_some() && obj.object_type != "GameObject" {
                let parent_key = go_id.as_ref().unwrap().clone();
                let entity_kind = EntityKind::Component;
                let type_name = obj.object_type.clone();

                let entity = BuiltEntity {
                    asset_id,
                    yaml_object_id: None,
                    entity_kind,
                    local_key: obj.local_identifier.clone(),
                    name: None,
                    type_name,
                    parent_local_key: Some(parent_key.clone()),
                    line_start: obj.line_start,
                    line_end: obj.line_end,
                };
                let idx = self.entities.len();
                self.local_key_to_entity_index
                    .insert((asset_id, obj.local_identifier.clone()), idx);
                self.entities.push(entity);

                // Create contains edge: GameObject -> Component
                self.edges.push(BuiltEdge {
                    from_local_key: parent_key,
                    to_local_key: obj.local_identifier.clone(),
                    edge_kind: "contains".to_string(),
                    edge_subkind: None,
                });
            }
        }

        // 3. Build Transform hierarchy (parent/child)
        for obj in yaml_objects {
            if obj.object_type == "Transform" || obj.object_type == "RectTransform" {
                let hierarchy = crate::resolve::hierarchy::extract_hierarchy(obj);
                let this_id = &obj.local_identifier;

                if let Some(parent_id) = &hierarchy.parent_transform_local_id {
                    // This Transform's parent is another Transform
                    self.edges.push(BuiltEdge {
                        from_local_key: this_id.clone(),
                        to_local_key: parent_id.clone(),
                        edge_kind: "parent_of".to_string(),
                        edge_subkind: None,
                    });
                }

                for child_id in &hierarchy.child_transform_local_ids {
                    self.edges.push(BuiltEdge {
                        from_local_key: child_id.clone(),
                        to_local_key: this_id.clone(),
                        edge_kind: "parent_of".to_string(),
                        edge_subkind: None,
                    });
                }
            }
        }

        // 4. Prefab instance edges
        for obj in yaml_objects {
            if obj.object_type == "PrefabInstance" {
                let hierarchy = crate::resolve::hierarchy::extract_hierarchy(obj);
                if let Some(source_guid) = &hierarchy.prefab_source_guid {
                    // This is a prefab instance referencing source prefab by GUID
                    // We'll resolve the cross-asset edge later
                    self.edges.push(BuiltEdge {
                        from_local_key: obj.local_identifier.clone(),
                        to_local_key: format!("guid:{}", source_guid),
                        edge_kind: "instance_of".to_string(),
                        edge_subkind: None,
                    });
                }
            }
        }
    }

    fn build_material(&mut self, asset_id: i64, yaml_objects: &[YamlObject]) {
        for obj in yaml_objects {
            if obj.object_type == "Material" {
                let entity = BuiltEntity {
                    asset_id,
                    yaml_object_id: None,
                    entity_kind: EntityKind::Material,
                    local_key: obj.local_identifier.clone(),
                    name: obj.name.clone(),
                    type_name: "Material".to_string(),
                    parent_local_key: None,
                    line_start: obj.line_start,
                    line_end: obj.line_end,
                };
                let idx = self.entities.len();
                self.local_key_to_entity_index
                    .insert((asset_id, obj.local_identifier.clone()), idx);
                self.entities.push(entity);
            }
        }
    }

    fn build_scriptable_object(&mut self, asset_id: i64, yaml_objects: &[YamlObject]) {
        for obj in yaml_objects {
            let entity_kind = if obj.object_type == "MonoBehaviour" {
                EntityKind::ScriptableObject
            } else {
                EntityKind::SubAsset
            };

            let entity = BuiltEntity {
                asset_id,
                yaml_object_id: None,
                entity_kind,
                local_key: obj.local_identifier.clone(),
                name: obj.name.clone(),
                type_name: obj.object_type.clone(),
                parent_local_key: None,
                line_start: obj.line_start,
                line_end: obj.line_end,
            };
            let idx = self.entities.len();
            self.local_key_to_entity_index
                .insert((asset_id, obj.local_identifier.clone()), idx);
            self.entities.push(entity);
        }
    }
}
