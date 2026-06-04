//! Batch editor for `tomlini`.
//!
//! The [`Editor`] collects a queue of mutation operations and applies them
//! all at once when [`commit`](Editor::commit) is called.  This avoids
//! re-indexing and repeated span-shifting for every single change.
//!
//! # Fluent entry point
//!
//! For convenience call [`FlatDoc::edit`](crate::FlatDoc::edit) to get an
//! [`EditorHandle`] that chains operations and commits automatically:
//!
//! ```ignore
//! doc.edit()
//!    .set("package.version", "\"2.0.0\"")
//!    .insert("package", "license", "\"MIT\"")
//!    .remove("package.edition")
//!    .commit()?;
//! ```
//!
//! # Standalone usage
//!
//! ```ignore
//! let mut editor = Editor::new();
//! editor.set("package.version", "\"2.0.0\"");
//! editor.commit(&mut doc)?;
//! ```

use crate::{FlatDoc, EditError, Span, SpanKind, edit};

#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;


/// A batch editor that queues TOML mutation operations.
///
/// Operations are collected in order and applied in a single pass when
/// [`commit`](Editor::commit) is called.
pub struct Editor {
    ops: Vec<Op>,
}

#[derive(Debug, Clone)]
struct Op {
    kind: OpKind,
    table: Vec<String>,
    key: String,
    value: String,
    prefix: Option<String>,
    suffix: Option<String>,
    to_table: Vec<String>,
    to_key: String,
    pairs: Vec<(String, String)>,
    index: Option<usize>,
    inline_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum OpKind { Set, Insert, Remove, InsertSection, ReplaceSection, ClearSection, RenameSection, RenameKey, MoveKey, PromoteKey, MoveKeyCreate, ArrayPush, ArraySet, ArrayInsert, ArrayRemove, AotPush, AotSet, AotRemove, InlineSet, InlineInsert, InlineRemove, ReorderRoot }

/// How comments between root entries are associated during
/// [`reorder_root_anchored`](Editor::reorder_root_anchored).
///
/// Given this input and `reorder_root(&["meta", "base"])`:
///
/// ```toml
/// base = "my-base"
/// # comment
/// [meta]
/// kind = "leaf"
/// ```
///
/// | Variant | Output |
/// |---------|--------|
/// | `Preceding` | `[meta]\nkind = "leaf"\nbase = "my-base"\n# comment\n` — comment follows `base` |
/// | `Following` | `[meta]\n# comment\nkind = "leaf"\nbase = "my-base"\n` — comment stays with `[meta]` |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentAnchor {
    /// Comments stay with the preceding entry (default, structural).
    Preceding,
    /// Comments stay with the following entry — use this when your project
    /// convention places comments above the section they describe.
    Following,
}
// Helpers
// ============================================================

/// Split a dotted path `"a.b.c.key"` into `(vec!["a","b","c"], "key")`.
pub(crate) fn split_path(path: &str) -> (Vec<String>, String) {
    let parts: Vec<&str> = path.split('.').collect();
    let key = parts.last().map(|s| s.to_string()).unwrap_or_default();
    let table: Vec<String> = parts[..parts.len().saturating_sub(1)]
        .iter().map(|s| s.to_string()).collect();
    (table, key)
}

/// Compare a `Vec<String>` path against a `&[&str]` slice.
pub(crate) fn path_eq(a: &[String], b: &[&str]) -> bool {
    a.len() == b.len() && a.iter().zip(b.iter()).all(|(a, b)| a == b)
}

/// Check if a span kind represents a TOML value node (leaf or container).
fn is_value_kind(k: SpanKind) -> bool {
    matches!(k,
        SpanKind::Integer | SpanKind::Float | SpanKind::Boolean
        | SpanKind::Datetime | SpanKind::BasicString | SpanKind::LiteralString
        | SpanKind::MlBasicString | SpanKind::MlLiteralString
        | SpanKind::InlineTableOpen | SpanKind::ArrayOpen)
}

