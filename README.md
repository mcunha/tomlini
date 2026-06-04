# tomlini — SAX TOML/INI parser and editor

A zero-dependency, three-tier (`core`/`alloc`/`std`) TOML implementation
that parses into a flat span index instead of a DOM tree. Edits are
byte-range splices on the source string — no decor model, no footguns.

[![Rust](https://img.shields.io/badge/rust-1.85+-blue.svg)](https://rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)

## Features

- **18× faster parse** than `toml_edit` (3.3 µs vs 59.5 µs for a 94-line Cargo.toml)
- **2–3× faster edits** with batch commit (single index build, descending sort, one-pass span fixup)
- **No footguns** — every API path preserves formatting unless explicitly overridden
- **Three tiers**: `core` (zero alloc), `alloc` (full editing), `std` (error impls)
- **Serde bridge** via `tomlini_serde` for struct deserialization
- **Three-mode validation**: Lenient (accept everything), Relaxed (INI support), Strict (spec-compliant)
- **INI `;` comment support** out of the box
- **490 toml-test compliance** tests (272 decoder + 218 encoder)

## Quick start

```rust
let mut doc = tomlini::parse("[server]\nport = 8080\n")?;

// Read
if doc.has("server.port") {
    let val = doc.get_decoded("server.port")?;
}

// Edit — batch, no footguns
doc.edit()
    .set("server.port", "9090")
    .insert("server", "host", "\"0.0.0.0\"")
        .with_above_comment("Bind address")
    .commit()?;
```

## INI files with `;` comments

```rust
let ini = "; Server settings\n[server]\nhost = localhost\nport = 8080\n";
let mut doc = tomlini::parse(ini)?;

// INI-style `;` comments are parsed as comments, same as `#`
let errors = doc.validate(tomlini::ValidationMode::Relaxed);
assert!(errors.is_empty()); // relaxed mode accepts INI conventions

doc.edit()
    .set("server.port", "9090")
    .commit()?;
```

## Validation modes

```rust
let mut doc = tomlini::parse(config)?;

// Lenient — everything accepted, no errors
let errors = doc.validate(tomlini::ValidationMode::Lenient);
assert_eq!(errors.len(), 0);

// Relaxed — structural TOML rules + INI extensions
let errors = doc.validate(tomlini::ValidationMode::Relaxed);

// Strict — full TOML 1.1.0 spec compliance
let errors = doc.validate(tomlini::ValidationMode::Strict);
```

## Acknowledgments

This crate builds on the excellent work of the [toml-rs](https://github.com/toml-rs/toml) project:

- **Ed Page** and contributors for the `toml_edit`, `toml_parser`, and `toml_writer` crates
  whose architecture and spec knowledge informed this implementation
- **`toml_datetime`** by the toml-rs team — the datetime types and parser used by the
  serde bridge
- **`toml-test`** by the TOML community — the compliance suite that validates correctness

## License
MIT OR Apache-2.0
