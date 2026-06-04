# Changelog

All notable changes to `tomlini` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] — 2026-06-04

### Added

#### Core parser
- Zero-dependency SAX (streaming) TOML parser with flat span index.
- INI `;` comment support — parsed as comments alongside TOML `#`.
- `FlatDoc` — parsed document holding source string + flat span index + optional key index.
- `Span` and `SpanKind` — classified byte ranges for every token in the source.
- `ParseError` — parse error with byte position.
- Feature-gated allocation: `alloc` for index-builder and accessors, core-only for span emission via `SpanSink` trait.
- 272/272 valid TOML decoder tests pass (toml-test compliance), 218 encoder tests pass.

#### Editor (batch mutation)
- 22 `OpKind` variants providing a complete editing surface:
  - **Scalar**: `Set`, `Insert`, `Remove`
  - **Section**: `InsertSection`, `ReplaceSection`, `ClearSection`, `RenameSection`
  - **Key**: `RenameKey`, `MoveKey`, `PromoteKey`, `MoveKeyCreate`
  - **Array**: `ArrayPush`, `ArraySet`, `ArrayInsert`, `ArrayRemove`
  - **AOT**: `AotPush`, `AotSet`, `AotRemove`
  - **Inline table**: `InlineSet`, `InlineInsert`, `InlineRemove`
  - **Reorder**: `ReorderRoot`
- `Editor` struct — accumulate operations, commit in one pass.
- `EditorHandle` — fluent API via `doc.edit().op1().op2().commit()?`.
- **`BringAlong` bitflags** — composable flags (`COMMENTS_ABOVE`, `COMMENTS_BELOW`, `EVERYTHING_ABOVE`, `EVERYTHING_BELOW`) controlling what adjacent text moves with a key or section during relocation. Combine with `|`.
- `reorder_root(&order)` / `reorder_root_bring(&order, bring)` — reorder root-level entries while preserving comments, whitespace, and formatting. Handles both scalars and table headers (including dotted names like `[profiles.dev]`).
- `move_key_bring(from, to, bring)` — move a key between sections with comment control.
- `promote_key(from)` / `promote_key_bring(from, bring)` — self-healing primitive: extract a key from a sub-table back to document root.
- `move_key_create(from, to)` / `move_key_create_bring(from, to, bring)` — like `move_key` but auto-creates the destination table.
- Comment-awareness: `with_prefix`, `with_above_comment`, `with_block_comment`, `with_suffix`.

#### Validation
- Three-mode validation: `Lenient` (accept all), `Relaxed` (TOML rules + INI extensions), `Strict` (full TOML 1.1.0 spec).
- Detects: duplicate keys, table conflicts, AOT ordering, bare key constraints, control characters.

#### Query API
- `doc.keys()` — list top-level entry names.
- `doc.has(path)` — check if a dotted path exists.
- `doc.get(path)` — retrieve raw source text of a value.
- `doc.get_decoded(path)` — get decoded value string (unescaped).
- `doc.is_table(path)` — distinguish scalars from tables.
- `doc.validate(mode)` — run validation.
- `Document::new()` — programmatic empty document construction.

#### Serde bridge (`tomlini_serde`)
- Serialize `Serialize` types to TOML via `to_string()`.
- Deserialize TOML into `Deserialize` types via `from_str()`.
- Supports: scalars, tables, arrays, inline tables, array-of-tables, datetimes (via `toml_datetime`).
- Feature-gated: `std` and `alloc` features chain through to `tomlini`.
#### Testing
- **290 tests**: 75 coverage, 12 edit, 47 negative editor, 103 positive editor, 11 footgun immunity, 8 proptest, 5 error proptest, 16 validation, 13 serde.
- **8 proptest fuzzers**: random document generation, span integrity, comment handling, editor operation sequences, garbage value injection, malformed input recovery.
- **Footgun immunity**: 11 tests proving format-preservation invariants that `toml_edit` cannot guarantee.
- **Pont fmt integration**: realistic pipeline tests (parse → promote → reorder → serialize).
- **Negative editor tests**: all error paths across all 22 OpKind variants with readable failure messages.
63:- 84.5% line coverage, 85.9% region coverage.

#### CI/CD
- `justfile` with recipes for build, test, coverage, benchmarks, lint, no_std, wasm, mutation testing, fuzzing, and git hook setup.
- `.github/workflows/ci.yml` — PR checks: clippy, fmt, test, zizmor, doc.
- `.github/workflows/nightly.yml` — nightly: 4-way sharded cargo-mutants, unbounded proptest fuzzing (parser, editor, garbage), CI security audit.
- Pre-commit hook: `just setup-git-hooks` → runs zizmor, fmt, clippy.
- `zizmor` CI security audit with config-file policies (not blanket ignores).

### Fixed
- Type confusion between `[table]` headers and `[array]` values in span classification.
- Stale index after successive removes and move/promote operations (commit now re-parses to guarantee correct index).
- `reorder_root` not handling table headers alongside scalars.
- `promote_key` inserting promoted scalars after `[table]` headers (TOML spec re-absorption).
- `EditorHandle` missing `reorder_root`, `promote_key`, `move_key_create`.
- `use std::fmt` unconditional in `lib.rs` (broke no_std builds).
- Missing `ToString` import for no_std in `validate.rs`.
- Dead code: `j += 1` before `break` in `build_index`, unused `is_table` field in reorder tuple.
- `get_decoded()` mangling values after certain edit operations.
- `has()` returning true after `remove()` on the same key.
- `set()` on INI documents returning `NotFound`.
- Stale-index panic on successive operations.
- `move_key` to root when no root entries exist — insert position now correct.
- Dotted table header handling in `reorder_root` (first-segment name matching `doc.keys()`).
### Documentation
- Crate-level docs: 12 sections covering quick start, features, key types, validation, reordering, container editing, comment control, core-only usage, INI support, footgun-free guarantees, and acknowledgments.
- 25 doc-test blocks with usage examples.
- 6 runnable examples: `basic-edit`, `batch-migrate`, `pont-fmt`, `containers`, `validation`, `ini-edit`.
- `README.md` for both `tomlini` and `tomlini_serde`.
- Zero rustdoc warnings.
- Design documents: `BATCH_EDITOR_DESIGN.md`, `DECODER_DESIGN.md`, `CONTAINER_EDITING.md`, `SERDE.md`, `POSITIONING.md`.

### Benchmarks
- Real-world TOML: `cargo.toml`, `pyproject.toml`, `cargo.lock`, `alacritty.toml`.
- Real-world INI: classic, Windows, Apache, Docker, large config.
- Pathological: 10k tiny KV, 1k sections, 10k array elements, 1k AOT entries, deep dotted nesting, 2k inline tables, comment-heavy INI, long key names.

---

[0.1.0]: https://github.com/user/tomlini/releases/tag/v0.1.0