/// Find the matching close bracket for an open bracket span.
/// Returns the span index of the matching `ArrayClose` or `InlineTableClose`.
fn find_matching_close(spans: &[Span], open_idx: usize) -> Option<usize> {
    let (open_k, close_k) = match spans[open_idx].kind {
        SpanKind::ArrayOpen => (SpanKind::ArrayOpen, SpanKind::ArrayClose),
        SpanKind::InlineTableOpen => (SpanKind::InlineTableOpen, SpanKind::InlineTableClose),
        _ => return None,
    };
    let mut depth = 1;
    for i in (open_idx + 1)..spans.len() {
        if spans[i].kind == open_k {
            depth += 1;
        } else if spans[i].kind == close_k {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
    }
    None
}

/// Walk top-level elements inside an array (open_idx..close_idx).
/// Returns a list of `(value_span_idx, trailing_comma_span_idx_or_none)`.
fn walk_array_elements(spans: &[Span], open_idx: usize, close_idx: usize) -> Vec<(usize, Option<usize>)> {
    let mut elems = Vec::new();
    let mut depth = 1;
    let mut i = open_idx + 1;

    while i < close_idx {
        match spans[i].kind {
            SpanKind::ArrayOpen | SpanKind::InlineTableOpen => {
                if depth == 1 {
                    let val_idx = i;
                    // Skip to matching close
                    if let Some(close) = find_matching_close(spans, i) {
                        i = close;
                    }
                    i += 1;
                    // Find trailing comma after this container element
                    let comma = skip_to_comma(spans, &mut i, close_idx);
                    elems.push((val_idx, comma));
                } else {
                    depth += 1;
                    i += 1;
                }
            }
            SpanKind::ArrayClose | SpanKind::InlineTableClose => {
                depth -= 1;
                i += 1;
            }
            k if is_value_kind(k) => {
                if depth == 1 {
                    let val_idx = i;
                    i += 1;
                    let comma = skip_to_comma(spans, &mut i, close_idx);
                    elems.push((val_idx, comma));
                } else {
                    i += 1;
                }
            }
            _ => { i += 1; }
        }
    }
    elems
}

/// Advance `i` past whitespace/newline/comment spans and return the
/// index of the next Comma span (if any), consuming it.
fn skip_to_comma(spans: &[Span], i: &mut usize, limit: usize) -> Option<usize> {
    while *i < limit {
        match spans[*i].kind {
            SpanKind::Whitespace | SpanKind::Newline | SpanKind::Comment => { *i += 1; }
            SpanKind::Comma => { let idx = *i; *i += 1; return Some(idx); }
            _ => return None,
        }
    }
    None
}

/// Check whether an array uses multi-line formatting.
fn array_is_multiline(spans: &[Span], open_idx: usize, close_idx: usize) -> bool {
    spans[open_idx + 1..close_idx].iter().any(|s| s.kind == SpanKind::Newline)
}

/// Find the indentation string used before array elements.
fn array_indent(spans: &[Span], source: &str, open_idx: usize) -> String {
    let mut i = open_idx + 1;
    // Skip past the newline after `[`
    while i < spans.len() && spans[i].kind == SpanKind::Newline { i += 1; }
    if i < spans.len() && spans[i].kind == SpanKind::Whitespace {
        source[spans[i].start as usize..spans[i].end as usize].to_string()
    } else {
        String::new()
    }
}

/// Find the array at `path` via the index, returning `(open_idx, close_idx)`.
fn resolve_array(
    doc: &FlatDoc,
    index: &[(Vec<String>, edit::Entry)],
    op: &Op,
) -> Result<(usize, usize), EditError> {
    let path: Vec<&str> = op.table.iter()
        .map(|s| s.as_str())
        .chain(core::iter::once(op.key.as_str()))
        .collect();
    let entry = index.iter()
        .find(|(p, _)| path_eq(p, &path))
        .ok_or(EditError::NotFound)?;
    let open_idx = entry.1.value_idx;
    if doc.spans[open_idx].kind != SpanKind::ArrayOpen {
        return Err(EditError::InvalidPath);
    }
    let close_idx = find_matching_close(&doc.spans, open_idx)
        .ok_or(EditError::InvalidPath)?;
    Ok((open_idx, close_idx))
}

/// Find the inline table at `path` via the index, returning `(open_idx, close_idx)`.
fn resolve_inline_table(
    doc: &FlatDoc,
    index: &[(Vec<String>, edit::Entry)],
    op: &Op,
) -> Result<(usize, usize), EditError> {
    let path: Vec<&str> = op.table.iter()
        .map(|s| s.as_str())
        .chain(core::iter::once(op.key.as_str()))
        .collect();
    let entry = index.iter()
        .find(|(p, _)| path_eq(p, &path))
        .ok_or(EditError::NotFound)?;
    let open_idx = entry.1.value_idx;
    if doc.spans[open_idx].kind != SpanKind::InlineTableOpen {
        return Err(EditError::InvalidPath);
    }
    let close_idx = find_matching_close(&doc.spans, open_idx)
        .ok_or(EditError::InvalidPath)?;
    Ok((open_idx, close_idx))
}

/// Walk key=value pairs inside an inline table.
/// Returns `(key_text, value_span_idx, trailing_comma_span_idx_or_none)`.
fn walk_inline_pairs(
    spans: &[Span],
    source: &str,
    open_idx: usize,
    close_idx: usize,
) -> Vec<(String, usize, Option<usize>)> {
    let mut pairs = Vec::new();
    let mut depth = 1;
    let mut i = open_idx + 1;

    while i < close_idx {
        match spans[i].kind {
            SpanKind::BareKey | SpanKind::BasicString | SpanKind::LiteralString => {
                if depth == 1 {
                    let key_text = source[spans[i].start as usize..spans[i].end as usize].to_string();
                    i += 1;
                    // Skip whitespace to find '='
                    while i < close_idx {
                        match spans[i].kind {
                            SpanKind::Whitespace | SpanKind::Newline | SpanKind::Comment => { i += 1; }
                            SpanKind::Equals => { i += 1; break; }
                            _ => break,
                        }
                    }
                    // Skip whitespace to find value
                    while i < close_idx {
                        match spans[i].kind {
                            SpanKind::Whitespace | SpanKind::Newline | SpanKind::Comment => { i += 1; }
                            k if is_value_kind(k) => {
                                let val_idx = i;
                                // Skip container values
                                if k == SpanKind::ArrayOpen || k == SpanKind::InlineTableOpen {
                                    if let Some(close) = find_matching_close(spans, i) {
                                        i = close;
                                    }
                                }
                                i += 1;
                                let comma = skip_to_comma(spans, &mut i, close_idx);
                                pairs.push((key_text, val_idx, comma));
                                break;
                            }
                            _ => break,
                        }
                    }
                } else {
                    if spans[i].kind == SpanKind::ArrayOpen || spans[i].kind == SpanKind::InlineTableOpen {
                        depth += 1;
                    }
                    i += 1;
                }
            }
            SpanKind::ArrayOpen | SpanKind::InlineTableOpen => {
                depth += 1;
                i += 1;
            }
            SpanKind::ArrayClose | SpanKind::InlineTableClose => {
                depth -= 1;
                i += 1;
            }
            _ => { i += 1; }
        }
    }
    pairs
}

// ============================================================
// Editor
// ============================================================

impl Editor {
    /// Create a new batch editor with an empty operation queue.
    pub fn new() -> Self { Editor { ops: Vec::new() } }

    /// Queue a set (replace) operation.
    ///
    /// `path` is a dotted key path, e.g. `"package.version"`.
    /// Only the value span is replaced; surrounding formatting is preserved.
    ///
    /// # Example
    ///
    /// ```ignore
    /// editor.set("package.version", "\"2.0.0\"");
    /// ```
    pub fn set(&mut self, path: &str, value: &str) -> &mut Self {
        let (table, key) = split_path(path);
        self.ops.push(Op {
            kind: OpKind::Set, table, key, value: value.to_string(),
            prefix: None, suffix: None,
            to_table: Vec::new(), to_key: String::new(), pairs: Vec::new(),
            index: None, inline_key: None,
        });
        self
    }

    /// Queue an insert operation.
    ///
    /// Inserts `key = value` into the table identified by `table_path`.
    /// Pass an empty string `""` for the document root.
    ///
    /// # Example
    ///
    /// ```ignore
    /// editor.insert("package", "license", "\"MIT\"");
    /// editor.insert("", "name", "\"my-app\"");  // root table
    /// ```
    pub fn insert(&mut self, table_path: &str, key: &str, value: &str) -> &mut Self {
        let table: Vec<String> = table_path.split('.').map(|s| s.to_string()).filter(|s| !s.is_empty()).collect();
        self.ops.push(Op {
            kind: OpKind::Insert, table, key: key.to_string(), value: value.to_string(),
            prefix: None, suffix: None,
            to_table: Vec::new(), to_key: String::new(), pairs: Vec::new(),
            index: None, inline_key: None,
        });
        self
    }

    /// Queue a remove operation.
    ///
    /// Removes the key-value pair at the dotted `path`, including the line
    /// it occupies and any trailing newline.
    ///
    /// # Example
    ///
    /// ```ignore
    /// editor.remove("package.edition");
    /// ```
    pub fn remove(&mut self, path: &str) -> &mut Self {
        let (table, key) = split_path(path);
        self.ops.push(Op {
            kind: OpKind::Remove, table, key, value: String::new(),
            prefix: None, suffix: None,
            to_table: Vec::new(), to_key: String::new(), pairs: Vec::new(),
            index: None, inline_key: None,
        });
        self
    }

    // ---- Formatting modifiers (apply to the LAST pushed op) ----

    /// Attach a literal prefix string to the last queued operation.
    ///
    /// The prefix is inserted immediately before the key on the output line.
    /// Call this *after* the operation it should decorate.
    ///
    /// # Example
    ///
    /// ```ignore
    /// editor.insert("deps", "serde", "\"1\"")
    ///       .with_prefix("# formatting\n");
    /// ```
    pub fn with_prefix(&mut self, prefix: &str) -> &mut Self {
        if let Some(op) = self.ops.last_mut() { op.prefix = Some(prefix.to_string()); }
        self
    }

    /// Attach a literal suffix string to the last queued operation.
    ///
    /// The suffix is inserted immediately after the value on the output line.
    /// Call this *after* the operation it should decorate.
    pub fn with_suffix(&mut self, suffix: &str) -> &mut Self {
        if let Some(op) = self.ops.last_mut() { op.suffix = Some(suffix.to_string()); }
        self
    }

    /// Attach a single `#` comment line above the last queued operation.
    ///
    /// The text is automatically prefixed with `"# "` and a trailing newline.
    ///
    /// # Example
    ///
    /// ```ignore
    /// editor.insert("", "port", "8080")
    ///       .with_above_comment("The listen port");
    /// // Produces:
    /// // # The listen port
    /// // port = 8080
    /// ```
    pub fn with_above_comment(&mut self, text: &str) -> &mut Self {
        if let Some(op) = self.ops.last_mut() { op.prefix = Some(format!("# {text}\n")); }
        self
    }

    /// Attach multiple `#` comment lines above the last queued operation.
    ///
    /// Each string in the slice becomes its own `#`-prefixed line.
    ///
    /// # Example
    ///
    /// ```ignore
    /// editor.insert("", "timeout", "30")
    ///       .with_block_comment(&["Connection settings", "Timeout in seconds"]);
    /// ```
    pub fn with_block_comment(&mut self, lines: &[&str]) -> &mut Self {
        let block: String = lines.iter().map(|l| format!("# {l}\n")).collect();
        if let Some(op) = self.ops.last_mut() { op.prefix = Some(block); }
        self
    }

    // ---- Reordering ----

    /// Reorder root-level entries (scalars and table headers) into the
    /// specified sequence.
    ///
    /// Each name in `order` must exist at root level. Entries not listed
    /// are placed after the listed entries in their original relative order.
    /// Formatting (comments, whitespace, key decor) is preserved for each
    /// entry because entire byte ranges are moved, not individual lines.
    ///
    /// Comments between entries are associated with the **preceding** entry.
    /// Use [`reorder_root_anchored`](Editor::reorder_root_anchored) with
    /// [`CommentAnchor::Following`] to associate them with the following entry.
    pub fn reorder_root(&mut self, order: &[&str]) -> &mut Self {
        self.reorder_root_anchored(order, CommentAnchor::Preceding)
    }

    /// Reorder root-level entries with explicit comment association.
    ///
    /// See [`CommentAnchor`] for concrete before/after examples.
    ///
    /// `anchor` controls which entry "owns" comments that appear between
    /// two root entries:
    ///
    /// - [`CommentAnchor::Preceding`] (default, `reorder_root`) — comments
    ///   stay with the entry above them.  Structural default: the TOML spec
    ///   does not define comment association.
    ///
    /// - [`CommentAnchor::Following`] — comments stay with the entry below
    ///   them.  Use this when your project convention places comments above
    ///   the section they describe (e.g., pont-style profile configs).
    pub fn reorder_root_anchored(&mut self, order: &[&str], anchor: CommentAnchor) -> &mut Self {
        let tag = match anchor { CommentAnchor::Preceding => None, CommentAnchor::Following => Some("following".to_string()) };
        self.ops.push(Op {
            kind: OpKind::ReorderRoot, table: Vec::new(), key: String::new(), value: String::new(),
            prefix: Some(order.join(",")), suffix: tag,
            index: None, inline_key: None, pairs: Vec::new(), to_table: Vec::new(), to_key: String::new(),
        });
        self
    }

    // ---- Commit ----
    ///
    /// Operations are resolved against the current document index,
    /// sorted by byte position descending, and spliced into the source
    /// string.  Span positions are adjusted for each splice.
    ///
    /// Returns `Ok(())` on success, or `Err(EditError::NotFound)` if
    /// any key path cannot be found in the document.
    ///
    /// # Example
    ///
    /// ```ignore
    /// editor.set("version", "2").commit(&mut doc)?;
    /// ```
    pub fn commit(&mut self, doc: &mut FlatDoc) -> Result<(), EditError> {
        if self.ops.is_empty() { return Ok(()); }


        let index = if let Some(ref idx) = doc.index { idx } else {
            doc.index = Some(edit::build_index(doc));
            doc.index.as_ref().unwrap()
        };
        struct Resolved { start: u32, end: u32, replacement: String }
        let mut resolved: Vec<Resolved> = Vec::with_capacity(self.ops.len());

        for op in &self.ops {
            match op.kind {
                OpKind::Set => {
                    let path: Vec<&str> = op.table.iter().map(|s| s.as_str()).chain(core::iter::once(op.key.as_str())).collect();
                    let entry = index.iter()
                        .find(|(p, _)| path_eq(p, &path))
                        .ok_or(EditError::NotFound)?;
                    let value_span = doc.spans[entry.1.value_idx];
                    resolved.push(Resolved {
                        start: value_span.start, end: value_span.end,
                        replacement: op.value.clone(),
                    });
                }
                OpKind::Insert => {
                    let entries: Vec<_> = index.iter()
                        .filter(|(p, _)| p.len() == op.table.len() + 1
                            && path_eq(&p[..op.table.len()], &op.table.iter().map(|s| s.as_str()).collect::<Vec<_>>()))
                        .collect();

                    let (insert_at, text) = if let Some((_, last)) = entries.last() {
                        let last_span = doc.spans[last.key_start];
                        let mut line_start = last_span.start as usize;
                        while line_start > 0 && doc.source.as_bytes()[line_start - 1] != b'\n' { line_start -= 1; }
                        let mut indent_end = line_start;
                        while indent_end < last_span.start as usize
                            && matches!(doc.source.as_bytes()[indent_end], b' ' | b'\t') { indent_end += 1; }
                        let indent = &doc.source[line_start..indent_end];

                        let last_value_span = doc.spans[last.value_idx];
                        let mut end = last_value_span.end as usize;
                        while end < doc.source.len() && doc.source.as_bytes()[end] != b'\n' { end += 1; }
                        if end < doc.source.len() { end += 1; }

                        let p = op.prefix.as_deref().unwrap_or("");
                        let s = op.suffix.as_deref().unwrap_or("\n");
                        (end as u32, format!("{indent}{p}{} = {}{s}", op.key, op.value))
                    } else if op.table.is_empty() {
                        let pos = doc.spans.last().map(|s| s.end).unwrap_or(0);
                        let p = op.prefix.as_deref().unwrap_or("");
                        (pos, format!("{p}{} = {}\n", op.key, op.value))
                    } else {
                        return Err(EditError::NotFound);
                    };
                    resolved.push(Resolved { start: insert_at, end: insert_at, replacement: text });
                }
                OpKind::Remove => {
                    let path: Vec<&str> = op.table.iter().map(|s| s.as_str()).chain(core::iter::once(op.key.as_str())).collect();
                    let entry = index.iter()
                        .find(|(p, _)| path_eq(p, &path))
                        .ok_or(EditError::NotFound)?;

                    let key_span = doc.spans[entry.1.key_start];
                    let value_span = doc.spans[entry.1.value_idx];
                    let mut start = key_span.start as usize;
                    while start > 0 && doc.source.as_bytes()[start - 1] != b'\n' { start -= 1; }
                    let mut end = value_span.end as usize;
                    while end < doc.source.len() && doc.source.as_bytes()[end] != b'\n' { end += 1; }
                    if end < doc.source.len() { end += 1; }
                    resolved.push(Resolved { start: start as u32, end: end as u32, replacement: String::new() });
                }
                // ---- Array operations ----
                OpKind::ArrayPush => {
                    let (open_idx, close_idx) = resolve_array(doc, &index, op)?;
                    let open = doc.spans[open_idx];
                    let close = doc.spans[close_idx];
                    let elems = walk_array_elements(&doc.spans, open_idx, close_idx);
                    let is_ml = array_is_multiline(&doc.spans, open_idx, close_idx);

                    let (ins_pos, text) = if elems.is_empty() {
                        (open.end, op.value.clone())
                    } else if is_ml {
                        let indent = array_indent(&doc.spans, &doc.source, open_idx);
                        (close.start, format!("{indent}{},\n", op.value))
                    } else {
                        // Single-line: check if last element already has trailing comma
                        let has_trailing_comma = elems.last().unwrap().1.is_some();
                        if has_trailing_comma {
                            (close.start, format!(" {}", op.value))
                        } else {
                            (close.start, format!(", {}", op.value))
                        }
                    };
                    resolved.push(Resolved { start: ins_pos, end: ins_pos, replacement: text });
                }
                OpKind::ArraySet => {
                    let (open_idx, close_idx) = resolve_array(doc, &index, op)?;
                    let elems = walk_array_elements(&doc.spans, open_idx, close_idx);
                    let idx = op.index.ok_or(EditError::InvalidPath)?;
                    if idx >= elems.len() {
                        return Err(EditError::InvalidPath);
                    }
                    let (val_idx, _) = elems[idx];
                    let val_span = doc.spans[val_idx];
                    resolved.push(Resolved {
                        start: val_span.start,
                        end: val_span.end,
                        replacement: op.value.clone(),
                    });
                }
                OpKind::ArrayInsert => {
                    let (open_idx, close_idx) = resolve_array(doc, &index, op)?;
                    let open = doc.spans[open_idx];
                    let close = doc.spans[close_idx];
                    let elems = walk_array_elements(&doc.spans, open_idx, close_idx);
                    let idx = op.index.ok_or(EditError::InvalidPath)?;
                    let is_ml = array_is_multiline(&doc.spans, open_idx, close_idx);
                    let indent = if is_ml { array_indent(&doc.spans, &doc.source, open_idx) } else { String::new() };

                    let (ins_pos, text) = if elems.is_empty() {
                        (open.end, op.value.clone())
                    } else if idx >= elems.len() {
                        // Insert at end (same as push)
                        if is_ml {
                            (close.start, format!("{indent}{},\n", op.value))
                        } else {
                            let has_comma = elems.last().unwrap().1.is_some();
                            if has_comma {
                                (close.start, format!(" {}", op.value))
                            } else {
                                (close.start, format!(", {}", op.value))
                            }
                        }
                    } else {
                        // Insert before element at index `idx`
                        let target_val_start = doc.spans[elems[idx].0].start;
                        if is_ml {
                            (target_val_start, format!("{},\n{indent}", op.value))
                        } else {
                            (target_val_start, format!("{}, ", op.value))
                        }
                    };
                    resolved.push(Resolved { start: ins_pos, end: ins_pos, replacement: text });
                }
                OpKind::ArrayRemove => {
                    let (open_idx, close_idx) = resolve_array(doc, &index, op)?;
                    let elems = walk_array_elements(&doc.spans, open_idx, close_idx);
                    let idx = op.index.ok_or(EditError::InvalidPath)?;
                    if idx >= elems.len() {
                        return Err(EditError::InvalidPath);
                    }
                    let &(val_idx, _comma) = &elems[idx];

                    let remove_start: u32;
                    let remove_end: u32;

                    if idx == 0 {
                        // First element: from value start to next element's value start (or `]`)
                        remove_start = doc.spans[val_idx].start;
                        remove_end = if elems.len() > 1 {
                            doc.spans[elems[1].0].start
                        } else {
                            doc.spans[close_idx].start
                        };
                    } else {
                        // Not first: from end of previous element's comma to this element's end
                        let &(prev_val_idx, prev_comma) = &elems[idx - 1];
                        remove_start = if let Some(comma_idx) = prev_comma {
                            doc.spans[comma_idx].end
                        } else {
                            doc.spans[prev_val_idx].end
                        };
                        remove_end = if idx + 1 < elems.len() {
                            doc.spans[elems[idx + 1].0].start
                        } else {
                            doc.spans[close_idx].start
                        };
                    }

                    resolved.push(Resolved {
                        start: remove_start,
                        end: remove_end,
                        replacement: String::new(),
                    });
                }
                // ---- AOT push ----
                OpKind::AotPush => {
                    // Find all [[name]] sections with matching path
                    let target_name = op.key.as_str();
                    let mut sections: Vec<(usize, usize)> = Vec::new(); // (header_open_idx, section_end_byte)
                    let mut i = 0;
                    while i < doc.spans.len() {
                        if doc.spans[i].kind == SpanKind::ArrayTableOpen {
                            let header_start = i;
                            let mut j = i + 1;
                            let mut path_parts: Vec<String> = Vec::new();
                            while j < doc.spans.len() {
                                match doc.spans[j].kind {
                                    SpanKind::BareKey | SpanKind::BasicString | SpanKind::LiteralString => {
                                        path_parts.push(edit::clean_key_span(&doc.source, &doc.spans[j]).to_string());
                                        j += 1;
                                    }
                                    SpanKind::Dot => { j += 1; }
                                    SpanKind::ArrayTableClose => {
                                        j += 1;
                                        break;
                                    }
                                    _ => break,
                                }
                            }
                            let header_path = path_parts.join(".");
                            if header_path == target_name {
                                // Find the end of this section
                                let mut k = j;
                                while k < doc.spans.len() {
                                    if doc.spans[k].kind == SpanKind::ArrayTableOpen
                                        || doc.spans[k].kind == SpanKind::ArrayOpen
                                    {
                                        break;
                                    }
                                    k += 1;
                                }
                                let section_end = if k > 0 && k <= doc.spans.len() {
                                    doc.spans[k - 1].end
                                } else {
                                    doc.spans[header_start].end
                                };
                                sections.push((header_start, section_end as usize));
                            }
                            i = j;
                        } else {
                            i += 1;
                        }
                    }

                    let last_section = sections.last().ok_or(EditError::NotFound)?;
                    let insert_pos = last_section.1 as u32;

                    // Detect blank-line separator before the last section header
                    let header_start = doc.spans[last_section.0].start as usize;
                    let mut sep_start = header_start;
                    while sep_start > 0 {
                        let b = doc.source.as_bytes()[sep_start - 1];
                        if b == b'\n' {
                            sep_start -= 1;
                            if sep_start > 0 && doc.source.as_bytes()[sep_start - 1] == b'\n' {
                                break;
                            }
                        } else if matches!(b, b' ' | b'\t') {
                            sep_start -= 1;
                        } else {
                            break;
                        }
                    }
                    // If no blank line found, default to "\n\n"
                    let sep = if sep_start < header_start {
                        &doc.source[sep_start..header_start]
                    } else {
                        "\n\n"
                    };

                    let header_name = if op.table.is_empty() {
                        target_name.to_string()
                    } else {
                        format!("{}.{}", op.table.join("."), target_name)
                    };
                    let kv_text: String = op.pairs.iter()
                        .map(|(k, v)| format!("{k} = {v}\n"))
                        .collect();
                    let replacement = format!("{sep}[[{header_name}]]\n{kv_text}");
                    resolved.push(Resolved { start: insert_pos, end: insert_pos, replacement });
                }
                // ---- AOT set (set a key in a specific AOT entry) ----
                OpKind::AotSet => {
                    let target_name = op.key.as_str();
                    let entry_key = op.inline_key.as_deref().ok_or(EditError::InvalidPath)?;
                    let entry_idx = op.index.ok_or(EditError::InvalidPath)?;

                    // Build the full header path for matching
                    let full_target = if op.table.is_empty() {
                        target_name.to_string()
                    } else {
                        format!("{}.{}", op.table.join("."), target_name)
                    };

                    // Scan for ArrayTableOpen sections matching the AOT path
                    let mut entries: Vec<(usize, usize, u32)> = Vec::new(); // (header_start, header_close, section_end_byte)
                    let mut i = 0;
                    while i < doc.spans.len() {
                        if doc.spans[i].kind == SpanKind::ArrayTableOpen {
                            let header_start = i;
                            let mut j = i + 1;
                            let mut path_parts: Vec<String> = Vec::new();
                            while j < doc.spans.len() {
                                match doc.spans[j].kind {
                                    SpanKind::BareKey | SpanKind::BasicString | SpanKind::LiteralString => {
                                        path_parts.push(edit::clean_key_span(&doc.source, &doc.spans[j]).to_string());
                                        j += 1;
                                    }
                                    SpanKind::Dot => { j += 1; }
                                    SpanKind::ArrayTableClose => {
                                        j += 1;
                                        break;
                                    }
                                    _ => break,
                                }
                            }
                            let header_path = path_parts.join(".");
                            if header_path == full_target {
                                let header_close = j - 1; // index of the ArrayTableClose span
                                // Find the end byte of this section
                                let mut k = j;
                                while k < doc.spans.len() {
                                    if doc.spans[k].kind == SpanKind::ArrayTableOpen
                                        || doc.spans[k].kind == SpanKind::ArrayOpen
                                    {
                                        break;
                                    }
                                    k += 1;
                                }
                                let section_end = if k > 0 && k <= doc.spans.len() {
                                    doc.spans[k - 1].end
                                } else {
                                    doc.spans[header_start].end
                                };
                                entries.push((header_start, header_close, section_end));
                            }
                            i = j;
                        } else {
                            i += 1;
                        }
                    }

                    if entry_idx >= entries.len() {
                        return Err(EditError::InvalidPath);
                    }

                    let (_hdr_start, hdr_close, section_end) = entries[entry_idx];

                    // Walk spans from after the header close to find the target key
                    let mut span_i = hdr_close + 1;
                    let mut found = false;
                    while span_i < doc.spans.len() {
                        let span_start = doc.spans[span_i].start;
                        if span_start >= section_end {
                            break;
                        }
                        match doc.spans[span_i].kind {
                            SpanKind::BareKey | SpanKind::BasicString | SpanKind::LiteralString => {
                                // Collect the full dotted key path
                                let mut key_parts: Vec<&str> = Vec::new();
                                key_parts.push(edit::clean_key_span(&doc.source, &doc.spans[span_i]));
                                let mut trail = span_i + 1;
                                while trail < doc.spans.len() && doc.spans[trail].start < section_end {
                                    match doc.spans[trail].kind {
                                        SpanKind::Dot => { trail += 1; }
                                        SpanKind::BareKey | SpanKind::BasicString | SpanKind::LiteralString => {
                                            key_parts.push(edit::clean_key_span(&doc.source, &doc.spans[trail]));
                                            trail += 1;
                                        }
                                        SpanKind::Whitespace | SpanKind::Newline | SpanKind::Comment
                                        | SpanKind::Equals => { break; }
                                        _ => break,
                                    }
                                }
                                let full_key = key_parts.join(".");
                                if full_key == entry_key {
                                    // Find the value: skip past Equals and whitespace
                                    let mut v = trail;
                                    while v < doc.spans.len() && doc.spans[v].start < section_end {
                                        match doc.spans[v].kind {
                                            SpanKind::Equals | SpanKind::Whitespace | SpanKind::Newline | SpanKind::Comment => { v += 1; }
                                            _ => {
                                                if is_value_kind(doc.spans[v].kind) {
                                                    let val_span = doc.spans[v];
                                                    resolved.push(Resolved {
                                                        start: val_span.start,
                                                        end: val_span.end,
                                                        replacement: op.value.clone(),
                                                    });
                                                    found = true;
                                                }
                                                break;
                                            }
                                        }
                                    }
                                    if !found {
                                        return Err(EditError::NotFound);
                                    }
                                    break;
                                }
                                // Skip to end of this key-value pair
                                span_i = trail;
                            }
                            _ => { span_i += 1; }
                        }
                    }

                    if !found {
                        return Err(EditError::NotFound);
                    }
                }
                OpKind::AotRemove => {
                    let target = op.key.as_str();
                    let idx = op.index.ok_or(EditError::InvalidPath)?;

                    let mut sections: Vec<(usize, u32, u32)> = Vec::new();
                    let mut i = 0;
                    while i < doc.spans.len() {
                        if doc.spans[i].kind == SpanKind::ArrayTableOpen {
                            let mut j = i + 1;
                            let mut parts: Vec<String> = Vec::new();
                            while j < doc.spans.len() {
                                match doc.spans[j].kind {
                                    SpanKind::BareKey | SpanKind::BasicString | SpanKind::LiteralString => {
                                        parts.push(edit::clean_key_span(&doc.source, &doc.spans[j]).to_string());
                                        j += 1;
                                    }
                                    SpanKind::Dot => { j += 1; }
                                    SpanKind::ArrayTableClose => { j += 1; break; }
                                    _ => break,
                                }
                            }
                            if parts.join(".") == target {
                                let mut k = j;
                                while k < doc.spans.len() {
                                    if doc.spans[k].kind == SpanKind::ArrayTableOpen
                                        || doc.spans[k].kind == SpanKind::ArrayOpen
                                    { break; }
                                    k += 1;
                                }
                                let end_byte = if k > 0 && k <= doc.spans.len() {
                                    doc.spans[k - 1].end as usize
                                } else {
                                    doc.source.len()
                                };
                                sections.push((i, doc.spans[i].start, end_byte as u32));
                            }
                            i = j;
                        } else {
                            i += 1;
                        }
                    }

                    if idx >= sections.len() {
                        return Err(EditError::InvalidPath);
                    }
                    let (_, remove_start, remove_end) = sections[idx];
                    resolved.push(Resolved { start: remove_start, end: remove_end, replacement: String::new() });
                }
                OpKind::InlineSet => {
                    let (open_idx, close_idx) = resolve_inline_table(doc, &index, op)?;
                    let pairs = walk_inline_pairs(&doc.spans, &doc.source, open_idx, close_idx);
                    let inline_key = op.inline_key.as_deref().ok_or(EditError::InvalidPath)?;
                    let (_key, val_idx, _comma) = pairs.iter()
                        .find(|(k, _, _)| k == inline_key)
                        .ok_or(EditError::NotFound)?;
                    let val_span = doc.spans[*val_idx];
                    resolved.push(Resolved {
                        start: val_span.start,
                        end: val_span.end,
                        replacement: op.value.clone(),
                    });
                }
                OpKind::InlineInsert => {
                    let (open_idx, close_idx) = resolve_inline_table(doc, &index, op)?;
                    let inline_key = op.inline_key.as_deref().ok_or(EditError::InvalidPath)?;
                    let pairs = walk_inline_pairs(&doc.spans, &doc.source, open_idx, close_idx);
                    let close = doc.spans[close_idx];

                    let (ins_pos, text) = if pairs.is_empty() {
                        (close.start, format!("{inline_key} = {}", op.value))
                    } else {
                        (close.start, format!(", {inline_key} = {}", op.value))
                    };
                    resolved.push(Resolved { start: ins_pos, end: ins_pos, replacement: text });
                }
                OpKind::InlineRemove => {
                    let (open_idx, close_idx) = resolve_inline_table(doc, &index, op)?;
                    let inline_key = op.inline_key.as_deref().ok_or(EditError::InvalidPath)?;
                    let pairs = walk_inline_pairs(&doc.spans, &doc.source, open_idx, close_idx);
                    let (idx_in_pairs, (_key, _val_idx, _comma)) = pairs.iter()
                        .enumerate()
                        .find(|(_, (k, _, _))| k == inline_key)
                        .ok_or(EditError::NotFound)?;
                    let remove_start: u32;
                    let remove_end: u32;

                    if idx_in_pairs == 0 {
                        // First pair: from start of key to start of next pair's key (or `}`)
                        let mut key_span_idx = open_idx + 1;
                        while key_span_idx < close_idx {
                            match doc.spans[key_span_idx].kind {
                                SpanKind::BareKey | SpanKind::BasicString | SpanKind::LiteralString => break,
                                _ => { key_span_idx += 1; }
                            }
                        }
                        remove_start = doc.spans[key_span_idx].start;
                        remove_end = if pairs.len() > 1 {
                            // Find start of next pair's key by walking back from its value
                            let (_next_key, next_val_idx, _) = &pairs[1];
                            let mut ks = *next_val_idx;
                            loop {
                                if ks == 0 { break doc.spans[close_idx].start; }
                                ks -= 1;
                                match doc.spans[ks].kind {
                                    SpanKind::BareKey | SpanKind::BasicString | SpanKind::LiteralString => {
                                        break doc.spans[ks].start;
                                    }
                                    SpanKind::Whitespace | SpanKind::Newline | SpanKind::Comment | SpanKind::Comma | SpanKind::Equals => {}
                                    _ => { break doc.spans[ks].start; }
                                }
                            }
                        } else {
                            doc.spans[close_idx].start
                        };
                    } else {
                        // Not first: from after previous pair's comma to this pair's end
                        let (_, _, prev_comma) = &pairs[idx_in_pairs - 1];
                        remove_start = if let Some(comma_idx) = prev_comma {
                            doc.spans[*comma_idx].end
                        } else {
                            doc.spans[pairs[idx_in_pairs - 1].1].end
                        };
                        remove_end = if idx_in_pairs + 1 < pairs.len() {
                            let (_, next_val_idx, _) = &pairs[idx_in_pairs + 1];
                            let mut ks = *next_val_idx;
                            loop {
                                if ks == 0 { break doc.spans[close_idx].start; }
                                ks -= 1;
                                match doc.spans[ks].kind {
                                    SpanKind::BareKey | SpanKind::BasicString | SpanKind::LiteralString => {
                                        break doc.spans[ks].start;
                                    }
                                    _ => {}
                                }
                            }
                        } else {
                            doc.spans[close_idx].start
                        };
                    }

                    resolved.push(Resolved {
                        start: remove_start,
                        end: remove_end,
                        replacement: String::new(),
                    });
                }
                OpKind::InsertSection => {
                    if section_header_exists(&doc.spans, &doc.source, &op.table) {
                        continue;
                    }
                    let insert_pos = doc.spans.last().map(|s| s.end).unwrap_or(0);
                    let sep = detect_blank_line_sep(&doc.spans, &doc.source);
                    let header_name = op.table.join(".");
                    let prefix = op.prefix.as_deref().unwrap_or("");
                    let suffix = op.suffix.as_deref().unwrap_or("");
                    let replacement = if insert_pos == 0 {
                        format!("{prefix}[{header_name}]{suffix}\n")
                    } else {
                        format!("{sep}{prefix}[{header_name}]{suffix}\n")
                    };
                    resolved.push(Resolved { start: insert_pos, end: insert_pos, replacement });
                }
                OpKind::ReplaceSection => {
                    let kv_text: String = op.pairs.iter()
                        .map(|(k, v)| format!("{k} = {v}\n"))
                        .collect();
                    if let Some((_open_idx, close_idx)) = find_section_header(&doc.spans, &doc.source, &op.table) {
                        let header_close = doc.spans[close_idx];
                        let mut content_start = header_close.end as usize;
                        while content_start < doc.source.len() && doc.source.as_bytes()[content_start] != b'\n' {
                            content_start += 1;
                        }
                        if content_start < doc.source.len() { content_start += 1; }
                        let content_end = find_next_section_start(&doc.spans, close_idx)
                            .unwrap_or(doc.source.len() as u32);
                        resolved.push(Resolved { start: content_start as u32, end: content_end, replacement: kv_text });
                    } else {
                        let insert_pos = doc.spans.last().map(|s| s.end).unwrap_or(0);
                        let sep = if insert_pos > 0 { detect_blank_line_sep(&doc.spans, &doc.source) } else { "" };
                        let header_name = op.table.join(".");
                        let replacement = format!("{sep}[{header_name}]\n{kv_text}");
                        resolved.push(Resolved { start: insert_pos, end: insert_pos, replacement });
                    }
                }
                OpKind::ClearSection => {
                    if let Some((_open_idx, close_idx)) = find_section_header(&doc.spans, &doc.source, &op.table) {
                        let header_close = doc.spans[close_idx];
                        let mut content_start = header_close.end as usize;
                        while content_start < doc.source.len() && doc.source.as_bytes()[content_start] != b'\n' {
                            content_start += 1;
                        }
                        if content_start < doc.source.len() { content_start += 1; }
                        let content_end = find_next_section_start(&doc.spans, close_idx)
                            .unwrap_or(doc.source.len() as u32);
                        resolved.push(Resolved { start: content_start as u32, end: content_end, replacement: String::new() });
                    }
                }
                OpKind::RenameSection => {
                    if section_header_exists(&doc.spans, &doc.source, &op.to_table) {
                        return Err(EditError::SectionExists);
                    }
                    let (open_idx, close_idx) = find_section_header(&doc.spans, &doc.source, &op.table)
                        .ok_or(EditError::NotFound)?;
                    let header_start = doc.spans[open_idx].start;
                    let header_end = doc.spans[close_idx].end;
                    let new_name = op.to_table.join(".");
                    let new_header = format!("[{new_name}]");
                    resolved.push(Resolved { start: header_start, end: header_end, replacement: new_header });
                }
                OpKind::RenameKey => {
                    if op.table != op.to_table {
                        return Err(EditError::TableMismatch);
                    }
                    let path: Vec<&str> = op.table.iter().map(|s| s.as_str())
                        .chain(core::iter::once(op.key.as_str())).collect();
                    let entry = index.iter()
                        .find(|(p, _)| path_eq(p, &path))
                        .ok_or(EditError::NotFound)?;
                    let last_key_idx = find_last_bare_key_in_chain(&doc.spans, entry.1.key_start);
                    let key_span = doc.spans[last_key_idx];
                    resolved.push(Resolved { start: key_span.start, end: key_span.end, replacement: op.to_key.clone() });
                }
                OpKind::MoveKey => {
                    let from_path: Vec<&str> = op.table.iter().map(|s| s.as_str())
                        .chain(core::iter::once(op.key.as_str())).collect();
                    let entry = index.iter()
                        .find(|(p, _)| path_eq(p, &from_path))
                        .ok_or(EditError::NotFound)?;
                    let key_span = doc.spans[entry.1.key_start];
                    let value_span = doc.spans[entry.1.value_idx];
                    let mut line_start = key_span.start as usize;
                    while line_start > 0 && doc.source.as_bytes()[line_start - 1] != b'\n' { line_start -= 1; }
                    let mut line_end = value_span.end as usize;
                    while line_end < doc.source.len() && doc.source.as_bytes()[line_end] != b'\n' { line_end += 1; }
                    if line_end < doc.source.len() { line_end += 1; }
                    let line_text = doc.source[line_start..line_end].to_string();

                    let to_table_refs: Vec<&str> = op.to_table.iter().map(|s| s.as_str()).collect();
                    let entries_in_target: Vec<_> = index.iter()
                        .filter(|(p, _)| p.len() == op.to_table.len() + 1
                            && path_eq(&p[..op.to_table.len()], &to_table_refs))
                        .collect();

                    let insert_pos = if let Some((_, last)) = entries_in_target.last() {
                        let last_val_span = doc.spans[last.value_idx];
                        let mut end = last_val_span.end as usize;
                        while end < doc.source.len() && doc.source.as_bytes()[end] != b'\n' { end += 1; }
                        if end < doc.source.len() { end += 1; }
                        end as u32
                    } else if op.to_table.is_empty() {
                        doc.source.len() as u32
                    } else if let Some((_open_idx, close_idx)) = find_section_header(&doc.spans, &doc.source, &op.to_table) {
                        let header_close = doc.spans[close_idx];
                        let mut pos = header_close.end as usize;
                        while pos < doc.source.len() && doc.source.as_bytes()[pos] != b'\n' { pos += 1; }
                        if pos < doc.source.len() { pos += 1; }
                        pos as u32
                    } else {
                        return Err(EditError::NotFound);
                    };

                    // Push remove then insert (order doesn't matter — they're sorted descending)
                    resolved.push(Resolved { start: line_start as u32, end: line_end as u32, replacement: String::new() });
                    resolved.push(Resolved { start: insert_pos, end: insert_pos, replacement: line_text });
                }
                OpKind::PromoteKey => {
                    // Resolve: find key in sub-table, extract its line, insert at root
                    let from_path: Vec<&str> = op.table.iter().map(|s| s.as_str())
                        .chain(core::iter::once(op.key.as_str())).collect();
                    let entry = index.iter()
                        .find(|(p, _)| path_eq(p, &from_path))
                        .ok_or(EditError::NotFound)?;
                    let key_span = doc.spans[entry.1.key_start];
                    let value_span = doc.spans[entry.1.value_idx];

                    // Extract the whole line (key = value, comments, whitespace)
                    let mut line_start = key_span.start as usize;
                    while line_start > 0 && doc.source.as_bytes()[line_start - 1] != b'\n' { line_start -= 1; }
                    let mut line_end = value_span.end as usize;
                    while line_end < doc.source.len() && doc.source.as_bytes()[line_end] != b'\n' { line_end += 1; }
                    if line_end < doc.source.len() { line_end += 1; }
                    let line_text = doc.source[line_start..line_end].to_string();

                    // Insert at root: after the last root-level scalar, or at EOF if
                    // there are no root scalars.  Must go BEFORE any [table] headers
                    // to avoid re-absorption by the TOML spec parser.
                    let root_entries: Vec<_> = index.iter()
                        .filter(|(p, _)| p.len() == 1)
                        .collect();
                    let insert_pos = if let Some((_, last)) = root_entries.last() {
                        let last_val_span = doc.spans[last.value_idx];
                        let mut end = last_val_span.end as usize;
                        while end < doc.source.len() && doc.source.as_bytes()[end] != b'\n' { end += 1; }
                        if end < doc.source.len() { end += 1; }
                        end as u32
                    } else {
                        0 // No root scalars — insert at top of document, before any [table]
                    };
                    resolved.push(Resolved { start: line_start as u32, end: line_end as u32, replacement: String::new() });
                    resolved.push(Resolved { start: insert_pos, end: insert_pos, replacement: line_text });
                }
                OpKind::MoveKeyCreate => {
                    // Check whether destination table exists
                    let to_table_refs: Vec<&str> = op.to_table.iter().map(|s| s.as_str()).collect();
                    let has_dest = if op.to_table.is_empty() {
                        true // promoting to root always works
                    } else {
                        section_header_exists(&doc.spans, &doc.source, &op.to_table)
                    };

                    if has_dest {
                        // Destination exists — same logic as MoveKey
                        let from_path: Vec<&str> = op.table.iter().map(|s| s.as_str())
                            .chain(core::iter::once(op.key.as_str())).collect();
                        let entry = index.iter()
                            .find(|(p, _)| path_eq(p, &from_path))
                            .ok_or(EditError::NotFound)?;
                        let key_span = doc.spans[entry.1.key_start];
                        let value_span = doc.spans[entry.1.value_idx];
                        let mut line_start = key_span.start as usize;
                        while line_start > 0 && doc.source.as_bytes()[line_start - 1] != b'\n' { line_start -= 1; }
                        let mut line_end = value_span.end as usize;
                        while line_end < doc.source.len() && doc.source.as_bytes()[line_end] != b'\n' { line_end += 1; }
                        if line_end < doc.source.len() { line_end += 1; }
                        let line_text = doc.source[line_start..line_end].to_string();

                        let entries_in_target: Vec<_> = index.iter()
                            .filter(|(p, _)| p.len() == op.to_table.len() + 1
                                && path_eq(&p[..op.to_table.len()], &to_table_refs))
                            .collect();
                        let insert_pos = if let Some((_, last)) = entries_in_target.last() {
                            let last_val_span = doc.spans[last.value_idx];
                            let mut end = last_val_span.end as usize;
                            while end < doc.source.len() && doc.source.as_bytes()[end] != b'\n' { end += 1; }
                            if end < doc.source.len() { end += 1; }
                            end as u32
                        } else if op.to_table.is_empty() {
                            // Moving to root, but no root entries exist yet — insert at EOF
                            doc.source.len() as u32
                        } else if let Some((_open_idx, close_idx)) = find_section_header(&doc.spans, &doc.source, &op.to_table) {
                            let header_close = doc.spans[close_idx];
                            let mut pos = header_close.end as usize;
                            while pos < doc.source.len() && doc.source.as_bytes()[pos] != b'\n' { pos += 1; }
                            if pos < doc.source.len() { pos += 1; }
                            pos as u32
                        } else {
                            return Err(EditError::NotFound);
                        };
                        resolved.push(Resolved { start: line_start as u32, end: line_end as u32, replacement: String::new() });
                        resolved.push(Resolved { start: insert_pos, end: insert_pos, replacement: line_text });
                    } else {
                        // Destination table does not exist — create it first, then move key
                        // 1. Move key logic (extract line)
                        let from_path: Vec<&str> = op.table.iter().map(|s| s.as_str())
                            .chain(core::iter::once(op.key.as_str())).collect();
                        let entry = index.iter()
                            .find(|(p, _)| path_eq(p, &from_path))
                            .ok_or(EditError::NotFound)?;
                        let key_span = doc.spans[entry.1.key_start];
                        let value_span = doc.spans[entry.1.value_idx];
                        let mut line_start = key_span.start as usize;
                        while line_start > 0 && doc.source.as_bytes()[line_start - 1] != b'\n' { line_start -= 1; }
                        let mut line_end = value_span.end as usize;
                        while line_end < doc.source.len() && doc.source.as_bytes()[line_end] != b'\n' { line_end += 1; }
                        if line_end < doc.source.len() { line_end += 1; }
                        let line_text = doc.source[line_start..line_end].to_string();

                        // 2. Create new section header at end of document
                        let insert_pos = doc.source.len() as u32;
                        let sep = detect_blank_line_sep(&doc.spans, &doc.source);
                        let header_name = op.to_table.join(".");
                        let header_text = format!("{}[{}]\n", sep, header_name);

                        // 3. Remove old line, insert header + moved key
                        resolved.push(Resolved { start: line_start as u32, end: line_end as u32, replacement: String::new() });
                        resolved.push(Resolved { start: insert_pos, end: insert_pos, replacement: format!("{}{}", header_text, line_text) });
                    }
                }
                OpKind::ReorderRoot => {
                    let desired: Vec<&str> = op.prefix.as_deref().unwrap_or("").split(',').filter(|s| !s.is_empty()).collect();

                    // Collect all root-level blocks: scalar key-value entries AND table headers
                    let mut root_entries: Vec<(String, u32, u32)> = Vec::new();

                    // 1. Scalar key-value entries from the index
                    for (path, entry) in index {
                        if path.len() == 1 {
                            let key_span = doc.spans[entry.key_start];
                            let val_span = doc.spans[entry.value_idx];
                            let mut start = key_span.start as usize;
                            while start > 0 && doc.source.as_bytes()[start - 1] != b'\n' { start -= 1; }
                            let mut end = val_span.end as usize;
                            while end < doc.source.len() && doc.source.as_bytes()[end] != b'\n' { end += 1; }
                            if end < doc.source.len() { end += 1; }
                            root_entries.push((path[0].clone(), start as u32, end as u32));
                        }
                    }

                    // 2. Table headers: find all [section] headers at the root level
                    //    (headers that don't have a parent table — single name like [meta])
                    let mut table_starts: Vec<(String, u32)> = Vec::new(); // (name, header_start)
                    let mut i = 0;
                    while i < doc.spans.len() {
                        if doc.spans[i].kind == SpanKind::ArrayOpen {
                            let mut j = i + 1;
                            let mut parts: Vec<String> = Vec::new();
                            while j < doc.spans.len() {
                                match doc.spans[j].kind {
                                    SpanKind::BareKey | SpanKind::BasicString | SpanKind::LiteralString => {
                                        parts.push(edit::clean_key_span(&doc.source, &doc.spans[j]).to_string());
                                        j += 1;
                                    }
                                    SpanKind::Dot => { j += 1; }
                                    SpanKind::ArrayClose => { break; }
                                    _ => break,
                                }
                            }
                            if !parts.is_empty() {
                                // Use first segment as root name (matches keys() semantics)
                                let name = &parts[0];
                                if !table_starts.iter().any(|(n, _)| n == name) {
                                    table_starts.push((name.clone(), doc.spans[i].start));
                                }
                            }
                        }
                        i += 1;
                    }

                    // 3. Merge scalars and tables into ordered blocks, computing section end
                    //    for each table as the start of the next root entry (or EOF).
                    let mut all_starts: Vec<(String, u32)> = Vec::new(); // (name, start)
                    for (name, start, _) in &root_entries {
                        all_starts.push((name.clone(), *start));
                    }
                    for (name, start) in &table_starts {
                        all_starts.push((name.clone(), *start));
                    }
                    all_starts.sort_by_key(|(_, s)| *s);

                    // For Following anchor: extend each entry's start backward
                    // to include comment lines directly above it (no blank-line gap).
                    let is_following = op.suffix.as_deref() == Some("following");
                    if is_following && all_starts.len() > 1 {
                        for idx in 1..all_starts.len() {
                            let mut pos = all_starts[idx].1 as usize;
                            // Walk back to the previous \n before this entry
                            if pos > 0 { pos -= 1; }
                            while pos > 0 && doc.source.as_bytes()[pos] != b'\n' { pos -= 1; }
                            // Now pos is at a \n. Check if the line above is a comment.
                            // Walk backward, collecting comment lines until a blank line.
                            let mut line_start = pos;
                            while line_start > 0 {
                                let line_end = line_start;
                                line_start -= 1;
                                while line_start > 0 && doc.source.as_bytes()[line_start - 1] != b'\n' { line_start -= 1; }
                                // Check if this line is a comment (leading `#` after optional whitespace)
                                let line = &doc.source[line_start..line_end];
                                let is_comment = line.trim_start().starts_with('#');
                                if is_comment {
                                    // Include this comment line in the following entry
                                    all_starts[idx].1 = line_start as u32;
                                    // Continue upward to previous line
                                } else if line.trim().is_empty() {
                                    // Blank line — stop extending
                                    break;
                                } else {
                                    // Non-comment, non-blank line — stop
                                    break;
                                }
                            }
                        }
                    }

                    root_entries.clear();
                    for idx in 0..all_starts.len() {
                        let (name, start) = &all_starts[idx];
                        let end = if idx + 1 < all_starts.len() {
                            all_starts[idx + 1].1 // end = start of next entry
                        } else {
                            doc.source.len() as u32
                        };
                        root_entries.push((name.clone(), *start, end));
                    }

                    // Find the span covering all root entries
                    let block_start = root_entries.first().map(|(_, s, _)| *s).unwrap_or(0);
                    let block_end = root_entries.last().map(|(_, _, e)| *e).unwrap_or(0);

                    // Build reordered text from desired order
                    let mut new_text = String::new();
                    for name in &desired {
                        if let Some((_, start, end)) = root_entries.iter().find(|(n, _, _)| n == name) {
                            new_text.push_str(&doc.source[*start as usize..*end as usize]);
                        }
                    }
                    // Remove the entire root block and insert reordered text
                    resolved.push(Resolved { start: block_start, end: block_end, replacement: new_text });
                }
            }
        }
        // Sort descending so each splice only affects positions AFTER it.
        // Byte offsets in spans after the earliest splice shift by the
        // total delta — we fix them up in one O(n) pass at the end.
        // span indices stay valid → index survives across commits.
        resolved.sort_by(|a, b| b.start.cmp(&a.start));

        for r in &resolved {
            doc.source.replace_range(r.start as usize..r.end as usize, &r.replacement);
        }

        // One pass over spans: apply accumulated deltas from all splices.
        // Sorted descending ensures later splices don't overlap earlier ones.
        let mut delta: i32 = 0;
        let mut splice_idx = 0;
        for span in &mut doc.spans {
            while splice_idx < resolved.len() && resolved[splice_idx].start <= span.start {
                let r = &resolved[splice_idx];
                delta += r.replacement.len() as i32 - (r.end - r.start) as i32;
                splice_idx += 1;
            }
            span.start = (span.start as i32 + delta) as u32;
            span.end = (span.end as i32 + delta) as u32;
        }
        // Rebuild index — byte-offset-only adjustment is insufficient for
        // Re-parse: span delta pass can't track relocation from move/promote ops.
        // Re-parsing gives us accurate spans aligned with the new source.
        match crate::parse(&doc.source) {
            Ok(reparsed) => {
                doc.spans = reparsed.spans;
                doc.index = None; // let lazy builder rebuild
            }
            // If the edited source is somehow unparseable, keep old spans.
            // This should never happen for valid mutations of valid input.
            Err(_) => {}
        }
        self.ops.clear();
        Ok(())
    }
    pub fn array_push(&mut self, path: &str, value: &str) -> &mut Self {
        let (table, key) = split_path(path);
        self.ops.push(Op {
            kind: OpKind::ArrayPush, table, key, value: value.to_string(),
            prefix: None, suffix: None,
            to_table: Vec::new(), to_key: String::new(), pairs: Vec::new(),
            index: None, inline_key: None,
        });
        self
    }

    /// Set (replace) the value at `index` within the array at `path`.
    pub fn array_set(&mut self, path: &str, index: usize, value: &str) -> &mut Self {
        let (table, key) = split_path(path);
        self.ops.push(Op {
            kind: OpKind::ArraySet, table, key, value: value.to_string(),
            prefix: None, suffix: None,
            to_table: Vec::new(), to_key: String::new(), pairs: Vec::new(),
            index: Some(index), inline_key: None,
        });
        self
    }

    /// Insert `value` at `index` within the array at `path`.
    /// Elements at `index` and beyond are shifted right.
    pub fn array_insert(&mut self, path: &str, index: usize, value: &str) -> &mut Self {
        let (table, key) = split_path(path);
        self.ops.push(Op {
            kind: OpKind::ArrayInsert, table, key, value: value.to_string(),
            prefix: None, suffix: None,
            to_table: Vec::new(), to_key: String::new(), pairs: Vec::new(),
            index: Some(index), inline_key: None,
        });
        self
    }

    /// Remove the element at `index` from the array at `path`.
    pub fn array_remove(&mut self, path: &str, index: usize) -> &mut Self {
        let (table, key) = split_path(path);
        self.ops.push(Op {
            kind: OpKind::ArrayRemove, table, key, value: String::new(),
            prefix: None, suffix: None,
            to_table: Vec::new(), to_key: String::new(), pairs: Vec::new(),
            index: Some(index), inline_key: None,
        });
        self
    }

    // ---- AOT operations ----

    /// Push a new entry to the array of tables at `path`.
    ///
    /// `pairs` provides the key-value pairs for the new entry.
    pub fn aot_push(&mut self, path: &str, pairs: &[(&str, &str)]) -> &mut Self {
        let (table, key) = split_path(path);
        let pairs: Vec<(String, String)> = pairs.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        self.ops.push(Op {
            kind: OpKind::AotPush, table, key, value: String::new(),
            prefix: None, suffix: None,
            to_table: Vec::new(), to_key: String::new(),
            pairs,
            index: None, inline_key: None,
        });
        self
    }

    /// Set (replace) the value for `key` within a specific entry of the
    /// array of tables at `path`.
    ///
    /// `index` is the 0-based index of the `[[path]]` entry.
    /// `key` is the key within that entry whose value to replace.
    pub fn aot_set(&mut self, path: &str, index: usize, key: &str, value: &str) -> &mut Self {
        let (table, aot_key) = split_path(path);
        self.ops.push(Op {
            kind: OpKind::AotSet, table, key: aot_key, value: value.to_string(),
            prefix: None, suffix: None,
            to_table: Vec::new(), to_key: String::new(), pairs: Vec::new(),
            index: Some(index), inline_key: Some(key.to_string()),
        });
        self
    }

    /// Remove the `index`-th `[[entry]]` from the array-of-tables at `path`.
    ///
    /// This deletes the entire section including its header line and all
    /// key-value pairs.  Indices are 0-based.  After removal, subsequent
    /// entries shift down, so a second `aot_remove` with the same index
    /// removes the new occupant.
    ///
    /// Returns [`EditError::InvalidPath`] if `path` is not an AOT or
    /// `index` is out of bounds.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // [[server]]\nhost = "a"\n[[server]]\nhost = "b"\n
    /// doc.edit().aot_remove("server", 0).commit()?;
    /// // Only "b" remains.
    /// ```
    pub fn aot_remove(&mut self, path: &str, index: usize) -> &mut Self {
        let (table, aot_key) = split_path(path);
        self.ops.push(Op {
            kind: OpKind::AotRemove, table, key: aot_key, value: String::new(),
            prefix: None, suffix: None,
            to_table: Vec::new(), to_key: String::new(), pairs: Vec::new(),
            index: Some(index), inline_key: None,
        });
        self
    }

    // ---- Inline table operations ----

    /// Set (replace) the value for `key` within the inline table at `path`.
    pub fn inline_set(&mut self, path: &str, key: &str, value: &str) -> &mut Self {
        let (table, inline_key) = split_path(path);
        self.ops.push(Op {
            kind: OpKind::InlineSet, table, key: inline_key, value: value.to_string(),
            prefix: None, suffix: None,
            to_table: Vec::new(), to_key: String::new(), pairs: Vec::new(),
            index: None, inline_key: Some(key.to_string()),
        });
        self
    }

    /// Insert `key = value` into the inline table at `path`.
    pub fn inline_insert(&mut self, path: &str, key: &str, value: &str) -> &mut Self {
        let (table, inline_key) = split_path(path);
        self.ops.push(Op {
            kind: OpKind::InlineInsert, table, key: inline_key, value: value.to_string(),
            prefix: None, suffix: None,
            to_table: Vec::new(), to_key: String::new(), pairs: Vec::new(),
            index: None, inline_key: Some(key.to_string()),
        });
        self
    }

    /// Remove `key` and its value from the inline table at `path`.
    pub fn inline_remove(&mut self, path: &str, key: &str) -> &mut Self {
        let (table, inline_key) = split_path(path);
        self.ops.push(Op {
            kind: OpKind::InlineRemove, table, key: inline_key, value: String::new(),
            prefix: None, suffix: None,
            to_table: Vec::new(), to_key: String::new(), pairs: Vec::new(),
            index: None, inline_key: Some(key.to_string()),
        });
        self
    }

    // ---- Section / table operations ----

    /// Queue insertion of a new `[section]` header.
    ///
    /// If the section already exists this is a no-op.  A blank-line
    /// separator is inserted before the header, matching the convention
    /// used by existing sections in the document.
    ///
    /// Use [`.with_above_comment()`](Editor::with_above_comment) or
    /// [`.with_block_comment()`](Editor::with_block_comment) to attach
    /// comments above the new section.
    pub fn insert_section(&mut self, path: &str) -> &mut Self {
        let table: Vec<String> = path.split('.').map(|s| s.to_string()).filter(|s| !s.is_empty()).collect();
        self.ops.push(Op {
            kind: OpKind::InsertSection,
            table,
            key: String::new(),
            value: String::new(),
            prefix: None, suffix: None,
            to_table: Vec::new(), to_key: String::new(), pairs: Vec::new(),
            index: None, inline_key: None,
        });
        self
    }

    /// Queue replacement of all keys in a section.
    ///
    /// Existing keys are removed (the header is kept) and replaced with
    /// the provided key-value pairs.  If the section does not exist it
    /// is created first.
    pub fn replace_section(&mut self, path: &str, pairs: &[(&str, &str)]) -> &mut Self {
        let table: Vec<String> = path.split('.').map(|s| s.to_string()).filter(|s| !s.is_empty()).collect();
        let pairs: Vec<(String, String)> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        self.ops.push(Op {
            kind: OpKind::ReplaceSection,
            table,
            key: String::new(),
            value: String::new(),
            prefix: None, suffix: None,
            to_table: Vec::new(), to_key: String::new(),
            pairs,
            index: None, inline_key: None,
        });
        self
    }

    /// Queue removal of all keys from a section, keeping the header and
    /// its comments.
    pub fn clear_section(&mut self, path: &str) -> &mut Self {
        let table: Vec<String> = path.split('.').map(|s| s.to_string()).filter(|s| !s.is_empty()).collect();
        self.ops.push(Op {
            kind: OpKind::ClearSection,
            table,
            key: String::new(),
            value: String::new(),
            prefix: None, suffix: None,
            to_table: Vec::new(), to_key: String::new(), pairs: Vec::new(),
            index: None, inline_key: None,
        });
        self
    }

    /// Queue renaming of a section from `from` to `to`.
    ///
    /// Returns an error at commit time if the target section already
    /// exists.
    pub fn rename_section(&mut self, from: &str, to: &str) -> &mut Self {
        let from_table: Vec<String> = from.split('.').map(|s| s.to_string()).filter(|s| !s.is_empty()).collect();
        let to_table: Vec<String> = to.split('.').map(|s| s.to_string()).filter(|s| !s.is_empty()).collect();
        self.ops.push(Op {
            kind: OpKind::RenameSection,
            table: from_table,
            key: String::new(),
            value: String::new(),
            prefix: None, suffix: None,
            to_table,
            to_key: String::new(),
            pairs: Vec::new(),
            index: None, inline_key: None,
        });
        self
    }

    /// Queue renaming of a key within the same section.
    ///
    /// Both `from` and `to` are dotted paths whose table prefix must
    /// match.  The value and any surrounding comments are preserved.
    pub fn rename_key(&mut self, from: &str, to: &str) -> &mut Self {
        let (from_table, from_key) = split_path(from);
        let (to_table, to_key) = split_path(to);
        self.ops.push(Op {
            kind: OpKind::RenameKey,
            table: from_table,
            key: from_key,
            value: String::new(),
            prefix: None, suffix: None,
            to_table,
            to_key,
            pairs: Vec::new(),
            index: None, inline_key: None,
        });
        self
    }

    /// Queue moving of a key from one section to another.
    ///
    /// Both `from` and `to` are dotted paths.  The key text (including
    /// inline comments and formatting) is preserved during the move.
    pub fn move_key(&mut self, from: &str, to: &str) -> &mut Self {
        let (from_table, from_key) = split_path(from);
        let (to_table, to_key) = split_path(to);
        self.ops.push(Op {
            kind: OpKind::MoveKey,
            table: from_table,
            key: from_key,
            value: String::new(),
            prefix: None, suffix: None,
            to_table,
            to_key,
            pairs: Vec::new(),
            index: None, inline_key: None,
        });
        self
    }

    ///
    /// Promote a key from a sub-table to the document root.
    ///
    /// This is the self-healing primitive for the TOML spec footgun where
    /// scalars placed after a `[table]` header are silently absorbed into
    /// that table.  `promote_key("meta.base")` extracts `base` from
    /// `[meta]` and inserts it at the root level, preserving its value,
    /// inline comments, and formatting.
    ///
    /// The leaf key name is preserved; only the table prefix is stripped.
    /// If a key with the same name already exists at the root level the
    /// document will contain a duplicate key after commit — callers
    /// should validate (or `remove` the target first) if avoiding
    /// duplicates matters.
    ///
    /// # Panics
    ///
    /// Panics if `from` is already a root-level key (no dots).
    /// Promoting a root key is a no-op but confusing — this signals
    /// a caller bug.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Input:  [meta]\nname = "Test"\nbase = "my-base"
    /// // Parser: `base` → meta.base (absorbed per TOML spec)
    /// doc.edit().promote_key("meta.base").commit()?;
    /// // Result: base = "my-base" at root, [meta] keeps only `name`
    /// ```
    pub fn promote_key(&mut self, from: &str) -> &mut Self {
        let (from_table, from_key) = split_path(from);
        assert!(!from_table.is_empty(), "promote_key requires a dotted path, e.g. meta.base — not a bare root key");
        self.ops.push(Op {
            kind: OpKind::PromoteKey,
            table: from_table,
            key: from_key,
            value: String::new(),
            prefix: None, suffix: None,
            to_table: Vec::new(), to_key: String::new(),
            pairs: Vec::new(),
            index: None, inline_key: None,
        });
        self
    }

    ///
    /// Move a key from one section to another, auto-creating the
    /// destination table if it does not already exist.
    ///
    /// Like [`move_key`](Editor::move_key), but when the target table
    /// doesn't have a `[section]` header in the document, one is
    /// created before the key is inserted.  The header is appended
    /// at the end of the document, using the prevailing blank-line
    /// convention.
    ///
    /// If the target table already exists, this is identical to
    /// `move_key(from, to)`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// doc.edit().move_key_create("meta.name", "game.name").commit()?;
    /// // If [game] didn't exist: creates `[game]` header, then inserts `name`
    /// // If [game] already exists: same behaviour as move_key
    /// ```
    pub fn move_key_create(&mut self, from: &str, to: &str) -> &mut Self {
        let (from_table, from_key) = split_path(from);
        let (to_table, to_key) = split_path(to);
        self.ops.push(Op {
            kind: OpKind::MoveKeyCreate,
            table: from_table,
            key: from_key,
            value: String::new(),
            prefix: None, suffix: None,
            to_table,
            to_key,
            pairs: Vec::new(),
            index: None, inline_key: None,
        });
        self
    }
}

// ============================================================
// Commit helpers
// ============================================================

/// Check whether a section header `[path]` exists in the spans.
fn section_header_exists(spans: &[Span], source: &str, path: &[String]) -> bool {
    find_section_header(spans, source, path).is_some()
}

/// Find a section header `[path]` in the spans.
/// Returns `(array_open_idx, array_close_idx)`.
fn find_section_header(spans: &[Span], source: &str, path: &[String]) -> Option<(usize, usize)> {
    let mut i = 0;
    while i < spans.len() {
        if spans[i].kind == SpanKind::ArrayOpen {
            let start = i;
            let mut j = i + 1;
            let mut hdr: Vec<String> = Vec::new();
            while j < spans.len() {
                match spans[j].kind {
                    SpanKind::BareKey | SpanKind::BasicString | SpanKind::LiteralString => {
                        hdr.push(edit::clean_key_span(source, &spans[j]).to_string());
                        j += 1;
                    }
                    SpanKind::Dot => { j += 1; }
                    SpanKind::ArrayClose => {
                        if hdr == path {
                            return Some((start, j));
                        }
                        break;
                    }
                    _ => { break; }
                }
            }
        }
        i += 1;
    }
    None
}

/// Find the byte position where the next section header starts after
/// the given span index.
fn find_next_section_start(spans: &[Span], after_idx: usize) -> Option<u32> {
    for i in (after_idx + 1)..spans.len() {
        if spans[i].kind == SpanKind::ArrayOpen {
            return Some(spans[i].start);
        }
    }
    None
}

/// Detect the blank-line convention used between sections.
/// Returns `"\n\n"` if any section is preceded by a blank line,
/// otherwise `"\n"`.
fn detect_blank_line_sep(spans: &[Span], source: &str) -> &'static str {
    for span in spans {
        if span.kind == SpanKind::ArrayOpen {
            let pos = span.start as usize;
            if pos >= 2 && source.as_bytes()[pos - 1] == b'\n' && source.as_bytes()[pos - 2] == b'\n' {
                return "\n\n";
            }
        }
    }
    "\n"
}

