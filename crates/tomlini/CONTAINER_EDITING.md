# Array, Array-of-Tables, and Inline Table Editing — Design

## What exists vs what's needed

| Container | Get | Set | Insert | Remove | Clear | Push | Format |
|---|---|---|---|---|---|---|---|
| Key-value | — | `set("k", "v")` | `insert("t", "k", "v")` | `remove("k")` | `clear_section("t")` | — | indent copy |
| Array | ? | ? | ? | ? | ? | ? | ? |
| Inline table | ? | ? | ? | ? | ? | — | ? |
| Array-of-tables | ? | ? | ? | ? | ? | ? | ? |

## 1. Array editing

The flat span model gives us `ArrayOpen` ... elements ... `ArrayClose`.
Elements are separated by `Comma` spans. We can find element N by counting
value spans between the brackets, tracking nesting depth for nested arrays.

### API

```rust
doc.edit()
    // Read
    .array_len("allowed_hosts")           // → usize
    .array_get("allowed_hosts", 0)        // → &str (raw source text)

    // Write
    .array_set("allowed_hosts", 1, "\"10.0.1.1\"")   // replace element
    .array_insert("allowed_hosts", 1, "\"10.0.0.2\"") // insert at position
    .array_remove("allowed_hosts", 0)                  // remove element
    .array_push("allowed_hosts", "\"10.0.0.3\"")       // append
    .array_clear("allowed_hosts")                      // empty the array

    // Formatting
    .array_push("allowed_hosts", "\"10.0.0.3\"")
        .with_element_prefix("\n    ")    // prefix before the new element
        .with_element_suffix(",\n")       // suffix (comma style)

    .commit()?;
```

### Internal: finding element N

```
spans for: arr = [1, 2, 3]

  ArrayOpen  BareKey  Equals  ArrayOpen  Integer  Comma  Integer  Comma  Integer  ArrayClose
                              ──────────────────────┬─────────────────────────────────────
                              depth = 1             │ element 0: Integer("1")
                                                    │ element 1: Integer("2")
                                                    │ element 2: Integer("3")

To find element 1:
  1. Find the ArrayOpen span after the Equals for "arr"
  2. Walk forward, incrementing a counter for each value at depth 1
     (skip nested ArrayOpen/ArrayClose pairs by tracking depth)
  3. Return the span of the second value at depth 1
```

### Formatting: push into a pretty array

```toml
# Before
arr = [
    1,
    2,
]

# After push(3)
arr = [
    1,
    2,
    3,       ← discovers indentation from previous elements
]
```

The editor inspects the last element's byte range, walks back to the start
of its line to discover the indentation string, and uses it for the new element.

## 2. Array-of-tables editing

An AOT is a sequence of `[[name]]` headers, each followed by key-value pairs.
In the flat span stream, these are consecutive `ArrayTableOpen` + key path +
`ArrayTableClose` sequences.

### API

```rust
doc.edit()
    // Read
    .aot_len("backend")                    // → usize (how many entries)

    // Write
    .aot_push("backend", &[
        ("host", "\"10.0.0.3\""),
        ("port", "9001"),
    ])                                     // append a new [[backend]]
    .aot_set("backend", 0, "host", "\"10.0.0.5\"")  // set value in entry 0
    .aot_insert_key("backend", 0, "weight", "100")  // add key to entry 0
    .aot_remove_key("backend", 0, "port")            // remove key from entry 0
    .aot_remove("backend", 1)                         // remove entire [[backend]] entry 1
    .aot_clear("backend")                             // remove all [[backend]] entries

    .commit()?;
```

### Internal: finding AOT entry N

The flat spans don't explicitly group AOT entries. The grouping is implicit:
consecutive `[[name]]` headers with the same name.

```
Span stream for [[backend]], [[backend]], [[bin]]:

  ArrayTableOpen  BareKey("backend")  ArrayTableClose  BareKey("host")  Equals  ...
  ←─── entry 0 ───→
  ...  ArrayTableOpen  BareKey("backend")  ArrayTableClose  BareKey("host")  Equals  ...
        ←─── entry 1 ───→
  ...  ArrayTableOpen  BareKey("bin")  ArrayTableClose  ...
        ←─── different AOT ──→
```

To find entry N:
1. Scan for `ArrayTableOpen` spans
2. Check if the key path inside matches the target name
3. Count matches until we reach N
4. The entry spans from that `ArrayTableOpen` to the byte just before the
   next `ArrayTableOpen` (or end of document)

### Formatting: push a new AOT entry

```toml
# Before
[[backend]]
host = "10.0.0.1"
port = 9001

[[backend]]
host = "10.0.0.2"
port = 9001

# After push
[[backend]]
host = "10.0.0.1"
port = 9001

[[backend]]
host = "10.0.0.2"
port = 9001

[[backend]]         ← new entry, copies spacing from above
host = "10.0.0.3"
port = 9001
```

The editor discovers the blank-line separator convention from existing
entries and uses it for the new entry.

## 3. Inline table editing

An inline table is `{ key = value, key = value }`. In the flat span stream:
`InlineTableOpen` ... key `Equals` value `Comma` ... `InlineTableClose`.

### API

```rust
doc.edit()
    .inline_set("colors", "red", "\"#cc0000\"")     // set value in inline table
    .inline_insert("colors", "blue", "\"#0000ff\"")  // add key to inline table
    .inline_remove("colors", "green")                // remove key from inline table
    .commit()?;
```

### Internal

Similar to array element access: walk the spans between `InlineTableOpen`
and matching `InlineTableClose`, tracking key=value pairs.

### Limitation

Inline tables NESTED inside arrays or other inline tables are NOT addressed
in v1. Path would need to be `"parent.inline_table.key"` which requires
structural parsing of nested containers. v2 feature.

## 4. What changes in the span index

Currently the parser produces a flat `Vec<Span>`. For container editing,
we need to resolve spans INTO the container. The `build_index` function
in `edit.rs` already walks spans and builds key paths. We extend it to
also build container boundaries:

```rust
struct ContainerMeta {
    kind: ContainerKind,       // Array or InlineTable
    open_idx: usize,           // index into spans
    close_idx: usize,          // matching close
    element_spans: Vec<usize>, // indices of value spans at depth 1
}

enum ContainerKind { Array, InlineTable }
```

This is built during the same pass as the key-path index. For inline
tables, the container is also a value (so it has a key path entry).

## 5. Priority for v1

| Feature | Priority | Reason |
|---|---|---|
| `array_push` | **v1** | Most common array operation |
| `array_set` | **v1** | Next most common |
| `array_remove` | **v1** | Completes CRUD |
| `aot_push` | **v1** | Most common AOT operation |
| `aot_set` | **v1** | Common |
| `inline_set` | **v1** | Common for config values |
| `array_insert` (positional) | v2 | Less common than push |
| `array_elements` | v2 | Need for iteration |
| `aot_insert_key` | v2 | Can be done via remove + re-add |
| Nested container paths | v2 | Complex structural parsing |

## 6. Performance expectations

Array and inline-table editing is more expensive than key-value editing
because finding element N requires walking the span stream within the
container brackets. For a 100-element array, finding element 50 is ~50 span
checks — still <1 µs. The splice+reparse cost dominates as always.
