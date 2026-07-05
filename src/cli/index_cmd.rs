use crate::cli::IndexAction;
use crate::db;
use crate::discovery;
use crate::extract;
use crate::materialize::VfsBuilder;
use crate::model::{AssetKind, FileKind};
use crate::resolve::{script_binding, EntityGraphBuilder};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

// ═══════════════════════════════════════════════════════════════════
//  Weighted multi-bar progress — mirrors real cost distribution
//
//  Renders three live lines:
//    ⠸ [00:00:12] ████████████░░░░░░░░  48% 3/8 extract · ETA 08s
//    ⠹  ████████████████░░░  extract 31,422/54,120 · 2,615/s · 892K objs · 1.2M refs
//    📄 Assets/Scenes/MainMenu.unity
// ═══════════════════════════════════════════════════════════════════

/// (long name, short name, weight) — weights mirror real cost distribution.
const PHASES: &[(&str, &str, u64)] = &[
    ("Scanning project files", "scan", 5),
    ("Registering files", "register", 5),
    ("Extracting YAML + C#", "extract", 50),
    ("Building assets", "assets", 5),
    ("Resolving entity graph", "graph", 15),
    ("Binding scripts", "bind", 5),
    ("Materializing VFS", "vfs", 13),
    ("Publishing index", "publish", 2),
];

const TOTAL_WEIGHT: u64 = 100;
const CURRENT_FILE_MAX: usize = 58;

/// Live counters surfaced on the phase bar while indexing.
#[derive(Default)]
struct LiveStats {
    yaml_objects: u64,
    yaml_refs: u64,
    cs_decls: u64,
    cs_mentions: u64,
    entities: u64,
    bytes_read: u64,
}