/// Find the index of the last bare-key or quoted-key span in a
/// possibly-dotted key chain starting at `key_start_idx`.
fn find_last_bare_key_in_chain(spans: &[Span], key_start_idx: usize) -> usize {
    let mut last_key = key_start_idx;
    let mut i = key_start_idx + 1;
    while i < spans.len() {
        match spans[i].kind {
            SpanKind::Dot => { i += 1; }
            SpanKind::BareKey | SpanKind::BasicString | SpanKind::LiteralString => {
                last_key = i;
                i += 1;
            }
            _ => break,
        }
    }
    last_key
}

// ============================================================
// EditorHandle — fluent entry point via FlatDoc::edit()
// ============================================================

/// A scoped handle that bundles an [`Editor`] with the [`FlatDoc`] it
/// will mutate.
///
/// Obtained via [`FlatDoc::edit`](crate::FlatDoc::edit).  Operations
/// are chained fluently and committed in one call.
///
/// # Example
///
/// ```ignore
/// doc.edit()
///    .set("package.version", "\"2.0.0\"")
///    .insert("package", "license", "\"MIT\"")
///    .commit()?;
/// ```
pub struct EditorHandle<'a> {
    pub(crate) doc: &'a mut FlatDoc,
    pub(crate) editor: Editor,
}

