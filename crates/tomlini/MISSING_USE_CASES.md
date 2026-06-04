# Missing use cases — what we haven't covered

## 1. Programmatic creation (no existing document)

Every use case so far assumes parsing an existing file. What if the user
wants to build a config from scratch?

```rust
let doc = toml_fast::Document::new();
doc.edit()
    .insert_section("server")
        .with_block_comment(&["Server configuration"])
    .insert("server", "host", "\"0.0.0.0\"")
    .insert("server", "port", "8080")
    .insert_section("database")
    .insert("database", "url", "\"postgres://...\"")
    .commit()?;
```

**Need:** `Document::new()` constructor. Currently `FlatDoc` only comes from `parse()`.
A `new()` that creates an empty source string + empty spans + empty index.

## 2. Reading and inspection

Users need to READ values before editing. Currently no accessors:

```rust
let doc = toml_fast::parse(config)?;

// Does this key exist?
if doc.has("server.port") { ... }

// What's the current value?
let port: &str = doc.get("server.port")?;

// What keys are in this section?
let keys: Vec<&str> = doc.keys("server");

// What's the comment above this key?
let comment: Option<&str> = doc.comment("server.port");
```

**Need:** `has()`, `get()`, `keys()`, `comment()` read-accessors. These operate
on the span index — no tree construction needed. `get()` returns the raw
source text for the value span. `comment()` walks back from the key to find
any `#` lines immediately above it.

## 3. Value transformation

The user wants to change a value based on its current value:

```rust
// Bump port number
let current = doc.get("server.port")?;       // "8080"
let new_port: u16 = current.parse()? + 1;     // 8081
doc.edit().set("server.port", &new_port.to_string()).commit()?;
```

This works with `get()` + `set()`. No new API needed, just the read accessor.

## 4. Array editing

Arrays are a major gap. Currently no array-related operations:

```toml
# Before
allowed_hosts = ["localhost", "10.0.0.1"]

# After — append
allowed_hosts = ["localhost", "10.0.0.1", "10.0.0.2"]

# After — remove
allowed_hosts = ["localhost"]

# After — replace element
allowed_hosts = ["localhost", "10.0.1.1"]
```

```rust
doc.edit()
    .array_push("allowed_hosts", "\"10.0.0.2\"")
    .array_remove("allowed_hosts", 1)
    .array_set("allowed_hosts", 1, "\"10.0.1.1\"")
    .commit()?;
```

**Need:** `array_push()`, `array_remove(index)`, `array_set(index, value)`.
These splice byte ranges within `[...]` — more delicate than key-value edits
because commas and whitespace between elements must be respected.

## 5. Array-of-tables editing

```toml
# Before
[[backend]]
host = "10.0.0.1"
port = 9001

[[backend]]
host = "10.0.0.2"
port = 9001

# After — add entry
[[backend]]
host = "10.0.0.3"
port = 9001
```

```rust
doc.edit()
    .aot_push("backend", &[
        ("host", "\"10.0.0.3\""),
        ("port", "9001"),
    ])
    .commit()?;
```

**Need:** `aot_push()` to append to an array-of-tables. `aot_remove(index)`
and `aot_replace(index, pairs)` for positional ops. `aot_keys(index)` to
read keys from a specific entry.

## 6. Comment editing

Editing the comment ABOVE a key (not just adding comments on insertion):

```toml
# Before
# Old comment
port = 8080

# After
# Updated comment — explains why 9090
port = 9090
```

```rust
doc.edit()
    .set("server.port", "9090")
        .with_above_comment("Updated comment — explains why 9090")
    .commit()?;
```

Wait — this already works with `.set().with_above_comment()`. But does it
REPLACE the old comment or ADD to it? `.with_above_comment()` on `set` should
REPLACE the existing comment (the set replaces the entire prefix). On `insert`,
it adds a new comment.

**Need:** Clarify semantics. On `set`: `.with_above_comment("text")` replaces
the existing comment. On `insert`: adds a comment. `.without_comment()` removes
any existing comment on `set`.

## 7. Key ordering

Reordering keys within a section:

```toml
# Before
[server]
port = 8080
host = "localhost"
timeout = 30

# After — alphabetical
[server]
host = "localhost"
port = 8080
timeout = 30
```

```rust
doc.edit()
    .sort_keys("server")          // alphabetical
    .sort_keys_by("server", |a, b| ...)  // custom
    .commit()?;
```

**Need:** `sort_keys()` and `sort_keys_by()`. Internally: read all keys + values
+ comments, remove them, insert them in new order. Comments move with their keys.

## 8. Dotted key handling

Documents can express the same structure two ways:

```toml
# Style A: dotted keys
server.host = "localhost"
server.port = 8080

# Style B: table sections
[server]
host = "localhost"
port = 8080
```

**The editor should preserve whichever style is in the source.** When inserting
a key into a section that uses dotted keys, the new key should also use dotted
key syntax. When inserting into a `[section]`, use table-section syntax.

**Need:** The editor must detect the style from the first existing key in the
section and follow it. For new sections, default to table-section syntax.

## 9. Line ending conventions

Documents may use `\n` (LF) or `\r\n` (CRLF). The editor should detect and
preserve the convention.

**Need:** Detect line ending from the first newline in the source. Use the same
convention for all newlines in inserted content.

## 10. Value encoding

When the user passes a raw string as a value, they must encode it correctly:

```rust
// User must quote strings themselves
.insert("s", "k", "\"hello world\"")     // correct
.insert("s", "k", "hello world")         // wrong — no quotes

.insert("s", "k", "42")                  // integer — ok
.insert("s", "k", "0x2A")               // hex — ok
```

**Need:** A `Value` type or helper that auto-quotes strings. Or document clearly
that raw string values must be pre-formatted by the caller.

```rust
.insert("s", "k", toml_fast::value("hello world"))  // auto-quotes to "\"hello world\""
.insert("s", "k", toml_fast::value(42))              // formats integer
.insert("s", "k", toml_fast::value_hex(42))           // "0x2A"
```

## Priority for v1

| Feature | Priority | Reason |
|---|---|---|
| Read accessors (`get`, `has`, `keys`) | **v1** | Required for any conditional edit |
| `Document::new()` | **v1** | Required for programmatic creation |
| Value helpers (`value()`, `value_hex()`) | **v1** | Users shouldn't manually quote strings |
| Dotted key style preservation | v2 | Nice-to-have, can document as limitation |
| Array editing | v2 | Complex, many edge cases |
| AOT editing | v2 | Complex, positional awareness needed |
| Key ordering (`sort_keys`) | v2 | Composition of get + remove + insert |
| Line ending detection | v2 | CRLF is rare in practice |
