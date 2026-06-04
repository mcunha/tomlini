# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/mcunha/tomlini/releases/tag/tomlini_serde-v0.1.0) - 2026-06-04

### Other

- Enable publishing: remove publish=false from tomlini and tomlini_serde Cargo.toml, set publish=true in release-plz.toml
- Fix CI: cargo fmt 2024 edition, clippy fixes (default impls, sort_by_key, remove dead to_string, allow if_same_then_else)
- Bump to v0.1.0, add workspace package metadata (license, repository, documentation)
- Wire tomlini_serde into workspace with toml_datetime from crates.io. Fix tomlini alloc-only build (cfg-gated std::fmt, ToString import)
- Add real-world TOML benchmarks (8-realworld.rs) and INI benchmarks (7-ini.rs). Clean workspace to tomlini only.
- Drop toml_writer — unused dependency in tomlini_serde (serializer uses fmt::Write directly)
- Rename toml_fast → tomlini, toml_fast_serde → tomlini_serde. 175 tests pass.