impl<'a> EditorHandle<'a> {
    /// Queue a set (replace) operation.
    ///
    /// See [`Editor::set`] for details.
    pub fn set(&mut self, path: &str, value: &str) -> &mut Self {
        self.editor.set(path, value);
        self
    }

    /// Queue an insert operation.
    ///
    /// See [`Editor::insert`] for details.
    pub fn insert(&mut self, table_path: &str, key: &str, value: &str) -> &mut Self {
        self.editor.insert(table_path, key, value);
        self
    }

    /// Queue a remove operation.
    ///
    /// See [`Editor::remove`] for details.
    pub fn remove(&mut self, path: &str) -> &mut Self {
        self.editor.remove(path);
        self
    }

    /// Attach a literal prefix to the last queued operation.
    ///
    /// See [`Editor::with_prefix`] for details.
    pub fn with_prefix(&mut self, prefix: &str) -> &mut Self {
        self.editor.with_prefix(prefix);
        self
    }

    /// Attach a literal suffix to the last queued operation.
    ///
    /// See [`Editor::with_suffix`] for details.
    pub fn with_suffix(&mut self, suffix: &str) -> &mut Self {
        self.editor.with_suffix(suffix);
        self
    }

    /// Attach a comment line above the last queued operation.
    ///
    /// See [`Editor::with_above_comment`] for details.
    pub fn with_above_comment(&mut self, text: &str) -> &mut Self {
        self.editor.with_above_comment(text);
        self
    }

