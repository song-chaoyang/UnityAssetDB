use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FileKind {
    Scene,
    Prefab,
    CSharp,
    AsmDef,
    AsmRef,
    Meta,
    Asset,
    YamlAsset,
    Shader,
    ShaderInclude,
    Material,
    AudioMixer,
    AnimationClip,
    AnimatorController,
    AnimatorOverrideController,
    VisualEffect,
    SceneTemplate,
    SpriteAtlas,
    PhysicsMaterial,
    PackageManifest,
    PackageLock,
    ProjectSettings,
    Binary,
    Text,
    Other,
}

impl FileKind {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            ".unity" | ".scene" => FileKind::Scene,
            ".prefab" => FileKind::Prefab,
            ".cs" => FileKind::CSharp,
            ".asmdef" => FileKind::AsmDef,
            ".asmref" => FileKind::AsmRef,
            ".meta" => FileKind::Meta,
            ".asset" => FileKind::Asset,
            ".mat" => FileKind::Material,
            ".mixer" => FileKind::AudioMixer,
            ".anim" => FileKind::AnimationClip,
            ".controller" => FileKind::AnimatorController,
            ".overridecontroller" => FileKind::AnimatorOverrideController,
            ".vfx" => FileKind::VisualEffect,
            ".scenetemplate" => FileKind::SceneTemplate,
            ".spriteatlas" | ".spriteatlasv2" => FileKind::SpriteAtlas,
            ".physicmaterial" => FileKind::PhysicsMaterial,
            ".physicsmaterial2d" => FileKind::PhysicsMaterial,
            ".shader" => FileKind::Shader,
            ".hlsl" | ".cginc" | ".shaderinc" => FileKind::ShaderInclude,
            ".fbx" | ".dae" | ".obj" | ".blend" | ".3ds" | ".mb" | ".ma" => FileKind::Binary,
            ".png" | ".jpg" | ".jpeg" | ".tga" | ".psd" | ".tif" | ".tiff" | ".exr" | ".hdr"
            | ".bmp" | ".webp" => FileKind::Binary,
            ".wav" | ".mp3" | ".ogg" | ".aiff" | ".aif" | ".flac" => FileKind::Binary,
            _ => FileKind::Other,
        }
    }

    pub fn is_unity_yaml(&self) -> bool {
        matches!(
            self,
            FileKind::Scene
                | FileKind::Prefab
                | FileKind::Material
                | FileKind::YamlAsset
                | FileKind::Asset
                | FileKind::AudioMixer
                | FileKind::AnimationClip
                | FileKind::AnimatorController
                | FileKind::AnimatorOverrideController
                | FileKind::VisualEffect
                | FileKind::SceneTemplate
                | FileKind::SpriteAtlas
                | FileKind::PhysicsMaterial
        )
    }

    pub fn is_text(&self) -> bool {
        !matches!(self, FileKind::Binary)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            FileKind::Scene => "scene",
            FileKind::Prefab => "prefab",
            FileKind::CSharp => "csharp",
            FileKind::AsmDef => "asmdef",
            FileKind::AsmRef => "asmref",
            FileKind::Meta => "meta",
            FileKind::Asset => "asset",
            FileKind::YamlAsset => "yaml-asset",
            FileKind::Shader => "shader",
            FileKind::ShaderInclude => "shader-include",
            FileKind::Material => "material",
            FileKind::AudioMixer => "audio-mixer",
            FileKind::AnimationClip => "animation-clip",
            FileKind::AnimatorController => "animator-controller",
            FileKind::AnimatorOverrideController => "animator-override-controller",
            FileKind::VisualEffect => "visual-effect",
            FileKind::SceneTemplate => "scene-template",
            FileKind::SpriteAtlas => "sprite-atlas",
            FileKind::PhysicsMaterial => "physics-material",
            FileKind::PackageManifest => "package-manifest",
            FileKind::PackageLock => "package-lock",
            FileKind::ProjectSettings => "project-settings",
            FileKind::Binary => "binary",
            FileKind::Text => "text",
            FileKind::Other => "other",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiscoveredFile {
    pub project_rel_path: String,
    pub abs_path: String,
    pub kind: FileKind,
    pub size_bytes: u64,
    pub mtime_ms: i64,
    pub content_hash: String,
    pub guid: Option<String>,
    pub importer_type: Option<String>,
    pub meta_main_object_file_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct MetaInfo {
    pub guid: Option<String>,
    pub importer_type: Option<String>,
    pub meta_main_object_file_id: Option<i64>,
}

impl MetaInfo {
    pub fn parse(content: &str) -> Self {
        let guid = regex::Regex::new(r"(?m)^guid:\s*([0-9a-fA-F]+)\s*$")
            .ok()
            .and_then(|re| re.captures(content))
            .and_then(|c| c.get(1).map(|m| m.as_str().to_string()));

        let importer_type = regex::Regex::new(r"(?m)^([A-Za-z0-9_]+Importer):\s*$")
            .ok()
            .and_then(|re| re.captures(content))
            .and_then(|c| c.get(1).map(|m| m.as_str().to_string()));

        let meta_main_object_file_id =
            regex::Regex::new(r"(?m)^\s{2}mainObjectFileID:\s*(\d+)\s*$")
                .ok()
                .and_then(|re| re.captures(content))
                .and_then(|c| c.get(1).and_then(|m| m.as_str().parse().ok()));

        MetaInfo {
            guid,
            importer_type,
            meta_main_object_file_id,
        }
    }
}

pub fn meta_path_for(asset_path: &str) -> String {
    if asset_path.ends_with(".meta") {
        asset_path.to_string()
    } else {
        format!("{}.meta", asset_path)
    }
}

pub fn strip_meta(path: &str) -> &str {
    if path.ends_with(".meta") {
        &path[..path.len() - 5]
    } else {
        path
    }
}

pub fn is_meta(path: &str) -> bool {
    path.ends_with(".meta")
}

pub fn extension(path: &str) -> &str {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
}
