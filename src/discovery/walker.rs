use crate::discovery::file_type::classify_file;
use crate::model::{DiscoveredFile, FileKind, MetaInfo};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use walkdir::WalkDir;

pub struct DiscoveryResult {
    pub files: Vec<DiscoveredFile>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: String,
    pub category: String,
    pub message: String,
    pub file_path: Option<String>,
}

/// Progress callback: `(files_done, files_total)` — `total` is `None`
/// during directory walking (unknown ahead of time) and `Some(n)` during
/// the per-file processing pass.
pub type ProgressFn<'a> = dyn FnMut(u64, Option<u64>) + 'a;

pub fn discover_files(project_root: &Path, include_packages: bool) -> DiscoveryResult {
    discover_files_with_progress(project_root, include_packages, &mut |_, _| {})
}

pub fn discover_files_with_progress(
    project_root: &Path,
    include_packages: bool,
    on_progress: &mut ProgressFn,
) -> DiscoveryResult {
    let mut files = Vec::new();
    let diagnostics = Vec::new();

    // Scan roots. Package sources mirror Unity's own resolution order:
    //   1. embedded packages   → Packages/<pkg>/
    //   2. registry packages   → Library/PackageCache/<pkg>@<version>/
    //      (needed to resolve script GUIDs for built-ins like TMP/URP)
    let mut scan_dirs: Vec<&str> = vec!["Assets", "ProjectSettings"];
    if include_packages {
        scan_dirs.extend(["Packages", "Library/PackageCache"]);
    }

    let mut path_to_meta: HashMap<String, MetaInfo> = HashMap::new();
    let mut raw_files: Vec<(String, String, FileKind, u64, i64)> = Vec::new();
    let mut walked = 0u64;

    for dir in &scan_dirs {
        let physical_dir = project_root.join(dir);
        if !physical_dir.exists() {
            continue;
        }

        for entry in WalkDir::new(&physical_dir)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }

            let abs_path = entry.path();
            let file_name = entry.file_name().to_string_lossy();

            if file_name.ends_with("~") {
                continue;
            }

            let rel_to_project = abs_path
                .strip_prefix(project_root)
                .unwrap_or(abs_path)
                .to_string_lossy()
                .replace('\\', "/");

            let metadata = entry.metadata().ok();
            let (size, mtime) = match &metadata {
                Some(m) => (
                    m.len(),
                    m.modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_millis() as i64)
                        .unwrap_or(0),
                ),
                None => (0, 0),
            };

            let kind = classify_file(&rel_to_project);

            if kind == FileKind::Meta {
                if let Ok(content) = std::fs::read_to_string(abs_path) {
                    let meta = MetaInfo::parse(&content);
                    let asset_path = rel_to_project.trim_end_matches(".meta").to_string();
                    path_to_meta.insert(asset_path, meta);
                }
            }

            let abs_str = abs_path.to_string_lossy().to_string();
            raw_files.push((rel_to_project, abs_str, kind, size, mtime));

            walked += 1;
            if walked.is_multiple_of(256) {
                on_progress(walked, None);
            }
        }
    }

    // Let unclassified files with known extensions get proper kind from meta owner
    let total = raw_files.len() as u64;
    for (i, (rel_path, abs_path, kind, size, mtime)) in raw_files.into_iter().enumerate() {
        if i.is_multiple_of(64) || i as u64 + 1 >= total {
            on_progress(i as u64 + 1, Some(total));
        }

        if kind == FileKind::Other {
            continue;
        }

        let (guid, importer_type, meta_main_object_file_id) = if kind == FileKind::Meta {
            // Meta files themselves don't carry the target's GUID —
            // only the asset file does
            (None, None, None)
        } else {
            match path_to_meta.get(&rel_path) {
                Some(m) => (
                    m.guid.clone(),
                    m.importer_type.clone(),
                    m.meta_main_object_file_id,
                ),
                None => (None, None, None),
            }
        };

        let content_hash = if kind != FileKind::Binary && kind.is_text() {
            compute_hash(&abs_path)
        } else {
            // For binary files, hash the file size + mtime as a quick fingerprint
            format!("{:x}:{}", size, mtime)
        };

        files.push(DiscoveredFile {
            project_rel_path: rel_path,
            abs_path,
            kind,
            size_bytes: size,
            mtime_ms: mtime,
            content_hash,
            guid,
            importer_type,
            meta_main_object_file_id,
        });
    }

    files.sort_by(|a, b| a.project_rel_path.cmp(&b.project_rel_path));

    DiscoveryResult { files, diagnostics }
}

fn compute_hash(path: &str) -> String {
    match std::fs::read(path) {
        Ok(data) => {
            let mut hasher = Sha256::new();
            hasher.update(&data);
            format!("{:x}", hasher.finalize())
        }
        Err(_) => String::new(),
    }
}