    /// Attach multiple comment lines above the last queued operation.
    ///
    /// See [`Editor::with_block_comment`] for details.
    pub fn with_block_comment(&mut self, lines: &[&str]) -> &mut Self {
        self.editor.with_block_comment(lines);
        self
    }

    /// Append a value to an array at `path`.
    ///
    /// See [`Editor::array_push`] for details.
    pub fn array_push(&mut self, path: &str, value: &str) -> &mut Self {
        self.editor.array_push(path, value);
        self
    }

    /// Set (replace) a value at an index in an array.
    ///
    /// See [`Editor::array_set`] for details.
    pub fn array_set(&mut self, path: &str, index: usize, value: &str) -> &mut Self {
        self.editor.array_set(path, index, value);
        self
    }

    /// Insert a value at an index in an array.
    ///
    /// See [`Editor::array_insert`] for details.
    pub fn array_insert(&mut self, path: &str, index: usize, value: &str) -> &mut Self {
        self.editor.array_insert(path, index, value);
        self
    }

    /// Remove an element at an index from an array.
    ///
    /// See [`Editor::array_remove`] for details.
    pub fn array_remove(&mut self, path: &str, index: usize) -> &mut Self {
        self.editor.array_remove(path, index);
        self
    }

    /// Push a new entry to an array of tables.
    ///
    /// See [`Editor::aot_push`] for details.
    pub fn aot_push(&mut self, path: &str, pairs: &[(&str, &str)]) -> &mut Self {
        self.editor.aot_push(path, pairs);
        self
    }

