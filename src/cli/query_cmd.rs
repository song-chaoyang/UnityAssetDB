use crate::db;
use crate::query;
use std::path::PathBuf;

pub fn run_refs(
    path: &str,
    project: Option<PathBuf>,
    direction: &str,
    filter: &str,
) -> anyhow::Result<()> {
    let conn = open_project_db(project)?;

    let dir = match direction.to_lowercase().as_str() {
        "out" => query::RefDirection::Out,
        _ => query::RefDirection::In,
    };

    let results = query::query_refs(&conn, path, dir, filter)?;

    if results.is_empty() {
        println!("No references found for: {}", path);
        return Ok(());
    }

    let dir_label = match &dir {
        query::RefDirection::In => "referenced by",
        query::RefDirection::Out => "references",
    };

    println!("📂 {} ({} {})", path, results.len(), dir_label);
    println!("{}", "─".repeat(80));

    for r in &results {
        match &dir {
            query::RefDirection::In => {
                println!("  ← {} [{}]", r.from_path, r.from_kind);
                if let Some(ref sub) = r.edge_subkind {
                    println!("    via {} ({})", r.edge_kind, sub);
                } else {
                    println!("    via {}", r.edge_kind);
                }
            }
            query::RefDirection::Out => {
                println!("  → {} [{}]", r.to_path, r.to_kind);
                if let Some(ref sub) = r.edge_subkind {
                    println!("    via {} ({})", r.edge_kind, sub);
                } else {
                    println!("    via {}", r.edge_kind);
                }
            }
        }
    }

    println!("\n{} reference(s) total", results.len());
    Ok(())
}

pub fn run_ls(path: &str, project: Option<PathBuf>, depth: usize) -> anyhow::Result<()> {
    let conn = open_project_db(project)?;
    let entries = query::query_ls(&conn, path, depth)?;

    if entries.is_empty() {
        println!("No entries found under: {}", path);
        return Ok(());
    }

    println!("📁 {} ({} entries)", path, entries.len());
    println!("{}", "─".repeat(80));

    for entry in &entries {
        let indent = "  ".repeat(
            entry
                .vfs_path
                .matches('/')
                .count()
                .saturating_sub(path.matches('/').count()),
        );
        let icon = match entry.entry_type.as_str() {
            "directory" => "📁",
            "file" => "📄",
            "node" => "🔷",
            "link" => "🔗",
            _ => "?",
        };
        let children = if entry.has_children { " +" } else { "" };
        println!(
            "{}{} {} [{}]{}",
            indent, icon, entry.display_name, entry.entry_kind, children
        );
    }

    Ok(())
}

pub fn run_glob(pattern: &str, project: Option<PathBuf>, entry_type: &str) -> anyhow::Result<()> {
    let conn = open_project_db(project)?;
    let results = query::query_glob(&conn, pattern, entry_type)?;

    if results.is_empty() {
        println!("No entries matching: {}", pattern);
        return Ok(());
    }

    println!("🔍 {} ({} matches)", pattern, results.len());
    println!("{}", "─".repeat(80));

    for r in &results {
        println!("  {} [{}] {}", r.vfs_path, r.entry_type, r.display_name);
    }

    Ok(())
}

pub fn run_grep(pattern: &str, project: Option<PathBuf>, path: Option<&str>) -> anyhow::Result<()> {
    let conn = open_project_db(project)?;
    let results = query::query_grep(&conn, pattern, path)?;

    if results.is_empty() {
        println!("No matches for: {}", pattern);
        return Ok(());
    }

    println!("🔎 {} ({} matches)", pattern, results.len());
    println!("{}", "─".repeat(80));

    for r in &results {
        println!("  {}:{} [{}]", r.vfs_path, r.line_number, r.entry_kind);
        println!("    {}", r.matched_line.trim());
    }

    Ok(())
}

pub fn run_read(path: &str, project: Option<PathBuf>) -> anyhow::Result<()> {
    let conn = open_project_db(project)?;
    let result = query::query_read(&conn, path)?;

    match result {
        Some(r) => {
            println!("📖 {} [{} / {}]", r.vfs_path, r.entry_type, r.entry_kind);
            println!("{}", "─".repeat(80));
            if let Some(content) = r.content {
                println!("{}", content);
            } else {
                println!("(no indexed content)");
            }
            if let Some(target) = r.target_vfs_path {
                println!("\n→ Link target: {}", target);
            }
        }
        None => {
            println!("Not found: {}", path);
        }
    }

    Ok(())
}

fn open_project_db(project: Option<PathBuf>) -> anyhow::Result<rusqlite::Connection> {
    let project_root = match project {
        Some(p) => p.canonicalize()?,
        None => std::env::current_dir()?,
    };

    let db_path = project_root.join(".unityassetdb").join("index.db");

    if !db_path.exists() {
        anyhow::bail!(
            "No index found at {}. Run `unityassetdb index build {}` first.",
            db_path.display(),
            project_root.display()
        );
    }

    let conn = db::open_db(&db_path)?;
    Ok(conn)
}
