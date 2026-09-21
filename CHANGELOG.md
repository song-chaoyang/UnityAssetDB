# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-09-21

### Fixed

- **Index build no longer stalls in VFS materialization on large projects.**
  GUID cross-references are resolved once into an indexed temp table instead
  of an inline `lower(guid)` join that let the SQLite planner pick a
  files × files nested loop — on a 29k-asset project the materialize phase
  went from an unbounded multi-minute stall (zero rows written, bar frozen
  at 85%) to ~7s.
- VFS edge insertion uses SQLite-assigned rowids. Manually allocated ids
  combined with `INSERT OR IGNORE` let later rows collide with taken ids,
  silently dropping edges.
- Progress reporting no longer freezes at the materialize phase weight:
  every substep (directory tree, per-file entries, per-node entries,
  set-based edge queries) reports through the phase bar with exact totals.

### Added

- **PackageCache GUID resolution**: `Library/PackageCache` is scanned so
  built-in package scripts (URP, TMP, etc.) resolve instead of dangling.
- **grep / read content indexing**: bodies of text assets under 256KB
  (scenes, prefabs, C#, materials, shaders, asmdefs) are indexed at build
  time so `grep` hits real content.
- Live per-phase progress with weighted overall bar, throughput, entity /
  reference / byte counters, current-file line, and a per-phase timing
  breakdown on completion.
- All build writes now run inside one transaction with prepared statements
  and an in-memory file-id map (~5× faster full builds).

## [0.1.0] - 2026-08-21

### Added

- **Core indexing pipeline** with 7 stages: discovery → extract → resolve → materialize → finalize → publish
- **Unity YAML parser** supporting multi-document format (`!u!ClassID &Anchor ObjectType:`)
  - Object extraction: class ID, anchor, object type, local identifier, script GUID, name
  - Recursive reference scanner for `{guid, fileID, localIdentifierInFile}` objects
  - Reference kinds: `guid-file` and `local-file`
- **C# tree-sitter parser** extracting:
  - Declarations: class, struct, interface, enum, namespace, method, property, constructor, delegate, field, event
  - Mentions: identifier and qualified-name references with receiver context
- **Meta file parser** extracting GUID, importer type, and mainObjectFileID
- **File discovery** walking `Assets/`, `Packages/` (embedded, local-file, package-cache), `ProjectSettings/`
- **Entity graph resolution**:
  - GameObject → Component `contains` edges
  - Transform parent/child `parent_of` edges
  - Prefab instance → source prefab `instance_of` edges
  - MonoBehaviour → C# symbol `binds_to` edges (via script GUID)
  - Cross-asset `refs` edges via GUID resolution
- **VFS materialization** projecting entities into queryable virtual paths
- **SQLite schema** with 19 tables and comprehensive indexes
- **Query layer**:
  - `refs` — recursive CTE for incoming/outgoing reference graph with filter (File/Component/GameObject/ALL)
  - `ls` — hierarchical VFS listing with configurable depth
  - `glob` — glob-pattern matching on VFS paths
  - `grep` — content search within indexed entries
  - `read` — read VFS entry content, meta content, or link targets
- **CLI** (clap) with commands: `index build/sync/status`, `refs`, `ls`, `glob`, `grep`, `read`, `serve`
- **Web server** (axum) with REST API: `/api/status`, `/api/ls`, `/api/refs`, `/api/glob`, `/api/grep`, `/api/read`, `/api/graph`
- **Interactive Web UI** with vis.js graph visualization, click-to-explore navigation, direction toggle, search
- **Test suite** with 32 tests covering unit and integration scenarios
- **Test fixture** containing a minimal Unity project (scene, prefab, material, script, shader)
