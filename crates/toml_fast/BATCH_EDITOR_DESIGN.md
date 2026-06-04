# Batch Editor for `toml_fast` — Design

## Goal

A translation layer between user intent ("set this, insert that, with a comment")
and fastdoc operations (splice byte ranges). The user never sees spans, decor
models, or index building. They express edits with rich formatting options.

## User API

```rust
let mut doc = toml_fast::parse(config_str)?;

doc.edit()
    // ---- Value updates ----
    .set("package.version", "2.0.0")          // replace value, keep comment
    .set("network.port", "9090")
        .with_suffix(" # production")         // update inline comment
    .set("network.bind", "\"0.0.0.0\"")       // keep existing comment

    // ---- Insertions ----
    .insert("network", "timeout", "30")       // append, copy indent, no comment
    .insert("network", "retries", "3")
        .with_prefix("\n")                    // blank line before this key
    .insert("database", "host", "localhost")
        .with_above_comment("Production database host")  // sugar: adds "# comment\n"
    .insert("database", "name", "myapp")
        .with_block_comment(&[
            "Database connection settings",
            "Do not change without DBA approval",
        ])                                    // multi-line comment block

    // ---- Removals ----
    .remove("logging.level")                  // removes key + its comment
    .remove("dev-dependencies")               // removes entire table + header

    .commit()?;
```

### Key paths

Paths use dot-notation: `"network.port"` maps to `&["network", "port"]`.
A single segment `"port"` is the root table.
Quoted segments are NOT supported in the dot form — use the array form
for keys containing dots: `&["network", "ip.addr"]`.

### Formatting primitives

Each op carries optional `prefix` and `suffix` strings, applied at
the byte level during splicing:

```
PREFIX + {key = value} + SUFFIX
```

- **`set`**: `prefix` and `suffix` default to the EXISTING prefix/suffix
  (preserves formatting). Explicitly setting them overrides.

- **`insert`**: `prefix` defaults to the previous key's whitespace indent
  (no comment). `suffix` defaults to `"\n"`.

**Sugar methods:**

| Method | What it does |
|---|---|
| `.with_prefix(s)` | Set the raw prefix string |
| `.with_suffix(s)` | Set the raw suffix string |
| `.with_above_comment(text)` | `with_prefix("{indent}# {text}\n")` |
| `.with_block_comment(lines)` | `with_prefix("{indent}# {line1}\n{indent}# {line2}\n...")` |
| `.with_blank_line_above()` | `with_prefix("\n")` |
| `.with_no_suffix()` | Remove trailing newline |

Where `{indent}` is the calculated indentation for this key (from neighbor
or parent table context).

### Sugar: heavy config editing

For files with extensive block comments (Apache/squid style):

```rust
doc.edit()
    // A section with a full header block
    .insert_section("cache", &[
        "# ═══════════════════════════════════════════",
        "# Cache settings",
        "# ═══════════════════════════════════════════",
        "# Adjust these based on available RAM.",
        "# See docs/caching.md for details.",
    ])
    .insert("cache", "max_size", "1GB")
        .with_above_comment("Maximum cache size (supports K, M, G suffix)")
    .insert("cache", "ttl", "3600")
        .with_above_comment("Time-to-live in seconds")
    .commit()?;
```

## Internal Architecture

### Op representation

```rust
struct Op {
    kind: OpKind,
    path: Vec<String>,
    value: String,            // for set/insert
    prefix: Option<String>,   // None = use default
    suffix: Option<String>,   // None = use default
}

enum OpKind { Set, Insert, Remove, InsertSection }
```

### Commit algorithm

```
1. Build key-path index       (one walk of spans, ~25 µs)
2. Resolve each op:
   a. Find the target key in the index
   b. Compute default prefix/suffix if None
   c. Determine byte range to splice
3. Sort ops by byte position descending
4. Apply each splice to source string  (no span fixup needed — descending order)
5. Build new FlatDoc from final source (re-parse, ~3 µs)
```

### Default formatting rules

**For `set` (existing key):**
- `prefix`: unchanged (keep whatever is before the key on its line)
- `suffix`: unchanged (keep inline comment, trailing whitespace)

**For `insert` (new key):**
- Walk back from last key in the table to find its line start
- Scan forward through whitespace only to get indent
- `prefix`: the indent string (spaces/tabs only, no comment text)
- `suffix`: `"\n"`

**For `insert_section` (new table):**
- `prefix`: `"\n"` (blank line separator)
- Section header prefix: the comment block
- First key prefix: indent from the comments

### Resolving neighbor context

When an op doesn't provide explicit formatting, the editor discovers it
from adjacent content:

```
Previous entry in same scope ──→ indent level, newline style
Parent table header         ──→ section separator width
Document conventions        ──→ comment style (# vs ##, spacer lines)
```

For `insert`, the editor looks at the LAST entry in the target table.
For `insert_section`, it looks at the previous table's decor.

### Collapsing redundant ops

Before applying, collapse ops on the same key:
- `set(key, "a").set(key, "b")` → `set(key, "b")`
- `set(key, "a").remove(key)` → `remove(key)`
- `remove(key).insert(scope, key, "a")` → keep both (different semantics)
- `insert(scope, key, "a").set([scope, key], "b")` → `insert(scope, key, "b")`

## Edge cases

### Key doesn't exist on `set`

Returns an error on `commit()`. The error includes the failing path.

### Table doesn't exist on `insert`

Returns an error. The user must `insert_section` first, or we could
auto-create the table with `insert` (with a flag like `.create_table_if_missing()`).

### Removing the last key from a table

The table header remains (empty `[table]`). To remove the header too,
use `remove("table")`.

### Array-of-tables

`insert` appends to the LAST `[[entry]]`'s key list. For inserting
into a specific entry, we'd need entry indexing (future work).

### Dotted keys in source

The editor treats `a.b = 1` as path `["a", "b"]`. It does NOT create
an implicit table `[a]`. If the user needs that, they use `insert_section`.

## Performance model

| Phase | Cost (94-line doc) | Notes |
|---|---|---|
| Op accumulation | ~0 µs | Just pushing to a Vec |
| Index build | ~25 µs | Once, shared across all ops |
| Op resolution | ~1 µs/op | Lookup in index |
| Sorting | ~0 µs | O(n log n) for <100 ops |
| Source splicing | ~1 µs/op | memmove of trailing bytes |
| Re-parse | ~3 µs | Build final FlatDoc |
| **Total (~5 ops)** | ~35 µs | vs toml_edit: ~450 µs (13×) |

## What's NOT in scope

- **Arbitrary key ordering**: keys are always APPENDED, not inserted at specific positions. For insert-at-top, the user must rebuild the table.
- **Inline table editing**: no `set("inline.key")` within an inline table value.
- **Array element editing**: no `set("arr[2]")` for array indices.
- **Comment preservation on `remove`**: removing a key always removes its associated comments. No orphaned comments.
- **Array-of-tables positional insert**: always appends to the end.
