#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssetKind {
    Scene,
    Prefab,
    Material,
    Script,
    ScriptableObject,
    AudioMixer,
    AnimationClip,
    AnimatorController,
    AnimatorOverrideController,
    VisualEffect,
    SceneTemplate,
    SpriteAtlas,
    PhysicsMaterial,
    Shader,
    ShaderGraph,
    Texture,
    Model,
    Audio,
    YamlAsset,
    ProjectSettings,
}

impl AssetKind {
    pub fn from_file_kind(kind: &crate::model::FileKind) -> Option<Self> {
        match kind {
            crate::model::FileKind::Scene => Some(AssetKind::Scene),
            crate::model::FileKind::Prefab => Some(AssetKind::Prefab),
            crate::model::FileKind::CSharp => Some(AssetKind::Script),
            crate::model::FileKind::YamlAsset => Some(AssetKind::YamlAsset),
            crate::model::FileKind::Material => Some(AssetKind::Material),
            crate::model::FileKind::Asset => Some(AssetKind::ScriptableObject),
            crate::model::FileKind::AudioMixer => Some(AssetKind::AudioMixer),
            crate::model::FileKind::AnimationClip => Some(AssetKind::AnimationClip),
            crate::model::FileKind::AnimatorController => Some(AssetKind::AnimatorController),
            crate::model::FileKind::AnimatorOverrideController => {
                Some(AssetKind::AnimatorOverrideController)
            }
            crate::model::FileKind::VisualEffect => Some(AssetKind::VisualEffect),
            crate::model::FileKind::SceneTemplate => Some(AssetKind::SceneTemplate),
            crate::model::FileKind::SpriteAtlas => Some(AssetKind::SpriteAtlas),
            crate::model::FileKind::PhysicsMaterial => Some(AssetKind::PhysicsMaterial),
            crate::model::FileKind::Shader => Some(AssetKind::Shader),
            crate::model::FileKind::Binary => None,
            _ => None,
        }
    }

    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            ".unity" | ".scene" => AssetKind::Scene,
            ".prefab" => AssetKind::Prefab,
            ".cs" => AssetKind::Script,
            ".mat" => AssetKind::Material,
            ".mixer" => AssetKind::AudioMixer,
            ".anim" => AssetKind::AnimationClip,
            ".controller" => AssetKind::AnimatorController,
            ".overridecontroller" => AssetKind::AnimatorOverrideController,
            ".vfx" => AssetKind::VisualEffect,
            ".scenetemplate" => AssetKind::SceneTemplate,
            ".shader" => AssetKind::Shader,
            ".asset" => AssetKind::ScriptableObject,
            _ => AssetKind::YamlAsset,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            AssetKind::Scene => "scene",
            AssetKind::Prefab => "prefab",
            AssetKind::Material => "material",
            AssetKind::Script => "script",
            AssetKind::ScriptableObject => "scriptable_object",
            AssetKind::AudioMixer => "audio_mixer",
            AssetKind::AnimationClip => "animation_clip",
            AssetKind::AnimatorController => "animator_controller",
            AssetKind::AnimatorOverrideController => "animator_override_controller",
            AssetKind::VisualEffect => "visual_effect_graph",
            AssetKind::SceneTemplate => "scene_template",
            AssetKind::SpriteAtlas => "sprite_atlas",
            AssetKind::PhysicsMaterial => "physics_material",
            AssetKind::Shader => "shader_lab",
            AssetKind::ShaderGraph => "shader_graph",
            AssetKind::Texture => "texture",
            AssetKind::Model => "model",
            AssetKind::Audio => "audio",
            AssetKind::YamlAsset => "yaml-asset",
            AssetKind::ProjectSettings => "project_settings",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Asset {
    pub id: i64,
    pub file_id: i64,
    pub asset_kind: AssetKind,
    pub guid: String,
    pub name: String,
    pub vfs_root_path: String,
}
