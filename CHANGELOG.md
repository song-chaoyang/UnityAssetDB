# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