    /// Set (replace) a key's value within a specific AOT entry.
    ///
    /// See [`Editor::aot_set`] for details.
    pub fn aot_set(&mut self, path: &str, index: usize, key: &str, value: &str) -> &mut Self {
        self.editor.aot_set(path, index, key, value);
        self
    }

    /// Queue removing an `[[entry]]` from an array-of-tables.
    ///
    /// See [`Editor::aot_remove`] for details.
    pub fn aot_remove(&mut self, path: &str, index: usize) -> &mut Self {
        self.editor.aot_remove(path, index);
        self
    }

    /// Queue setting a value in an inline table.
    ///
    /// See [`Editor::inline_set`] for details.
    pub fn inline_set(&mut self, path: &str, key: &str, value: &str) -> &mut Self {
        self.editor.inline_set(path, key, value);
        self
    }

    /// Insert a key-value pair into an inline table.
    ///
    /// See [`Editor::inline_insert`] for details.
    pub fn inline_insert(&mut self, path: &str, key: &str, value: &str) -> &mut Self {
        self.editor.inline_insert(path, key, value);
        self
    }

    /// Remove a key-value pair from an inline table.
    ///
    /// See [`Editor::inline_remove`] for details.
    pub fn inline_remove(&mut self, path: &str, key: &str) -> &mut Self {
        self.editor.inline_remove(path, key);
        self
    }

