# Contributing

Thanks for your interest in contributing! This document covers the basics.

## Prerequisites

- Rust toolchain (stable, ≥ 1.75)
- `cargo` build system

```bash
rustc --version  # ensure stable toolchain is installed
```

## Getting Started

```bash
git clone <repo-url>
cd unityassetdb
cargo build
cargo test
```

All 32 tests should pass. If any fail, please open an issue.

## Development Workflow

1. Fork the repo and create a feature branch
2. Make your changes
3. Ensure `cargo test` passes
4. Ensure `cargo clippy` has no errors
5. Submit a pull request

### Code Style

- Follow `rustfmt` defaults: `cargo fmt`
- Address all `cargo clippy` warnings
- Keep public API documented with `///` doc comments

### Project Structure

| Directory | Responsibility |
|-----------|---------------|
| `src/model/` | Data types and enums |
| `src/discovery/` | File system scanning |
| `src/extract/` | Parsers (Unity YAML, C#, meta) |
| `src/resolve/` | Entity graph construction |
| `src/materialize/` | VFS projection |
| `src/db/` | SQLite schema |
| `src/query/` | Query implementations |
| `src/cli/` | CLI command handlers |
| `src/server/` | Web server + REST API |
| `tests/` | Integration and unit tests |

### Adding a New File Type

1. Add the extension to `FileKind::from_extension()` in `src/model/file.rs`
2. If it's Unity YAML, add it to `is_unity_yaml()` and `AssetKind::from_file_kind()`
3. Add a test in `tests/integration_test.rs`
4. Update `README.md` supported file types table

### Adding a New Query

1. Create a new file in `src/query/` (e.g. `src/query/tree.rs`)
2. Add the module to `src/query/mod.rs`
3. Add a CLI command in `src/cli/` and wire it in `src/cli/mod.rs`
4. Add a REST endpoint in `src/server/routes.rs`
5. Write tests

### Test Fixture

The test project at `tests/fixtures/TestProject/` contains:
- A scene with 3 GameObjects (including hierarchy)
- A material with shader/texture references
- A prefab with a MonoBehaviour
- A C# script (PlayerController)
- A shader file

When adding new file types or features, extend the fixture accordingly.

## Reporting Issues

When reporting a bug, please include:
- Rust version (`rustc --version`)
- OS and architecture
- The Unity project structure (or a minimal repro)
- The exact command and output
- The SQLite index file if possible

## Pull Request Checklist

- [ ] `cargo test` passes
- [ ] `cargo clippy` is clean
- [ ] `cargo fmt` applied
- [ ] New features have tests
- [ ] `README.md` updated if needed