struct IndexProgress {
    overall: ProgressBar,
    phase_bar: ProgressBar,
    file_bar: ProgressBar,
    phase_idx: usize,
    base: u64,
    weight: u64,
    phase_len: u64,
    phase_pos: u64,
    phase_start: Instant,
    phase_times: Vec<(&'static str, f64)>,
    stats: LiveStats,
}

/// 31,422 — thousands separators.
fn fmt_exact(n: u64) -> String {
    let s = n.to_string();
    let len = s.len();
    let mut out = String::with_capacity(len + len / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// 892K / 1.2M — compact magnitude.
fn fmt_short(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 100_000 {
        format!("{}K", n / 1_000)
    } else if n >= 10_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/// 2.3 GB / 812 MB — human-readable byte size.
fn fmt_bytes(n: u64) -> String {
    if n >= 1_073_741_824 {
        format!("{:.1} GB", n as f64 / 1_073_741_824.0)
    } else if n >= 1_048_576 {
        format!("{:.0} MB", n as f64 / 1_048_576.0)
    } else if n >= 1024 {
        format!("{:.0} KB", n as f64 / 1024.0)
    } else {
        format!("{} B", n)
    }
}

impl IndexProgress {
    fn new() -> Self {
        let multi = MultiProgress::new();

        let overall = multi.add(ProgressBar::new(TOTAL_WEIGHT));
        overall.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{elapsed_precise}] {wide_bar:.cyan/blue} {percent:>3}% {msg} · ETA {eta}",
            )
            .expect("valid progress template")
            .progress_chars("█▓░"),
        );

        let phase_bar = multi.add(ProgressBar::new(0));
        phase_bar.set_style(Self::phase_msg_style());

        let file_bar = multi.add(ProgressBar::new(0));
        file_bar.set_style(
            ProgressStyle::with_template("  📄 {msg}").expect("valid progress template"),
        );
        file_bar.set_message("");

        Self {
            overall,
            phase_bar,
            file_bar,
            phase_idx: 0,
            base: 0,
            weight: 0,
            phase_len: 0,
            phase_pos: 0,
            phase_start: Instant::now(),
            phase_times: Vec::new(),
            stats: LiveStats::default(),
        }
    }

    fn phase_msg_style() -> ProgressStyle {
        ProgressStyle::with_template("  {spinner:.blue} {msg}").expect("valid progress template")
    }

    fn phase_bar_style() -> ProgressStyle {
        ProgressStyle::with_template("  {spinner:.blue} {wide_bar:.white/blue} {msg}")
            .expect("valid progress template")
            .progress_chars("█░")
    }

    fn phase(&mut self, idx: usize) {
        // record the phase that just ended (self.phase_idx still points to it)
        if idx > 0 && self.phase_times.len() < idx {
            self.phase_times.push((
                PHASES[self.phase_idx].1,
                self.phase_start.elapsed().as_secs_f64(),
            ));
        }
        self.phase_idx = idx;
        self.base = PHASES[..idx].iter().map(|(_, _, w)| w).sum();
        self.weight = PHASES[idx].2;
        self.phase_len = 0;
        self.phase_pos = 0;
        self.phase_start = Instant::now();
        self.file_bar.set_message("");
        self.phase_bar.set_style(Self::phase_msg_style());
        self.render();
    }

    /// Set the item count for the current phase (enables per-item increments).
    fn set_len(&mut self, len: u64) {
        self.phase_len = len;
        if len > 0 {
            self.phase_bar.set_length(len);
            self.phase_bar.set_style(Self::phase_bar_style());
        }
        self.render();
    }

    /// Report sub-progress directly (used by discovery callback).
    fn set_items(&mut self, pos: u64, total: u64) {
        self.phase_len = total;
        self.phase_pos = pos.min(total);
        if total > 0 && self.phase_bar.length() == Some(0) {
            self.phase_bar.set_length(total);
            self.phase_bar.set_style(Self::phase_bar_style());
        }
        self.render();
    }

    /// Walking-phase feedback where the total is unknown: "found N files".
    fn set_walking_count(&mut self, count: u64) {
        let mut msg = format!("scan · {} files found", fmt_exact(count));
        let dt = self.phase_start.elapsed().as_secs_f64();
        if dt > 0.1 && count > 0 {
            msg.push_str(&format!(" · {}/s", fmt_short((count as f64 / dt) as u64)));
        }
        self.phase_bar.set_message(msg);
    }

    /// Show which file is currently being processed.
    fn set_current_file(&self, path: &str) {
        let shown = if path.len() > CURRENT_FILE_MAX {
            format!("…{}", &path[path.len() - CURRENT_FILE_MAX..])
        } else {
            path.to_string()
        };
        self.file_bar.set_message(shown);
    }

    /// Live counters — keep the phase bar informative without redrawing logic
    /// in every call site.
    fn stat_yaml(&mut self, objects: u64, refs: u64, bytes: u64) {
        self.stats.yaml_objects += objects;
        self.stats.yaml_refs += refs;
        self.stats.bytes_read += bytes;
        self.render();
    }

    fn stat_cs(&mut self, decls: u64, mentions: u64, bytes: u64) {
        self.stats.cs_decls += decls;
        self.stats.cs_mentions += mentions;
        self.stats.bytes_read += bytes;
        self.render();
    }

    fn stat_entities(&mut self, n: u64) {
        self.stats.entities += n;
        self.render();
    }

    fn inc(&mut self) {
        if self.phase_len > 0 {
            self.phase_pos = (self.phase_pos + 1).min(self.phase_len);
        }
        self.render();
    }

    fn finish_phase(&mut self) {
        self.phase_pos = self.phase_len;
        if self.phase_len > 0 {
            self.phase_bar.set_position(self.phase_len);
        }
        self.overall
            .set_position((self.base + self.weight).min(TOTAL_WEIGHT));
        self.render();
    }

    fn render(&self) {
        let (_, short, _) = PHASES[self.phase_idx];
        self.overall
            .set_message(format!("{}/{} {}", self.phase_idx + 1, PHASES.len(), short));

        let mut msg = if self.phase_len > 0 {
            let mut m = format!(
                "{} {}/{}",
                short,
                fmt_exact(self.phase_pos),
                fmt_exact(self.phase_len)
            );
            let dt = self.phase_start.elapsed().as_secs_f64();
            if dt > 0.1 && self.phase_pos > 0 {
                m.push_str(&format!(
                    " · {}/s",
                    fmt_short((self.phase_pos as f64 / dt) as u64)
                ));
            }
            m
        } else {
            short.to_string()
        };

        // live extraction / resolution stats
        let extras: Vec<String> = match self.phase_idx {
            2 => {
                let mut v = Vec::new();
                if self.stats.yaml_objects > 0 {
                    v.push(format!("{} objs", fmt_short(self.stats.yaml_objects)));
                }
                if self.stats.yaml_refs > 0 {
                    v.push(format!("{} refs", fmt_short(self.stats.yaml_refs)));
                }
                if self.stats.cs_decls > 0 {
                    v.push(format!("{} decls", fmt_short(self.stats.cs_decls)));
                }
                if self.stats.bytes_read > 0 {
                    v.push(fmt_bytes(self.stats.bytes_read));
                }
                v
            }
            4 => {
                let mut v = Vec::new();
                if self.stats.entities > 0 {
                    v.push(format!("{} entities", fmt_short(self.stats.entities)));
                }
                v
            }
            _ => Vec::new(),
        };
        if !extras.is_empty() {
            msg.push_str(&format!(" · {}", extras.join(" · ")));
        }

        if self.phase_len > 0 {
            self.phase_bar.set_position(self.phase_pos);
        }
        self.phase_bar.set_message(msg);

        // weighted overall position
        let pos = if self.phase_len > 0 {
            let frac = self.phase_pos as f64 / self.phase_len as f64;
            self.base + (self.weight as f64 * frac).round() as u64
        } else {
            self.base
        };
        self.overall.set_position(pos.min(self.base + self.weight));
    }

    fn finish(&mut self, elapsed_s: f64) {
        // record the final phase
        self.phase_times.push((
            PHASES[self.phase_idx].1,
            self.phase_start.elapsed().as_secs_f64(),
        ));
        // clear the transient sub-bars, keep only the overall line
        self.file_bar.finish_and_clear();
        self.phase_bar.finish_and_clear();
        self.overall
            .finish_with_message(format!("done in {elapsed_s:.1}s"));

        // per-phase timing breakdown
        let max_dur = self
            .phase_times
            .iter()
            .map(|(_, d)| *d)
            .fold(0.0_f64, f64::max);
        println!("\n  phase breakdown:");
        for (name, dur) in &self.phase_times {
            let bar_len = if max_dur > 0.0 {
                ((dur / max_dur) * 24.0).round() as usize
            } else {
                0
            };
            println!("    {:<10} {:>6.1}s  {}", name, dur, "█".repeat(bar_len));
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Command entry
// ═══════════════════════════════════════════════════════════════════

pub fn run_index_action(action: IndexAction) -> anyhow::Result<()> {
    match action {
        IndexAction::Build {
            project,
            output,
            packages,
        } => run_build(project, output, packages),
        IndexAction::Sync { project } => run_sync(project),
        IndexAction::Status { project } => run_status(project),
    }
}

fn default_db_path(project: &Path) -> PathBuf {
    project.join(".unityassetdb").join("index.db")
}

pub fn run_build_internal(project_root: &Path) -> anyhow::Result<()> {
    run_build(project_root.to_path_buf(), None, true)
}

fn run_build(
    project: PathBuf,
    output: Option<PathBuf>,
    include_packages: bool,
) -> anyhow::Result<()> {
    let started = Instant::now();
    let project_root = project.canonicalize()?;
    let db_path = output.unwrap_or_else(|| default_db_path(&project_root));

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Remove existing DB
    if db_path.exists() {
        std::fs::remove_file(&db_path)?;
    }

    tracing::info!("Indexing project: {}", project_root.display());

    let mut progress = IndexProgress::new();

    // ── Phase 1/8: Discovery ─────────────────────────────────────
    progress.phase(0);
    let discovery_result = {
        let p = &mut progress;
        discovery::discover_files_with_progress(
            &project_root,
            include_packages,
            &mut |done, total| match total {
                Some(t) => p.set_items(done, t),
                None => p.set_walking_count(done),
            },
        )
    };
    let total_files = discovery_result.files.len();
    progress.finish_phase();

    // ── Phase 2/8: Schema + register files ────────────────────────
    progress.phase(1);
    let conn = db::open_db(&db_path)?;
    db::init_schema(&conn)?;

    // Batch every write of the build into one transaction. Auto-commit
    // per statement would fsync the journal for each of the hundreds of
    // thousands of row inserts — a 10-50x slowdown on real projects.
    let tx = conn.unchecked_transaction()?;

    conn.execute(
        "INSERT INTO projects (id, project_path, schema_version, indexed_at)
         VALUES (1, ?1, 1, ?2)",
        params![
            project_root.to_string_lossy(),
            chrono::Utc::now().to_rfc3339()
        ],
    )?;
    conn.execute(
        "UPDATE projects SET indexed_at = datetime('now') WHERE id = 1",
        [],
    )?;

    progress.set_len(total_files as u64);
    {
        let mut insert_file = conn.prepare_cached(
            "INSERT INTO files (id, project_id, project_rel_path, abs_path, kind, guid,
                                size_bytes, mtime_ms, content_hash, importer_type)
             VALUES (NULL, 1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?;
        for file in &discovery_result.files {
            insert_file.execute(params![
                file.project_rel_path,
                file.abs_path,
                file.kind.as_str(),
                file.guid,
                file.size_bytes as i64,
                file.mtime_ms,
                file.content_hash,
                file.importer_type,
            ])?;
            progress.inc();
        }
    }

    // Resolve rel_path → file_id once, in memory. Later phases do tens of
    // thousands of lookups; per-row SELECTs re-parse SQL every time.
    let file_ids: HashMap<String, i64> = {
        let mut stmt =
            conn.prepare_cached("SELECT id, project_rel_path FROM files WHERE project_id = 1")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i64>(0)?))
        })?;
        rows.flatten().collect()
    };
    progress.finish_phase();

    // ── Phase 3/8: Extract YAML + C# ──────────────────────────────
    progress.phase(2);

    let yaml_files: Vec<_> = discovery_result
        .files
        .iter()
        .filter(|f| f.kind.is_unity_yaml())
        .collect();

    let cs_files: Vec<_> = discovery_result
        .files
        .iter()
        .filter(|f| f.kind == FileKind::CSharp)
        .collect();

    progress.set_len((yaml_files.len() + cs_files.len()) as u64);

    // Row-ID counters live in memory for the whole phase; querying
    // MAX(id) per file meant thousands of redundant index scans.
    let mut next_yaml_obj_id: i64 = conn.query_row(
        "SELECT COALESCE(MAX(id), 0) + 1 FROM yaml_objects",
        [],
        |r| r.get(0),
    )?;
    let mut next_ref_id: i64 = conn.query_row(
        "SELECT COALESCE(MAX(id), 0) + 1 FROM yaml_references",
        [],
        |r| r.get(0),
    )?;

    let mut insert_yaml_obj = conn.prepare_cached(
        "INSERT INTO yaml_objects
         (id, file_id, doc_index, unity_class_id, anchor, object_type,
          local_identifier, game_object_file_id, component_type_name,
          script_guid, script_file_id, name, line_start, line_end)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
    )?;
    let mut insert_yaml_ref = conn.prepare_cached(
        "INSERT INTO yaml_references
         (id, file_id, source_yaml_object_id, field_path,
          target_guid, target_file_id, target_local_id, ref_kind)
         VALUES (?1, ?2,
            (SELECT id FROM yaml_objects WHERE file_id = ?2 AND local_identifier = ?3 LIMIT 1),
            ?4, ?5, ?6, ?7, ?8)",
    )?;

    // Process YAML files
    for file in &yaml_files {
        progress.set_current_file(&file.project_rel_path);
        let content = match std::fs::read_to_string(&file.abs_path) {
            Ok(c) => c,
            Err(_) => {
                progress.inc();
                continue;
            }
        };

        let file_id: i64 = *file_ids.get(&file.project_rel_path).unwrap_or(&0);

        if let Some(result) = extract::extract_from_unity_yaml(&content) {
            progress.stat_yaml(
                result.objects.len() as u64,
                result.references.len() as u64,
                content.len() as u64,
            );

            for obj in &result.objects {
                insert_yaml_obj.execute(params![
                    next_yaml_obj_id,
                    file_id,
                    obj.doc_index as i64,
                    obj.unity_class_id,
                    obj.anchor,
                    obj.object_type,
                    obj.local_identifier,
                    obj.game_object_file_id,
                    obj.component_type_name,
                    obj.script_guid,
                    obj.script_file_id,
                    obj.name,
                    obj.line_start,
                    obj.line_end,
                ])?;
                next_yaml_obj_id += 1;
            }

            for r in &result.references {
                insert_yaml_ref.execute(params![
                    next_ref_id,
                    file_id,
                    r.source_local_identifier,
                    r.field_path,
                    r.target_guid,
                    r.target_file_id,
                    r.target_local_id,
                    r.ref_kind,
                ])?;
                next_ref_id += 1;
            }
        }
        progress.inc();
    }
    drop(insert_yaml_obj);
    drop(insert_yaml_ref);

    let mut next_decl_id: i64 = conn.query_row(
        "SELECT COALESCE(MAX(id), 0) + 1 FROM cs_declarations",
        [],
        |r| r.get(0),
    )?;
    let mut next_mention_id: i64 = conn.query_row(
        "SELECT COALESCE(MAX(id), 0) + 1 FROM cs_mentions",
        [],
        |r| r.get(0),
    )?;

    let mut insert_cs_decl = conn.prepare_cached(
        "INSERT INTO cs_declarations
         (id, file_id, decl_kind, simple_name, qualified_name, signature, line_start, line_end)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?;
    let mut insert_cs_mention = conn.prepare_cached(
        "INSERT INTO cs_mentions
         (id, file_id, mention_kind, text, receiver_text, containing_declaration_id, line_start, line_end)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?;

    // Process C# files
    for file in &cs_files {
        progress.set_current_file(&file.project_rel_path);
        let content = match std::fs::read_to_string(&file.abs_path) {
            Ok(c) => c,
            Err(_) => {
                progress.inc();
                continue;
            }
        };

        let file_id: i64 = *file_ids.get(&file.project_rel_path).unwrap_or(&0);

        if let Some(result) = extract::extract_from_csharp(&content) {
            progress.stat_cs(
                result.declarations.len() as u64,
                result.mentions.len() as u64,
                content.len() as u64,
            );

            // simple_name → first declaration id in this file; used to
            // bind mentions without a per-mention SELECT (millions of
            // lookups on real projects).
            let mut decl_ids: HashMap<&str, i64> = HashMap::new();

            for decl in &result.declarations {
                insert_cs_decl.execute(params![
                    next_decl_id,
                    file_id,
                    decl.decl_kind,
                    decl.simple_name,
                    decl.qualified_name,
                    decl.signature,
                    decl.line_start as i64,
                    decl.line_end as i64,
                ])?;
                decl_ids
                    .entry(decl.simple_name.as_str())
                    .or_insert(next_decl_id);
                next_decl_id += 1;
            }

            for mention in &result.mentions {
                let containing_id: Option<i64> = mention
                    .containing_declaration
                    .as_deref()
                    .and_then(|name| decl_ids.get(name).copied());

                insert_cs_mention.execute(params![
                    next_mention_id,
                    file_id,
                    mention.mention_kind,
                    mention.text,
                    mention.receiver_text,
                    containing_id,
                    mention.line_start as i64,
                    mention.line_end as i64,
                ])?;
                next_mention_id += 1;
            }
        }
        progress.inc();
    }
    drop(insert_cs_decl);
    drop(insert_cs_mention);
    progress.finish_phase();

    // ── Phase 4/8: Build assets ──────────────────────────────────
    progress.phase(3);

    let mut next_asset_id =
        conn.query_row("SELECT COALESCE(MAX(id), 0) + 1 FROM assets", [], |row| {
            row.get::<_, i64>(0)
        })?;

    let asset_files: Vec<_> = discovery_result
        .files
        .iter()
        .filter(|f| f.guid.is_some() && f.kind != FileKind::Meta)
        .collect();

    progress.set_len(asset_files.len() as u64);

    for file in &asset_files {
        let guid = file.guid.as_ref().unwrap();
        let file_id: i64 = conn.query_row(
            "SELECT id FROM files WHERE project_rel_path = ?1",
            params![&file.project_rel_path],
            |row| row.get(0),
        )?;

        let asset_kind = AssetKind::from_file_kind(&file.kind).unwrap_or(AssetKind::YamlAsset);

        let name = Path::new(&file.project_rel_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        conn.execute(
            "INSERT OR IGNORE INTO assets (id, project_id, file_id, asset_kind, guid, name, vfs_root_path)
             VALUES (?1, 1, ?2, ?3, ?4, ?5, ?6)",
            params![
                next_asset_id,
                file_id,
                asset_kind.as_str(),
                guid,
                name,
                file.project_rel_path
            ],
        )?;
        next_asset_id += 1;
        progress.inc();
    }
    progress.finish_phase();

    // ── Phase 5/8: Build entities ────────────────────────────────
    progress.phase(4);

    let mut next_entity_id =
        conn.query_row("SELECT COALESCE(MAX(id), 0) + 1 FROM entities", [], |row| {
            row.get::<_, i64>(0)
        })?;

    let assets: Vec<(i64, i64, String, String)> = {
        let mut stmt = conn.prepare(
            "SELECT id, file_id, asset_kind, vfs_root_path FROM assets WHERE project_id = 1",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?;
        let mut result = Vec::new();
        for v in rows.flatten() {
            result.push(v);
        }
        result
    };

    progress.set_len(assets.len() as u64);

    for (asset_id, file_id, asset_kind_str, vfs_root) in &assets {
        progress.set_current_file(vfs_root);
        let asset_kind = parse_asset_kind(asset_kind_str);

        // Load YAML objects for this file
        let yaml_objects: Vec<crate::extract::YamlObject> = {
            let mut stmt = conn.prepare(
                "SELECT doc_index, unity_class_id, anchor, object_type, local_identifier,
                        game_object_file_id, component_type_name, script_guid, script_file_id,
                        name, line_start, line_end
                 FROM yaml_objects WHERE file_id = ?1",
            )?;

            let rows: Vec<_> = stmt
                .query_map(params![file_id], |row| {
                    Ok(crate::extract::YamlObject {
                        doc_index: row.get::<_, i64>(0)? as usize,
                        unity_class_id: row.get(1)?,
                        anchor: row.get(2)?,
                        object_type: row.get(3)?,
                        local_identifier: row.get(4)?,
                        game_object_file_id: row.get(5)?,
                        component_type_name: row.get(6)?,
                        script_guid: row.get(7)?,
                        script_file_id: row.get(8)?,
                        name: row.get(9)?,
                        line_start: row.get(10)?,
                        line_end: row.get(11)?,
                        payload: serde_yaml::Value::Null,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();
            rows
        };

        let mut builder = EntityGraphBuilder::new();
        builder.build_for_asset(*asset_id, &asset_kind, &yaml_objects);
        progress.stat_entities(builder.entities.len() as u64);

        // Insert entities
        for entity in &builder.entities {
            let yaml_obj_id: Option<i64> = if entity.yaml_object_id.is_some() {
                entity.yaml_object_id
            } else {
                // Try to find yaml_object_id by local_key
                conn.query_row(
                    "SELECT id FROM yaml_objects WHERE file_id = ?1 AND local_identifier = ?2 LIMIT 1",
                    params![file_id, entity.local_key],
                    |row| row.get(0),
                )
                .ok()
            };

            conn.execute(
                "INSERT INTO entities
                 (id, asset_id, yaml_object_id, entity_kind, local_key, name,
                  type_name, parent_entity_id, line_start, line_end)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7,
                   (SELECT id FROM entities WHERE asset_id = ?2 AND local_key = ?8 LIMIT 1),
                   ?9, ?10)",
                params![
                    next_entity_id,
                    asset_id,
                    yaml_obj_id,
                    entity.entity_kind.as_str(),
                    entity.local_key,
                    entity.name,
                    entity.type_name,
                    entity.parent_local_key,
                    entity.line_start,
                    entity.line_end,
                ],
            )?;
            next_entity_id += 1;
        }

        // Insert edges
        let mut next_edge_id = conn.query_row(
            "SELECT COALESCE(MAX(id), 0) + 1 FROM entity_edges",
            [],
            |row| row.get::<_, i64>(0),
        )?;

        for edge in &builder.edges {
            // Resolve local keys to entity IDs
            let from_id: Option<i64> = conn
                .query_row(
                    "SELECT id FROM entities WHERE asset_id = ?1 AND local_key = ?2",
                    params![asset_id, edge.from_local_key],
                    |row| row.get(0),
                )
                .ok();

            let to_id: Option<i64> = if edge.to_local_key.starts_with("guid:") {
                None // Cross-asset edges resolved later
            } else {
                conn.query_row(
                    "SELECT id FROM entities WHERE asset_id = ?1 AND local_key = ?2",
                    params![asset_id, edge.to_local_key],
                    |row| row.get(0),
                )
                .ok()
            };

            if let (Some(from), Some(to)) = (from_id, to_id) {
                conn.execute(
                    "INSERT OR IGNORE INTO entity_edges (id, from_entity_id, to_entity_id, edge_kind, edge_subkind)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![next_edge_id, from, to, edge.edge_kind, edge.edge_subkind],
                )?;
                next_edge_id += 1;
            }
        }
        progress.inc();
    }
    progress.finish_phase();

    // ── Phase 6/8: Symbols + script binding ───────────────────────
    // NOTE: symbols must exist before binding (binding looks up class
    // symbols by file); binding before symbol creation always bound 0.
    progress.phase(5);
    build_symbols(&conn)?;
    let bound = script_binding::bind_scripts_to_symbols(&conn)?;
    progress.finish_phase();

    // ── Phase 7/8: VFS materialization ───────────────────────────
    progress.phase(6);
    let mut vfs_builder = VfsBuilder::new(&conn, 1);
    vfs_builder.build()?;
    progress.finish_phase();

    // ── Phase 8/8: Publish summary ───────────────────────────────
    progress.phase(7);
    conn.execute(
        "INSERT OR REPLACE INTO rebuild_summary
         (project_id, mode, discovered_file_count, diagnostic_count,
          completed_stages_json, published_index_path, created_at)
         VALUES (1, 'full', ?1, 0, '[]', ?2, datetime('now'))",
        params![total_files as i64, db_path.to_string_lossy()],
    )?;
    tx.commit()?;
    progress.finish_phase();

    let elapsed = started.elapsed().as_secs_f64();
    let throughput = if elapsed > 0.0 {
        total_files as f64 / elapsed
    } else {
        0.0
    };
    progress.finish(elapsed);

    println!(
        "✅ Index built in {:.1}s: {} ({} files, {} assets, {} scripts bound, {:.0} files/s)",
        elapsed,
        db_path.display(),
        total_files,
        asset_files.len(),
        bound,
        throughput
    );
    Ok(())
}

fn build_symbols(conn: &Connection) -> rusqlite::Result<()> {
    // Create symbols from cs_declarations
    let mut next_symbol_id =
        conn.query_row("SELECT COALESCE(MAX(id), 0) + 1 FROM symbols", [], |row| {
            row.get::<_, i64>(0)
        })?;

    let mut stmt = conn.prepare(
        "SELECT id, file_id, decl_kind, simple_name, qualified_name, line_start, line_end
         FROM cs_declarations ORDER BY file_id, id",
    )?;

    let declarations: Vec<(i64, i64, String, String, String, i64, i64)> = stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?, // declaration id
                row.get(1)?, // file_id
                row.get(2)?, // decl_kind
                row.get(3)?, // simple_name
                row.get(4)?, // qualified_name
                row.get(5)?, // line_start
                row.get(6)?, // line_end
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);

    for (decl_id, file_id, decl_kind, simple_name, qualified_name, line_start, line_end) in
        declarations
    {
        conn.execute(
            "INSERT INTO symbols
             (id, project_id, file_id, declaration_id, symbol_kind, simple_name,
              qualified_name, display_name, line_start, line_end)
             VALUES (?1, 1, ?2, ?3, ?4, ?5, ?6, ?5, ?7, ?8)",
            params![
                next_symbol_id,
                file_id,
                decl_id,
                &decl_kind,
                &simple_name,
                &qualified_name,
                line_start,
                line_end,
            ],
        )?;
        next_symbol_id += 1;
    }

    Ok(())
}

fn parse_asset_kind(s: &str) -> AssetKind {
    match s {
        "scene" => AssetKind::Scene,
        "prefab" => AssetKind::Prefab,
        "material" => AssetKind::Material,
        "script" => AssetKind::Script,
        "scriptable_object" => AssetKind::ScriptableObject,
        "yaml-asset" => AssetKind::YamlAsset,
        _ => AssetKind::YamlAsset,
    }
}

fn run_sync(project: PathBuf) -> anyhow::Result<()> {
    // For now, sync = rebuild
    tracing::warn!("Sync not yet implemented, falling back to full rebuild");
    run_build(project, None, true)
}

fn run_status(project: PathBuf) -> anyhow::Result<()> {
    let project_root = project.canonicalize()?;
    let db_path = default_db_path(&project_root);

    if !db_path.exists() {
        println!("❌ No index found for project: {}", project_root.display());
        println!(
            "   Run `unityassetdb index build {}` to create one.",
            project_root.display()
        );
        return Ok(());
    }

    let conn = db::open_db(&db_path)?;

    let file_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE project_id = 1",
        [],
        |row| row.get(0),
    )?;

    let yaml_obj_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM yaml_objects", [], |row| row.get(0))?;
    let yaml_ref_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM yaml_references", [], |row| row.get(0))?;
    let cs_decl_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM cs_declarations", [], |row| row.get(0))?;
    let asset_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM assets WHERE project_id = 1",
        [],
        |row| row.get(0),
    )?;
    let entity_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM entities", [], |row| row.get(0))?;
    let edge_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM entity_edges", [], |row| row.get(0))?;
    let vfs_entry_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM vfs_entries WHERE project_id = 1",
        [],
        |row| row.get(0),
    )?;
    let vfs_edge_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM vfs_edges", [], |row| row.get(0))?;

    let summary: Option<(String, i64, String)> = conn
        .query_row(
            "SELECT mode, discovered_file_count, published_index_path FROM rebuild_summary WHERE project_id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .ok();

    println!("📊 UnityAssetDB Index Status");
    println!("   Project:  {}", project_root.display());
    println!("   Database: {}", db_path.display());
    if let Some((mode, discovered, path)) = summary {
        println!("   Mode:     {}", mode);
        println!("   Indexed:  {} files discovered", discovered);
        println!("   DB Path:  {}", path);
    }
    println!();
    println!("   Files:          {}", file_count);
    println!("   YAML Objects:   {}", yaml_obj_count);
    println!("   YAML Refs:      {}", yaml_ref_count);
    println!("   C# Decls:       {}", cs_decl_count);
    println!("   Assets:         {}", asset_count);
    println!("   Entities:       {}", entity_count);
    println!("   Entity Edges:   {}", edge_count);
    println!("   VFS Entries:    {}", vfs_entry_count);
    println!("   VFS Edges:      {}", vfs_edge_count);

    Ok(())
}