    /// Queue insertion of a new `[section]` header.
    ///
    /// See [`Editor::insert_section`] for details.
    pub fn insert_section(&mut self, path: &str) -> &mut Self {
        self.editor.insert_section(path);
        self
    }

    /// Queue replacement of all keys in a section.
    ///
    /// See [`Editor::replace_section`] for details.
    pub fn replace_section(&mut self, path: &str, pairs: &[(&str, &str)]) -> &mut Self {
        self.editor.replace_section(path, pairs);
        self
    }

    /// Queue removal of all keys from a section.
    ///
    /// See [`Editor::clear_section`] for details.
    pub fn clear_section(&mut self, path: &str) -> &mut Self {
        self.editor.clear_section(path);
        self
    }

    /// Queue renaming of a section.
    ///
    /// See [`Editor::rename_section`] for details.
    pub fn rename_section(&mut self, from: &str, to: &str) -> &mut Self {
        self.editor.rename_section(from, to);
        self
    }

    /// Queue renaming of a key within the same section.
    ///
    /// See [`Editor::rename_key`] for details.
    pub fn rename_key(&mut self, from: &str, to: &str) -> &mut Self {
        self.editor.rename_key(from, to);
        self
    }

    /// Queue moving of a key from one section to another.
    ///
    /// See [`Editor::move_key`] for details.
    pub fn move_key(&mut self, from: &str, to: &str) -> &mut Self {
        self.editor.move_key(from, to);
        self
    }

