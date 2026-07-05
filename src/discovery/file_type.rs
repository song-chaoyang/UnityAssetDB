use crate::model::FileKind;
use std::path::Path;

pub fn classify_file(rel_path: &str) -> FileKind {
    let normalized = rel_path.replace('\\', "/");

    if normalized == "Packages/manifest.json" {
        return FileKind::PackageManifest;
    }
    if normalized == "Packages/packages-lock.json" {
        return FileKind::PackageLock;
    }
    if normalized.starts_with("ProjectSettings/") {
        let ext = Path::new(&normalized)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let ext_lower = format!(".{}", ext.to_lowercase());
        if ext_lower == ".asset" || ext_lower == ".yaml" || ext_lower == ".yml" {
            return FileKind::ProjectSettings;
        }
        return FileKind::Other;
    }

    let ext = Path::new(&normalized)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    if ext.is_empty() {
        return FileKind::Other;
    }

    let ext_with_dot = format!(".{}", ext.to_lowercase());
    FileKind::from_extension(&ext_with_dot)
}
