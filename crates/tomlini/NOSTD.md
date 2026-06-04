# `no_std` and `alloc` support for `toml_fast`

## Current dependency footprint

```
toml_fast
  └── (no dependencies beyond core/alloc)
```

Zero non-std deps already. The only barrier to `no_std` is that we
don't declare `#![no_std]` and we don't feature-gate `std`-only items.

## What needs `alloc` vs `std`

| Feature | Requires | Reason |
|---|---|---|
| `parse()` → `FlatDoc` | `alloc` | Returns `String` + `Vec<Span>` |
| `FlatDoc::set/insert/remove` | `alloc` | String mutation (`replace_range`, `insert_str`) |
| `FlatDoc::to_string()` | `alloc` | Returns `String` |
| `EditError` impl `Error` | `std` | `std::error::Error` trait |
| `EditError` impl `Display` | `core` | Already available |
| `SpanKind` | `core` | Plain enum |
| `Span` | `core` | Plain struct |

## Feature flag design

```toml
[features]
default = ["std"]
std = ["alloc"]
alloc = []
```

Following the pattern in `toml_parser`, `toml_writer`, `toml_datetime`:

```rust
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;
```

## API surface by feature level

### `core` only (no `alloc`, no `std`)

No `FlatDoc`, no `parse()`, no editing. The parser operates as a
callback-based span emitter:

```rust
/// Sink trait for receiving classified spans without allocation.
pub trait SpanSink {
    fn emit(&mut self, kind: SpanKind, start: u32, end: u32);
}

/// Parse into a caller-provided sink. No heap allocation.
pub fn parse_into(input: &str, sink: &mut impl SpanSink) -> Result<(), ParseError> {
    // Same parser loop, but calls sink.emit() instead of spans.push()
}
```

Use case: embedded systems classifying TOML for read-only inspection.

### `alloc` (no `std`)

Full `FlatDoc`, `parse()`, all edit operations. `EditError` implements
`Display` but not `Error`. Users on `alloc`-only targets (WASM, embedded
with allocator) get the complete editing experience.

```rust
// Everything works except std::error::Error
let mut doc = toml_fast::parse(config)?;
doc.set(&["server", "port"], "9090")?;
let output = doc.to_string(); // allocates String
```

### `std` (default)

Full API including `impl std::error::Error for EditError`, `impl
std::fmt::Display for FlatDoc` (already exists via `core::fmt`).

## What changes in the codebase

### 1. Add crate attributes

```rust
// lib.rs
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;
```

### 2. Gate `FlatDoc` and `parse()` on `alloc`

```rust
#[cfg(feature = "alloc")]
pub struct FlatDoc { ... }

#[cfg(feature = "alloc")]
pub fn parse(input: &str) -> Result<FlatDoc, ParseError> { ... }
```

### 3. Gate `EditError: Error` on `std`

```rust
#[cfg(feature = "std")]
impl std::error::Error for EditError {}
```

### 4. Add `core`-only parse_into

```rust
pub fn parse_into(input: &str, sink: &mut impl SpanSink) -> Result<(), ParseError> {
    // Same parser, emits via trait instead of Vec::push
}
```

The existing `parse()` becomes a thin wrapper:

```rust
#[cfg(feature = "alloc")]
pub fn parse(input: &str) -> Result<FlatDoc, ParseError> {
    struct VecSink { spans: Vec<Span> }
    impl SpanSink for VecSink {
        fn emit(&mut self, kind: SpanKind, start: u32, end: u32) {
            self.spans.push(Span { kind, start, end });
        }
    }
    let mut sink = VecSink { spans: Vec::new() };
    parse_into(input, &mut sink)?;
    Ok(FlatDoc { source: input.to_string(), spans: sink.spans })
}
```

But wait — `String` isn't in `core`. `input.to_string()` requires `alloc`.
And `format!` requires `alloc`. So `parse()` genuinely needs `alloc`.

The `parse_into` path avoids all allocation. The sink just receives spans.
The caller decides what to do with them.

### 5. Proptest and benchmarks stay `std`

Tests already require `std`. No changes needed.

## Target audience

| Target | Feature | Use case |
|---|---|---|
| Linux/macOS/Windows CLI | `std` | Full editing experience |
| WASM (browser) | `alloc` | Config editing in web apps |
| WASM (edge functions) | `alloc` | Lightweight config transforms |
| Embedded (RTOS with alloc) | `alloc` | Device configuration files |
| Embedded (bare metal) | `core` | Read-only TOML inspection |
| Bootloader | `core` | Parse config without heap |

## What's NOT possible in `core`-only

- No `String` — can't return a `FlatDoc`
- No edit operations — they mutate a `String`
- No key-path index — requires `Vec` and `String`
- The sink receives raw spans; structural interpretation is the caller's job

But the parser still classifies every byte correctly — which is enough
for read-only use cases like configuration validation in constrained
environments.

## Migration path for existing `toml` crate users

The `toml` crate (serde-focused) currently requires `std`. Users who
need `no_std` today must use `toml_parser` + `toml_writer` directly.
`toml_fast` with `alloc` would give them a higher-level editing API
than the raw parser pipeline, without requiring `std`.
