# Migration patterns for TOML documents

## The operations

All migrations are compositions of five primitives:

| Primitive | What it does |
|---|---|
| `.set("s.k", "v")` | Replace value (key stays) |
| `.insert("s", "k", "v")` | Add key to table (append) |
| `.remove("s.k")` | Remove key + its comment |
| `.rename_section("a", "b")` | Rename `[a]` → `[b]` |
| `.insert_section("s")` | Create new table header |

## Pattern 1: Key moves section

```toml
# Before
[server]
host = "0.0.0.0"
db_host = "localhost"    ← move this to [database]

[database]
port = 5432

# After
[server]
host = "0.0.0.0"

[database]
port = 5432
host = "localhost"       ← arrived here
```

```rust
doc.edit()
    .remove("server.db_host")
    .insert("database", "host", "\"localhost\"")
    .commit()?;
```

**What happens to the comment on `db_host`?** `.remove()` deletes the key's line
including any comment above it. The comment does NOT move to the new location
unless the user explicitly reads it and passes it to `.with_above_comment()`.

**Better: migrate with formatting.**

```rust
// 1. Read the source before editing
let original = doc.to_string();
let db_host_line = extract_line(&original, "db_host"); // helper

// 2. Parse any comment above the key
let comment = extract_above_comment(&original, "server.db_host");

doc.edit()
    .remove("server.db_host")
    .insert("database", "host", "\"localhost\"")
        .with_above_comment(&comment.unwrap_or_default())
    .commit()?;
```

The batch editor doesn't provide `extract_above_comment` — that's a read-time
operation on the source string. The user can do it themselves or we could
add read-accessors to `FlatDoc`.

## Pattern 2: Section promoted to dotted namespace

```toml
# Before
[cache]
max_size = "1GB"
ttl = 3600

# After
[performance.cache]
max_size = "1GB"
ttl = 3600
```

```rust
doc.edit()
    .rename_section("cache", "performance.cache")
    .commit()?;
```

The header text changes from `[cache]` to `[performance.cache]`. All keys
within the section are unchanged — their paths now resolve under the new name.

**Edge case: destination namespace already exists.**
If `[performance]` already exists, the rename just adds `[performance.cache]`
as a child. The parent `[performance]` is unaffected.

**Edge case: destination is an existing section.**
`rename_section("cache", "existing-name")` should error — can't overwrite
a section by renaming into it.

## Pattern 3: Section demoted (flattened)

```toml
# Before
[server.database]
host = "localhost"
port = 5432

# After
[database]
host = "localhost"
port = 5432
```

```rust
doc.edit()
    .rename_section("server.database", "database")
    .commit()?;
```

Same operation as promotion, just the path gets shorter. The implicit parent
`[server]` may become empty after the child is renamed away — `clear_section`
or `remove` the parent if needed.

## Pattern 4: Multiple keys promoted to a new section

```toml
# Before
[app]
name = "myapp"
db_host = "localhost"    ← these two
db_port = 5432           ← are becoming a section
log_level = "info"

# After
[app]
name = "myapp"
log_level = "info"

[app.database]
host = "localhost"
port = 5432
```

```rust
doc.edit()
    // 1. Create the new section
    .insert_section("app.database")
        .with_above_comment("Database connection")
    // 2. Move the keys (remove from old location, insert into new)
    .remove("app.db_host")
    .remove("app.db_port")
    .insert("app.database", "host", "\"localhost\"")
    .insert("app.database", "port", "5432")
    .commit()?;
```

**Better: migrate with rename.**

```rust
doc.edit()
    .insert_section("app.database")
        .with_above_comment("Database connection")
    .rename_key("app.db_host", "app.database.host")   // NEW primitive
    .rename_key("app.db_port", "app.database.port")   // moves + renames
    .commit()?;
```

A `rename_key` primitive would be valuable here. It's semantically "remove from
old path, insert at new path with the same value" but preserves formatting.

## Pattern 5: Merging two sections

```toml
# Before
[network]
bind = "0.0.0.0"

[tls]
enabled = true
cert = "/etc/cert.pem"

# After
[network]
bind = "0.0.0.0"
tls_enabled = true
tls_cert = "/etc/cert.pem"   ← section gone, keys moved
```

```rust
doc.edit()
    .remove("tls.enabled")
    .remove("tls.cert")
    .insert("network", "tls_enabled", "true")
    .insert("network", "tls_cert", "\"/etc/cert.pem\"")
    .remove_section("tls")       // remove the now-empty header
    .commit()?;
```

## Pattern 6: Splitting a section

The inverse of merging: take some keys from `[app]` and create `[app.database]`.

```rust
doc.edit()
    .insert_section("app.database")
        .with_above_comment("Extracted database config")
    .move_key("app.db_host", "app.database.host")    // NEW primitive
    .move_key("app.db_port", "app.database.port")
    .commit()?;
```

## Pattern 7: Renaming a key within the same section

```toml
# Before
[server]
bind_addr = "0.0.0.0"

# After
[server]
bind = "0.0.0.0"
```

```rust
doc.edit()
    .rename_key("server.bind_addr", "server.bind")
    .commit()?;
```

**Internal for `rename_key`:**
1. Find the key's span and its comment
2. Remove the old key line
3. Insert the new key at the same position, with the comment moved
4. Same value, same formatting, just the key name changes

## New primitives needed

| Primitive | What it does | Priority |
|---|---|---|
| `rename_key(from, to)` | Move + rename a key, preserving its comment and value | **v1** |
| `move_key(from, to)` | Move a key to a different section, preserving formatting | **v1** |
| `remove_section("s")` | Remove `[s]` header and all its keys | **v1** (already have `remove` — need to verify it handles sections) |
| `extract_above_comment(path)` | Read the comment above a key (read-accessor) | v2 |
| `extract_line(path)` | Read the full source line for a key (read-accessor) | v2 |

## Migration recipe: "make it so"

The user wants to migrate from one config layout to another. They write a
migration that reads the old document and produces the new one:

```rust
fn migrate_v1_to_v2(old: &mut FlatDoc) -> Result<(), EditError> {
    old.edit()
        // Network: flatten tls into network section
        .rename_key("tls.enabled", "network.tls_enabled")
        .rename_key("tls.cert", "network.tls_cert")
        .remove_section("tls")
        // Server: extract database settings
        .insert_section("database")
            .with_above_comment("Database connection (migrated from [server])")
        .move_key("server.db_host", "database.host")
        .move_key("server.db_port", "database.port")
        // Cache: rename section
        .rename_section("cache", "performance.cache")
        .commit()?;
    Ok(())
}
```

The migration is self-documenting: each line says what it does and why.
