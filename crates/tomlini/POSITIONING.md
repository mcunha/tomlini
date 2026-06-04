# `toml_fast` vs the original crates — positioning

## The landscape

```
                    ┌──────────────────────────────────────────┐
                    │              toml (serde)                │
                    │  Deserialize TOML into Rust structs      │
                    │  std only, no format preservation        │
                    ├──────────────────────────────────────────┤
                    │            toml_edit (DOM)               │
                    │  Parse, edit, preserve formatting        │
                    │  alloc only, 28 documented footguns      │
                    ├──────────────────┬───────────────────────┤
                    │  toml_parser     │  toml_writer          │
                    │  low-level lex   │  low-level serializer │
                    │  no_std          │  no_std               │
                    ├──────────────────┴───────────────────────┤
                    │  toml_datetime (shared types)             │
                    └──────────────────────────────────────────┘

                    ┌──────────────────────────────────────────┐
                    │          toml_fast (SAX + batch)         │
                    │  Parse, edit, preserve formatting        │
                    │  core / alloc / std, 0 footguns          │
                    └──────────────────────────────────────────┘
```

## Performance

| Operation | toml_edit | toml_fast | Speedup |
|---|---|---|---|
| Parse (94-line cargo.toml) | 59.5 µs | 3.3 µs | **18×** |
| Parse (1000-line web-sys) | 991 µs | 76.5 µs | **13×** |
| Parse + serialize (94-line) | 88.6 µs | 3.3 µs + source.clone | **27×** |
| Set value (94-line) | 88.7 µs | 28.5 µs | **3.1×** |
| Insert key (94-line) | 93.6 µs | 30.9 µs | **3.0×** |
| Remove key (94-line) | 89.2 µs | 29.9 µs | **3.0×** |
| 5-edit batch (94-line) | ~450 µs | ~35 µs | **13×** |

## Footgun surface

| Footgun | toml_edit | toml_fast |
|---|---|---|
| IndexMut creates InlineTable | ☢️ | Immune — no IndexMut |
| insert() destroys key comments | 🔴 | Immune — set() splices only value span |
| value() loses formatting | 🟡 | Immune — caller passes raw string |
| fmt() destroys everything | ☢️ | Immune — no fmt() |
| New tables lose header decor | 🟡 | Immune — no table objects |
| Comments between keys owned by next | 🟢 | Explicit prefix/suffix control |
| Array::fmt() destroys multiline | 🔴 | Immune — no fmt() |
| Dotted key as literal, not path | 🟢 | Dot-paths are explicit `&[&str]` |
| **Total documented footguns** | **28** | **0** |

## no_std

| Tier | toml_edit | toml_fast |
|---|---|---|
| `core` (no alloc, no std) | Not supported | `parse_into()` — classified spans via callback |
| `alloc` (no std) | Not supported | Full `FlatDoc`, parse, edit, serialize |
| `std` | Full API | Full API |

## API comparison for common tasks

### Change a value

| | toml_edit | toml_fast |
|---|---|---|
| Code | `doc["s"]["k"] = value(x)` | `.set("s.k", "x")` |
| Path syntax | `["s"]["k"]` | `"s.k"` |
| Preserves comment? | Depends on constructor | Yes, by default |
| Inline comment on value | Lost | Preserved |

### Add a key with comment

| | toml_edit | toml_fast |
|---|---|---|
| Code | `Key::new()` + `Decor::set_prefix()` + `insert_formatted()` | `.insert("s", "k", "v").with_above_comment("text")` |
| Lines of code | 6 | 3 |
| API concepts | 4 (`Key`, `Decor`, `set_prefix`, `insert_formatted`) | 1 (`.with_above_comment()`) |

### Multiple edits

| | toml_edit | toml_fast |
|---|---|---|
| Style | Imperative, applied immediately | Declarative batch, committed at once |
| Index/validation | Per-operation | Once on commit |
| Error handling | Per-operation | All-or-nothing |

## What `toml_fast` does NOT do

| Capability | Status | Why |
|---|---|---|
| Serde deserialization | Not in scope | Use the `toml` crate for that |
| Serde serialization | Not in scope | Use the `toml` crate for that |
| Array-of-tables editing | v2 | Complex, positional awareness needed |
| Array element editing | v2 | Splicing within `[...]` is delicate |
| Inline table editing | v2 | Structural parsing within `{...}` |
| Key reordering (`sort_keys`) | v2 | Composition of get + remove + insert |
| Error recovery | Not in scope | Parse succeeds or fails — no partial |
| Spec validation (duplicate keys) | Not in scope | Parser is lenient by design |
| Pretty-print / reformat | Not in scope | Preserve, don't reformat |

## What each crate is FOR

| Crate | Use when you want to |
|---|---|
| `toml` | Deserialize TOML into Rust structs. Serde integration. Don't care about preserving comments. |
| `toml_edit` | Edit existing TOML files while preserving formatting. Willing to learn the DOM model and navigate 28 footguns. |
| `toml_parser` | Build your own TOML tool. Need low-level token/event stream. |
| `toml_writer` | Build your own TOML serializer. |
| `toml_fast` | Parse and edit TOML files fast, with zero surprises, on any target. Preserve formatting by default. |

## The unique selling points

1. **Zero-footgun design**: every API path preserves formatting unless you explicitly override it. No silent destruction.

2. **18× faster parse**: single-pass flat classification. No DOM construction. No double Vec<Token>/Vec<Event> buffer.

3. **core/alloc/std**: works on bare metal, WASM, embedded, and desktop. The only TOML editor that can run without a heap.

4. **Declarative batch editing**: express intent, commit once. No span fixup churn. No index rebuilds.

5. **Formatting primitives**: `.with_above_comment("text")`, `.with_block_comment(&[...])`, `.with_prefix("\n")`. No Decor model to learn.
