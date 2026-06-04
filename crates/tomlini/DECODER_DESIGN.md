# toml-test decoder — direct from spans, no toml_edit

## Architecture: two-pass

**Phase 1 — Collect**: walk all spans once, produce a flat `Vec<FlatEntry>`:

```rust
struct FlatEntry {
    table_path: Vec<String>,  // parent table path, e.g. ["server", "database"]
    key: String,              // leaf key name
    value: DecodedValue,      // already decoded
}
```

This handles:
- Root-level kv pairs → `table_path = []`
- `[section]` → sets current `table_path`, subsequent kv pairs inherit it
- Dotted keys `a.b = 1` → `table_path = []`, `key = "a"`, value contains `{b: 1}`
- `[[aot]]` → groups consecutive entries, collects into array

**Phase 2 — Nest**: take the flat list and build `HashMap<String, DecodedValue>`:

```rust
fn nest(entries: &[FlatEntry]) -> HashMap<String, DecodedValue> {
    let mut grouped: HashMap<Vec<String>, Vec<(&str, DecodedValue)>> = HashMap::new();
    for e in entries {
        grouped.entry(e.table_path.clone()).or_default().push((&e.key, e.value));
    }
    // Build tree from root downward
    build_tree(&mut grouped, &[])
}
```

This recursively inserts into nested HashMaps by matching path prefixes.

## What the previous attempt got wrong

1. **Tried to insert into HashMap during the walk** — the flat-to-nested conversion needs
   to happen AFTER all entries are collected, because table headers can appear after
   keys that reference the same table (TOML allows `[x]` after `x.y = 1`).

2. **Dotted keys beyond 2 levels**: `a.b.c = 1` should produce `{a: {b: {c: 1}}}`.
   This requires recursively nesting the dotted key parts into sub-maps.

3. **AOT implicit tables**: `[[a.b]]` creates `a = [{b: ...}]`. The intermediate table
   `[a]` must be implicitly created if it doesn't exist.

4. **String value decoding**: The decoder must run string escape processing to produce
   the actual decoded string, not the raw source. `\n` → actual newline.

5. **Datetime classification**: The decoder must look at the raw datetime string to
   determine whether it's offset-datetime, local-datetime, date-local, or time-local.

## Detailed design

### FlatEntry collection

Walk spans in order:
1. Track `current_table: Vec<String>` — set by `[header]` markers
2. When a key=value pair is found:
   - If key is a dotted key like `a.b.c`:
     - The value goes under `table_path = current_table + ["a", "b"]`, key = "c"
   - If key is bare like `key`:
     - The value goes under `table_path = current_table`, key = "key"
3. For `[[aot]]`: push all entries with the same header name into a Vec
4. For arrays: recursively collect elements
5. For inline tables: recursively collect key-value pairs

### Nested HashMap construction

```rust
fn build_nested(all: &[(Vec<String>, String, DecodedValue)]) -> HashMap<String, DecodedValue> {
    let mut root: HashMap<String, DecodedValue> = HashMap::new();
    
    // Group by first path component
    let mut first_groups: HashMap<String, Vec<_>> = HashMap::new();
    for (path, key, val) in all {
        if path.is_empty() {
            root.insert(key.clone(), val.clone());
        } else {
            first_groups.entry(path[0].clone()).or_default().push((path.clone(), key.clone(), val.clone()));
        }
    }
    
    // Recurse into each group
    for (first, entries) in first_groups {
        let sub_entries: Vec<_> = entries.iter().map(|(p, k, v)| (p[1..].to_vec(), k.clone(), v.clone())).collect();
        root.insert(first, DecodedValue::Table(build_nested(&sub_entries)));
    }
    
    root
}
```

## What to handle

| Construct | Approach |
|---|---|
| Root kv pairs | `table_path = []` |
| `[section]` | Sets `table_path` to `["section"]` |
| `[a.b]` | Sets `table_path` to `["a", "b"]` |
| `[[aot]]` | Collects entries, wraps in `DecodedValue::Array` |
| `key = bare_value` | Emit as `(current_table, key, value)` |
| `a.b.c = value` | Split key: `table = [current_table, "a", "b"]`, `key = "c"` |
| `[1, 2, 3]` | Depth-aware walk within `[...]`, produce `Vec<DecodedValue>` |
| `{x = 1}` | Depth-aware walk within `{...}`, produce `HashMap` |
| `nan`, `inf` | Raw source text passed as `DecodedScalar::Float("nan")` |
| `1979-05-27T07:32:00Z` | Classify by presence of `-`, `:`, `Z` in raw text |
| `\n`, `\t`, `\\`, `\"` | Decode escapes in basic strings |
| `\uXXXX` | Decode unicode escapes |

## Edge cases

- **Empty table `[a]` with no keys**: should produce `{a: {}}`. We need to emit a table entry even if no kv pairs exist.
- **`[a]` after `a.b = 1`**: the FlatEntry for `a.b = 1` has `table_path = ["a"]`. When `[a]` appears later with no keys, we still need `{a: {}}` in the output. The `[a]` marker should register that the table exists.
- **`[[a]]` with no keys**: should produce `{a: [{}]}`.
- **BOM at start of file**: UTF-8 BOM (0xEF 0xBB 0xBF) should be stripped before parsing.

## Implementation plan

1. Add `collect_entries(doc) -> Vec<(Vec<String>, String, DecodedValue)>` as phase 1
2. Add `build_nested(entries) -> HashMap<String, DecodedValue>` as phase 2  
3. Add `decode_doc(doc) -> DecodedValue` that chains 1 + 2
4. Handle edge cases: empty tables, AOT, BOM
5. Test against toml-test with empty ignore list
