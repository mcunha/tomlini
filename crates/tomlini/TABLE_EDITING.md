# Editing Existing Tables — Use Cases and API

## Use case taxonomy

| Intent | Operation | Status |
|---|---|---|
| Add one key | `.insert("s", "k", "v")` | Defined |
| Set one value | `.set("s.k", "v")` | Defined |
| Remove one key | `.remove("s.k")` | Defined |
| Add several keys | `.insert("s", "k1", "v1").insert("s", "k2", "v2")` | Chaining |
| Set several values | `.set("s.k1", "v1").set("s.k2", "v2")` | Chaining |
| Remove several keys | `.remove("s.k1").remove("s.k2")` | Chaining |
| Replace section wholesale | **NEW** | Needs design |
| Clear section (keep header) | **NEW** | Needs design |
| Rename section | **NEW** | Needs design |
| Add key at specific position | **NEW** | Needs design |
| Bulk-set from map | **NEW** | Needs design |
| Conditional ops | v2 | Deferred |

---

## 1. Replace section wholesale

User wants to say: "This is the complete, canonical set of keys for `[database]`.
Make the file match."

```rust
doc.edit()
    .replace_section("database", &[
        ("host", "\"db.example.com\""),
        ("port", "5432"),
        ("name", "\"myapp\""),
        ("pool_size", "10"),
    ])
    .commit()?;
```

**Internal:**
1. Find all existing keys in `[database]` via the index
2. Sort by byte position descending
3. Remove each existing key's line (splice out)
4. Insert each new key from the list (all appended at the end of the section)
5. The section header and its comments are preserved

**Edge case: section doesn't exist.**
Two options:
- **A**: Error — the user must use `insert_section` first.
- **B**: Auto-create — `replace_section` is semantically "make this section contain these keys" including creating it.

Recommend **B** — `replace_section` is a strong intent: "I want this section to exist with these keys." If it doesn't exist, create it.

---

## 2. Clear a section

Remove all keys from a section but keep the header and its comments.

```rust
doc.edit()
    .clear_section("database")           // removes all keys in [database]
    .commit()?;

// After:
// # Database settings
// [database]           ← header + comment preserved
//                       ← empty
```

**Internal:**
1. Find all keys in the section via the index
2. Remove each key's line (same as individual `.remove()` calls)
3. Section header and its decor remain

---

## 3. Rename a section

```rust
doc.edit()
    .rename_section("database", "db")    // [database] → [db]
    .commit()?;
```

**Internal:**
1. Find the `[database]` header span
2. Replace the header text: `[database]` → `[db]`
3. Update all key paths referencing `["database", ...]` — but we don't need to
   since we re-parse after commit

**Edge cases:**
- Dotted header `[a.b]` → `[x.y]`: only renames the last component? Or the full path?
  `.rename_section("a.b", "x.y")` renames the full path.
- Target name already exists: error.
- Array-of-tables header `[[items]]`: not supported in v1.

---

## 4. Add key at specific position

Currently `insert` always appends. For positional insertion:

```rust
// Insert before an existing key
doc.edit()
    .insert_before("server", "port", "host", "\"0.0.0.0\"")
    .commit()?;

// Insert after an existing key
doc.edit()
    .insert_after("server", "host", "timeout", "30")
    .commit()?;

// Insert at the top of a section
doc.edit()
    .insert_first("server", "host", "\"0.0.0.0\"")
    .commit()?;
```

**Parameter semantics:** `insert_before(section, anchor_key, new_key, value)` —
"insert new_key before anchor_key in this section."

**Internal for `insert_before`:**
1. Find the anchor key's span
2. Walk back to the start of its line
3. Insert the new key-value pair at that position, with the same indentation
4. The anchor key's comment stays with the anchor key

**When to defer (v2):** positional insertion is nice-to-have. The batch editor
already supports "edit, then reorder manually" by removing and re-inserting.

---

## 5. Bulk-set from map

```rust
doc.edit()
    .set_all("server", &[
        ("host", "\"0.0.0.0\""),
        ("port", "9090"),
        ("timeout", "30"),
    ])
    .commit()?;
```

Sugar for chaining individual `.set()` calls. The difference: a single `.set_all()`
with 3 entries builds the index once and splices all values in one pass.

**Interaction with formatting**: each value's existing inline comment is preserved
(same as individual `.set()`). If you want to replace comments too, chain
`.with_suffix(...)` on the returned builder — but `.set_all()` returns a
batch-level builder, not a per-key builder. For per-key formatting control,
use individual `.set()` chains.

---

## 6. Compound operations

### "Move a key from one section to another"

```rust
doc.edit()
    .remove("old-section.moving-key")
    .insert("new-section", "moving-key", "\"value\"")
    .commit()?;
```

The `.remove()` and `.insert()` are sorted descending by byte position
at commit time, so they don't interfere with each other.

### "Promote a key to a new section"

```toml
# Before
[server]
db_host = "localhost"
db_port = 5432

# After
[server.database]
host = "localhost"
port = 5432
```

```rust
doc.edit()
    .remove("server.db_host")
    .remove("server.db_port")
    .insert_section("server.database")
        .with_above_comment("Extracted from server section")
    .insert("server.database", "host", "\"localhost\"")
    .insert("server.database", "port", "5432")
    .commit()?;
```

---

## API surface summary (v1)

```rust
doc.edit()
    // ---- Value mutations ----
    .set("s.k", "v")                        // existing key: replace value
    .set("s.k", "v").with_suffix(" # cmt")   // replace value + inline comment

    // ---- Key mutations ----
    .insert("s", "k", "v")                   // append key to table
    .insert("s", "k", "v").with_prefix("\n")  // with blank line before
    .insert("s", "k", "v").with_above_comment("text")  // with comment
    .remove("s.k")                            // remove key

    // ---- Section mutations ----
    .insert_section("s")                      // create new table header
    .insert_section("s").with_block_comment(&[...])
    .replace_section("s", &[(k1,v1),...])     // replace all keys in section
    .clear_section("s")                       // remove all keys, keep header
    .rename_section("s", "new")               // rename [s] → [new]

    // ---- Bulk operations ----
    .set_all("s", &[(k1,v1),...])            // set multiple values at once

    .commit()?;
```

v2 candidates: `insert_before`, `insert_after`, `insert_first`, conditional ops,
array-of-tables support.
