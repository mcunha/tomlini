# Table insertion in the batch editor

## The operation: `.insert_section("name")`

Creates a `[name]` table header in the document at the correct position
with the correct spacing. Subsequent `.insert("name", key, val)` calls
populate it.

## Behavior by scenario

### 1. Table already exists

```toml
# Before
[server]
port = 8080

[database]
host = "localhost"
```

```rust
doc.edit()
    .insert("server", "timeout", "30")  // server exists → appends key
    .commit()?;
```

`insert_section` is NOT needed. The table already has a header.
`insert("server", ...)` just appends keys to the existing section.

### 2. New table at end of document

```toml
# Before
[server]
port = 8080

# After
[server]
port = 8080

[cache]
max_size = "1GB"
```

```rust
doc.edit()
    .insert_section("cache")              // creates [cache] header
        .with_above_comment("Cache settings")
    .insert("cache", "max_size", "\"1GB\"")
    .commit()?;
```

**Internal behavior:**
1. Find the last byte of the last table's content (after `port = 8080\n`)
2. After that position, insert `\n[cache]\n` (or `[cache]\n` if no spacing convention)
3. The `\n` prefix is discovered from the paragraph spacing between previous sections
4. Then `insert("cache", "max_size", ...)` appends normally

### 3. New nested table

```toml
# Before
[network]
bind = "0.0.0.0"

# After
[network]
bind = "0.0.0.0"

[network.tls]
enabled = true
cert = "/etc/cert.pem"
```

```rust
doc.edit()
    .insert_section("network.tls")        // creates [network.tls]
    .insert("network.tls", "enabled", "true")
    .insert("network.tls", "cert", "\"/etc/cert.pem\"")
    .commit()?;
```

`[network]` already exists. Only `[network.tls]` is created.

### 4. Implicit parent table creation (dotted path, parent missing)

```toml
# Before
# (empty document)

# After
[alpha]
[alpha.beta]
key = "value"
```

```rust
doc.edit()
    .insert_section("alpha.beta")  // creates [alpha] then [alpha.beta]
    .insert("alpha.beta", "key", "\"value\"")
    .commit()?;
```

**Edge case**: does this auto-create `[alpha]` as well? Two options:
- **A (explicit)**: Only creates the exact header specified. User must call
  `insert_section("alpha")` first if they want the parent too.
- **B (implicit)**: Auto-creates missing parents so the path is valid.

Recommend **A (explicit)** — avoids surprise implicit tables, matches user intent.
They asked for a specific section, they get that section. If they also need
the parent, they chain `.insert_section("alpha").insert_section("alpha.beta")`.

### 5. Section with comment block

```toml
# After

# ═══════════════════════
# Database settings
# ═══════════════════════
# Connection pooling configuration.
# See docs/database.md for tuning.
[database]
host = "db.example.com"
port = 5432
```

```rust
doc.edit()
    .insert_section("database")
        .with_block_comment(&[
            "═══════════════════════",
            "Database settings",
            "═══════════════════════",
            "Connection pooling configuration.",
            "See docs/database.md for tuning.",
        ])
    .insert("database", "host", "\"db.example.com\"")
        .with_above_comment("Primary database host")
    .insert("database", "port", "5432")
    .commit()?;
```

### 6. Inserting between existing sections

```toml
# Before
[server]
port = 8080

[logging]
level = "info"

# After
[server]
port = 8080

[database]        ← inserted here (after server, before logging)
host = "localhost"

[logging]
level = "info"
```

This requires positional awareness: insert after `[server]`, before `[logging]`.
The batch editor currently only supports APPEND (end of document).
For insert-between, we'd need `.insert_section_after("server", "database")`
or `.insert_section_before("logging", "database")`.

**Recommend deferring positional insert to v2.** Appending is the common case.

## Spacing convention discovery

When creating a new section header, the editor looks at the spacing between
existing sections to decide the new section's prefix (blank lines above it):

```
[server]           ← section 1
port = 8080

[logging]          ← section 2, preceded by "\n\n" (blank line)
level = "info"
```

New section inserted at end:
```
[logging]          ← last section
level = "info"
                   ← end of document
[cache]            ← new section, copy the "\n\n" pattern from logging
max_size = "1GB"
```

The algorithm: look at the last section header's line-start position. Count the
blank lines (consecutive `\n` characters) between the previous section's last
content and the header. Replicate that count for the new section.

## Implementation sketch

```rust
fn resolve_insert_section(
    &self,
    doc: &FlatDoc,
    index: &[(Vec<String>, Entry)],
    path: &[&str],
    comment: Option<&[&str]>,
) -> Result<(u32, String), EditError> {
    // 1. Check if table already exists
    let exists = index.iter().any(|(p, _)| path_eq(p, path));
    if exists {
        return Ok((0, String::new())); // no-op: table header already there
    }

    // 2. Find insertion point: after the last content in the document
    // (or after the last content in the parent table for nested paths)
    let last_span_end = doc.spans.last().map(|s| s.end).unwrap_or(0);

    // 3. Discover spacing convention
    let spacing = discover_section_spacing(doc, index, path);

    // 4. Build header
    let header = path.join(".");
    let mut text = String::new();
    text.push_str(&spacing.before); // blank lines before section
    if let Some(lines) = comment {
        for line in lines {
            text.push_str(&format!("# {line}\n"));
        }
    }
    text.push_str(&format!("[{header}]\n"));

    Ok((last_span_end, text))
}
```

## Comparison with toml_edit

| Operation | toml_edit | toml_fast batch editor |
|---|---|---|
| Create table | `doc["name"] = table()` | `.insert_section("name")` |
| Add key to new table | `doc["name"]["key"] = value(x)` | `.insert("name", "key", "val")` |
| Section header comment | Must manually set table decor | `.with_block_comment(&[...])` |
| Implicit parent creation | Not explicit — happens via `[]` chain | Must be explicit (or auto-created in v2) |
| Section spacing | Manual blank lines in decor | Auto-discovers from neighbors |
| Positional insert | Not supported | Append-only in v1, positional in v2 |

## Known limitations (v1)

- **Append-only**: new sections always go at the end. No insert-between.
- **No implicit parent creation**: `insert_section("a.b")` only creates `[a.b]`,
  not `[a]`. The user must create `[a]` explicitly if needed.
- **No inline table creation**: sections are always `[header]` style, not `key = { ... }`.
- **Array-of-tables**: not supported yet. `[[aot]]` creation is a v2 feature.
