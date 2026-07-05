#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EntityKind {
    GameObject,
    Component,
    Material,
    ScriptableObject,
    SceneSettings,
    SubAsset,
    ShaderProperty,
    ShaderPass,
    ShaderFallback,
    ShaderGraphProperties,
    ShaderGraphSettings,
    ShaderGraphNode,
    VisualEffectGraphProperties,
    VisualEffectGraphContext,
    VisualEffectGraphBlock,
}

impl EntityKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            EntityKind::GameObject => "gameobject",
            EntityKind::Component => "component",
            EntityKind::Material => "material",
            EntityKind::ScriptableObject => "scriptable_object",
            EntityKind::SceneSettings => "scene_settings",
            EntityKind::SubAsset => "subasset",
            EntityKind::ShaderProperty => "shader_property",
            EntityKind::ShaderPass => "shader_pass",
            EntityKind::ShaderFallback => "shader_fallback",
            EntityKind::ShaderGraphProperties => "shader_graph_properties",
            EntityKind::ShaderGraphSettings => "shader_graph_settings",
            EntityKind::ShaderGraphNode => "shader_graph_node",
            EntityKind::VisualEffectGraphProperties => "visual_effect_graph_properties",
            EntityKind::VisualEffectGraphContext => "visual_effect_graph_context",
            EntityKind::VisualEffectGraphBlock => "visual_effect_graph_block",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Entity {
    pub id: i64,
    pub asset_id: i64,
    pub yaml_object_id: Option<i64>,
    pub entity_kind: EntityKind,
    pub local_key: String,
    pub name: Option<String>,
    pub hierarchy_name: Option<String>,
    pub type_name: String,
    pub script_symbol_id: Option<i64>,
    pub parent_entity_id: Option<i64>,
    pub line_start: i64,
    pub line_end: i64,
}