    /// Queue reordering of root-level entries.
    ///
    /// See [`Editor::reorder_root`] for details.
    pub fn reorder_root(&mut self, order: &[&str]) -> &mut Self {
        self.editor.reorder_root(order);
        self
    }

    /// Queue reordering with explicit comment association.
    ///
    /// See [`Editor::reorder_root_anchored`] for details.
    pub fn reorder_root_anchored(&mut self, order: &[&str], anchor: CommentAnchor) -> &mut Self {
        self.editor.reorder_root_anchored(order, anchor);
        self
    }

    /// Queue promoting of a key from a sub-table to the document root.
    ///
    /// See [`Editor::promote_key`] for details.
    pub fn promote_key(&mut self, from: &str) -> &mut Self {
        self.editor.promote_key(from);
        self
    }

    /// Queue moving of a key to another section, auto-creating the
    /// destination table if needed.
    ///
    /// See [`Editor::move_key_create`] for details.
    pub fn move_key_create(&mut self, from: &str, to: &str) -> &mut Self {
        self.editor.move_key_create(from, to);
        self
    }

    /// Apply all queued operations to the document and clear the queue.
    /// See [`Editor::commit`] for details.
    pub fn commit(&mut self) -> Result<(), EditError> {
        self.editor.commit(self.doc)
    }
}