# Issue analysis: how the batch editor maps to open needs

## Directly addressed (12 issues)

| Issue | Title | How the batch editor handles it |
|---|---|---|
| **#231** | Allow inserting where we carry over original formatting | Core design. `set()` preserves by default. `insert()` copies indent. Explicit prefix/suffix control. |
| **#818** | Format drops comments between items in a table | `set()` only splices the value span — comments untouched. |
| **#812** | Output integers as hexadecimal | `value_hex(n)` helper planned for v1. |
| **#590** | Customize literal-ness (single/double quotes) of a string | `value_literal(s)`, `value_basic(s)` helpers. |
| **#753** | Allow setting key's representation | Pass quoted keys as raw strings, or `.with_quoted_key()`. |
| **#742** | Missing output on or_insert()ing nested Item::None | Immune — no Item::None, no nested [] chains. |
| **#683** | Document Item::None return values | Eliminated — no Item::None in the API. |
| **#697** | Remove Item support from Document's API | Eliminated — no Item type anywhere. |
| **#888** | Split Decor out of types | Eliminated — no Decor type. Raw prefix/suffix strings. |
| **#1027** | Macros/formatting hooks for serialization layout | `with_above_comment`, `with_block_comment`, `with_prefix`, `with_suffix` are the hooks. |
| **#1158** | Preserve absence of final newline | Line ending detection planned for v2. |
| **#685** | Examples need improvement | Migration recipes serve as worked examples. |

## Partially addressed (3 issues)

| Issue | Title | Status |
|---|---|---|
| **#747** | Insert table key at ordinal | v2 — `insert_before`, `insert_after`, `insert_first` designed. |
| **#1083** | Table::position + AOT rendering | v1 is append-only — positional AOT editing is v2. |
| **#1088** | to_string_pretty | Not in scope — batch editor is format-preserving, not reformatting. |

## Gaps we should cover (4 issues)

| Issue | Title | What we need |
|---|---|---|
| **#1114** | Support u64, i128, u128 integer types | `value()` helper must handle large integers. Today's `value(s)` takes `&str`, so it already handles any integer representation the caller provides. But a typed `value(42u64)` would need explicit support. |
| **#937** | Prefer literal strings for key representation | Auto-detect key quoting: use `'literal'` when the key has no special chars, `"basic"` when it does. A `key_repr(s)` helper. |
| **#1008** | Display produces empty string | Verify `to_string()` never produces empty output for non-empty documents. Currently returns `source.clone()` — should be fine. |
| **#1028** | Compile more quickly | `toml_fast` has zero non-std deps. Already addresses this. |

## Not in scope

| Issue | Title | Why |
|---|---|---|
| #1162 | Strong type broke serializer | `toml` crate serde issue, not `toml_fast` |
| #1134 | Make Spanned generic | Serde concern |
| #1128 | Empty HashMap deserialization | Serde concern |
| #1120 | Error highlight + multi-byte chars | `toml` crate error rendering |
| #1108 | Inaccuracies in errors | `toml` crate |
| #1091 | Overly verbose error message | `toml` crate |
| #1054 | no_std for riscv32 | Build system |
| #1053 | MSRV violation | Build system |
| #1017 | DeInteger/DeFloat awkward | `toml` crate serde |
| #1012 | Duplicate key error lost name | `toml` crate error rendering |
| #1010 | Deserialize impls for DeValue | `toml` crate serde |
| #965 | Deserialize newtype with None | `toml` crate serde |
| #762 | Value not written if table constructed beforehand | `toml_edit` footgun (we documented this) |
| #729 | Allow creating Decor instances | Eliminated — no Decor type |
| #717 | From<inner> for Item | Eliminated — no Item type |
| #696 | Time doesn't impl Serialize/Deserialize | `toml_datetime` serde |
| #662 | AOT inlined by default | Serde concern |
| #636 | typetag serialization | Serde concern |
| #589 | Spans lost with flatten | Serde concern |
| #573 | Preserve order cargo feature | `toml` crate discussion |
| #565 | Top-level ValueSerializer shorthand | `toml` crate serde |
| #515 | Crater run | CI |
| #504 | Single-line errors contain trailing newlines | `toml` crate error rendering |
| #458 | Multi-line spans not rendered correctly | `toml` crate error rendering |
| #440 | Explicit datetime deserialization | Serde concern |
| #390 | Parse chrono Local Dates | Serde concern |
| #368 | Make Spanned fields public | Serde concern |
| #267 | decor_mut injects arbitrary data | Eliminated — no decor_mut |
| #262 | TableLike::fmt doesn't format dotted keys | Eliminated — no fmt() |
| #229 | Decouple end users from iterator impls | Not applicable |
| #208 | Array drain | v2 — array editing |
| #163 | Dotted key ordering not preserved | Discussed — v2 dotted key style detection |
