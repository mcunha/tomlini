# toml_fast

DOM-free TOML parser and editor. Zero dependencies, three-tier (`core`/`alloc`/`std`).

## Features

- **18× faster parse** than `toml_edit` (3.3 µs vs 59.5 µs for a 94-line Cargo.toml)
- **2–3× faster edits** with batch commit (single index build, descending sort, one-pass span fixup)
- **No footguns** — every API path preserves formatting unless explicitly overridden
- **Three tiers**: `core` (zero alloc), `alloc` (full editing), `std` (error impls)
- **Serde bridge** via `toml_fast_serde` for struct deserialization

## Quick start

```rust
let mut doc = toml_fast::parse("[server]\nport = 8080\n")?;

// Read
assert!(doc.has("server.port"));
let raw = doc.get("server.port").unwrap();         // "8080"
let val = doc.get_decoded("server.port").unwrap(); // "8080"

// Edit
doc.edit()
    .set("server.port", "9090")
    .insert("server", "host", "\"0.0.0.0\"")
        .with_above_comment("Bind address")
    .commit()?;
```

## Feature flags

| Feature | What you get |
|---|---|
| *(none)* | `parse_into()` — span emitter, zero alloc |
| `alloc`  | `FlatDoc`, `parse()`, full editing API |
| `std`    | (default) `std::error::Error` impls |

## License

MIT OR Apache-2.0
