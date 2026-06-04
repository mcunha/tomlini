# tomlini — next-generation TOML parser and editor

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
- **Serde bridge** via `toml_fast_serde` for struct deserialization
- **Three-mode validation**: Lenient (accept everything), Relaxed (INI support), Strict (spec-compliant)
- **INI `;` comment support** out of the box
- **490 toml-test compliance** tests (272 decoder + 218 encoder)

## Quick start

```rust
let mut doc = toml_fast::parse("[server]\nport = 8080\n")?;

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

## License

MIT OR Apache-2.0
