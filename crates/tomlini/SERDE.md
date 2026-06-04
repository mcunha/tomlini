# Serde layer for `toml_fast` — design sketch

## Where it fits

```
toml_fast (this crate)          toml_fast_serde (separate crate)
──────────────────────          ──────────────────────────────
parse() → FlatDoc               Deserialize<'de> for FlatDocRef<'de>
  .source: String                 impl serde::Deserializer
  .spans: Vec<Span>               impl serde::Serializer (optional)
  .index: key→span mapping
edit operations                  serde + toml_fast + toml_datetime
```

The serde layer is a **separate crate** (`toml_fast_serde`). It adds
`serde` and `toml_datetime` as dependencies. `toml_fast` stays dependency-free.

## Deserialization: from spans to Rust types

The span index already has everything serde needs in order:

```
spans: [Comment, Newline, BareKey("host"), Equals, BasicString("\"localhost\""), Newline,
        Comment, Newline, BareKey("port"), Equals, Integer("8080"), Newline]

index: ["server", "host"] → { key_start: 2, value_idx: 4 }
       ["server", "port"] → { key_start: 7, value_idx: 10 }
```

A serde `Deserializer` walks the index for the current table scope,
yielding keys and dispatching values by `SpanKind`:

```rust
struct FlatDeserializer<'de> {
    doc: &'de FlatDoc,
    index: &'de [(Vec<String>, Entry)],
    table_path: Vec<String>,      // current table scope
    key_cursor: usize,            // position in the index
}

impl<'de> serde::Deserializer<'de> for FlatDeserializer<'de> {
    fn deserialize_struct(
        self, name: &str, fields: &[&str], visitor: V
    ) -> Result<V::Value, Error> {
        // 1. Collect all keys in current table scope from the index
        // 2. For each field the visitor asks for, look up the key
        // 3. Dispatch to the value deserializer based on SpanKind
    }
}
```

### Value type dispatch

| SpanKind | serde type |
|---|---|
| `BasicString`, `LiteralString`, `MlBasicString`, `MlLiteralString` | `deserialize_str` |
| `Integer` | `deserialize_i64` |
| `Float` | `deserialize_f64` |
| `Boolean` | `deserialize_bool` |
| `Datetime` | `deserialize_str` (or `toml_datetime::Datetime`) |
| `ArrayOpen` | `deserialize_seq` (walk spans until matching `ArrayClose`) |
| `InlineTableOpen` | `deserialize_map` (walk spans until matching `InlineTableClose`) |

### String decoding

The raw source byte range for a string span may contain escapes (`\n`, `\t`,
`\"`, `\\`, `\uXXXX`). The deserializer must decode these:

```rust
fn decode_string(src: &str) -> String {
    // Walk the span, process escape sequences
    // Reuse toml_parser's decoder or implement inline
}
```

Alternatively, lazy-decode only when serde asks for `deserialize_str` —
most values are integers/booleans and never need decoding.

### Array and inline table handling

These require depth tracking over the flat spans:

```rust
// For ArrayOpen at index i:
// Walk forward from i+1, tracking depth.
// Push elements until we hit ArrayClose at depth 0.
// Each element becomes a serde SeqAccess item.

// For InlineTableOpen at index i:
// Walk forward from i+1, tracking depth.
// Collect key=value pairs until InlineTableClose at depth 0.
// Each pair becomes a serde MapAccess entry.
```

### What's easy

- Flat key-value structs: `struct Server { host: String, port: u16 }`
- Nested tables: `[server.database]` → `server: { database: { ... } }`
- Arrays of primitives: `[1, 2, 3]` → `Vec<i64>`
- Option types: missing keys → `None`
- Defaults via `#[serde(default)]`

### What's harder

- **Arrays of tables**: `[[bin]]` — requires grouping consecutive entries
- **Flatten**: `#[serde(flatten)]` — requires merging two table scopes
- **Untagged enums**: trying multiple deserializations
- **Dotted keys as implicit tables**: `a.b = 1` needs to be expanded into `{a: {b: 1}}`

## Serialization: from Rust types to TOML

Serialization is a write path, not a read path. It doesn't use the span index
at all. Options:

**A. Use `toml_writer` directly.** Write keys, values, tables using the
low-level writer API. Simple, no new code.

**B. Build a `FlatDoc` and return it.** Serialize into a `String`, parse
with `toml_fast`, return the `FlatDoc`. The user gets an editable document
that they can further modify.

**C. `toml_fast` serializer from scratch.** Write a `Serializer` impl that
produces TOML output directly. Adds complexity for marginal gain.

Recommend **B** for the common case: serialize into a String, then offer
it as an editable `FlatDoc`. The user gets the full editing API on the
serialized output.

```rust
#[derive(Serialize)]
struct Config {
    server: ServerConfig,
    database: DatabaseConfig,
}

let config = Config { ... };

// Serialize into an editable document
let doc: FlatDoc = toml_fast_serde::to_document(&config)?;

// Now the user can edit further
doc.set("server.port", "9090")?;
let final_output = doc.to_string();
```

## Performance expectations

Serde deserialization from a `FlatDoc` should be faster than from `toml::Value`
because:
- No DOM construction (the span index is already built)
- No `IndexMap` lookups (key paths are pre-resolved)
- No `Formatted` allocation (values are read from source byte ranges)
- String decoding only happens when serde actually asks for a `String` type

Estimate: ~5-10 µs for a 94-line cargo.toml struct deserialization
(vs ~60 µs for `toml::from_str` which builds the full `toml::Table` DOM).

## What's NOT in scope for v1

- `Serialize` impl — defer to `toml_writer` or build a String
- Array-of-tables deserialization — complex, positional grouping needed
- `#[serde(flatten)]` — requires cross-table merging
- Untagged enums — requires speculative deserialization
- Spanned values (`serde_spanned`) — possible but needs span tracking
