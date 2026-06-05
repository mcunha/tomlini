# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/mcunha/tomlini/releases/tag/tomlini-v0.1.0) - 2026-06-05

### Other

- Enable publishing: remove publish=false from tomlini and tomlini_serde Cargo.toml, set publish=true in release-plz.toml
- Fix remaining clippy warnings: #[allow(needless_range_loop)] on find_matching_close and walk_array_elements, fix lost match arms in find_next_section_start
- Fix CI: cargo fmt 2024 edition, clippy fixes (default impls, sort_by_key, remove dead to_string, allow if_same_then_else)
- Add issue/PR templates, SECURITY.md, Cargo.toml keywords/categories
- Reframe crate docs: format preservation guarantees, movement & comment control. Show invariants we test for, not comparisons we avoid.
- Add 8 BringAlong coverage tests across all _bring methods. Fix COMMENTS_BELOW scan in apply_bring — pre-advancement was skipping comments.
- Update crate docs and CHANGELOG for v0.1.0: BringAlong, _bring methods, container editing, comment control, 12 doc sections, 290 tests
- Add move_key_bring, promote_key_bring, move_key_create_bring with shared apply_bring helper. BringAlong composition works across all four relocation ops.
- Remove CommentAnchor enum in favour of BringAlong bitflags. reorder_root_anchored → reorder_root_bring. Developers express intent: 'move this thing, bring these things with it.'
- Add BringAlong bitflag type: composable flags for what to carry when relocating (Nothing, CommentsAbove, CommentsBelow, EverythingAbove, EverythingBelow). CommentAnchor now delegates to BringAlong.
- Add concrete before/after examples to CommentAnchor docs showing Preceding vs Following output
- Add CommentAnchor enum + reorder_root_anchored: developers choose whether comments precede or follow each entry during reorder
- Add 6 runnable examples: basic-edit, batch-migrate, pont-fmt, containers, validation, ini-edit. Add Display+Error for ParseError.
- Add aot_remove primitive: remove entire [[entry]] from array-of-tables. 6 tests, EditorHandle, commit handler, doc-test.
- Fix public API inconsistencies: ParseError.pos u32, with_prefix/suffix param naming unified, proptest casts
- Bump to v0.1.0, add workspace package metadata (license, repository, documentation)
- Add build_index coverage tests: dotted table headers, AOT+dotted mixed documents
- Add garbage-value injection proptest: random strings via set(), asserts re-parse never panics on broken TOML
- Add proptest editor fuzzer: 2000 random op sequences on random TOML docs, asserts no panics + round-trip parse
- Add reorder_root edge cases + pont fmt pipeline integration tests. Fix reorder_root to handle dotted table headers via first segment
- Wire tomlini_serde into workspace with toml_datetime from crates.io. Fix tomlini alloc-only build (cfg-gated std::fmt, ToString import)
- Remove dead code: drop dead j+=1 before break, remove unused is_table field from all_starts tuple
- Expand negative editor tests: 47 cases covering all 21 OpKind error paths + 12 new gap-filling tests
- Push coverage to 84.2%: 22 new tests, fix reorder_root table support, fix commit re-parse for move/promote ops
- Add promote_key and move_key_create primitives with full EditorHandle support. 224 tests pass, zero doc warnings.
- Polish crate docs: reorder_root, container editing, core-only, broken links, code blocks
- Add reorder_root primitive for root-level entry reordering. Pont fmt use case.
- Add 30 negative-path editor tests. Coverage: 74.4% line (+3.6pp), editor.rs 62.4% (+7pp). Proptest found stale-index bug (noted).
- Add doc.keys() and doc.is_table() for pont fmt reorder integration
- Add Acknowledgments: toml-rs, toml_edit, toml_datetime, toml-test credit
- Update tagline: SAX TOML/INI parser and editor
- Add INI and validation mode docs to README and crate-level documentation
- Rename toml_fast → tomlini, toml_fast_serde → tomlini_serde. 175 tests pass.
