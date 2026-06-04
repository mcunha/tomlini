//! Validation pass for `FlatDoc`.
//!
//! Validates structural TOML rules, duplicate keys, table conflicts,
//! AOT ordering, and (in strict mode) control characters and key syntax.

use crate::edit;
use crate::{FlatDoc, SpanKind};

#[cfg(not(feature = "std"))]
use alloc::collections::BTreeMap;
#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::collections::BTreeMap;

/// Validation strictness level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationMode {
    /// Accept everything, no validation (current behavior)
    Lenient,
    /// Check structural TOML rules, accept INI extensions (`;` comments, loose quoting)
    Relaxed,
    /// Full TOML 1.1.0 spec compliance
    Strict,
}

/// The kind of validation problem found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationErrorKind {
    DuplicateKey,
    TableConflict,
    AotOrdering,
    ControlCharacter,
    InvalidKey,
    MissingValue,
}

/// A single validation error with position and description.
#[derive(Debug)]
pub struct ValidationError {
    pub pos: usize,
    pub kind: ValidationErrorKind,
    pub msg: String,
}

/// Run the full validation suite against a document.
pub(crate) fn validate(doc: &FlatDoc, mode: ValidationMode) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    let index = edit::build_index(doc);

    // Always check: duplicate keys (and duplicate tables)
    check_duplicate_keys(doc, &index, &mut errors);

    if mode == ValidationMode::Relaxed || mode == ValidationMode::Strict {
        // Check: table conflicts (defining table after using as scalar)
        check_table_conflicts(doc, &index, &mut errors);
        // Check: AOT ordering
        check_aot_ordering(doc, &mut errors);
    }

    if mode == ValidationMode::Strict {
        // Check: control characters in keys/values
        check_control_characters(doc, &mut errors);
        // Check: invalid key syntax
        check_key_syntax(doc, &mut errors);
        // Check: missing values
        check_missing_values(doc, &mut errors);
    }

    errors
}

// =========================================================================
// Span-walk helper: ordered walk collecting structural info
// =========================================================================

struct SpanWalk {
    table_headers: Vec<Vec<String>>,
    entries: Vec<(Vec<String>, usize)>,
    /// Implicit tables: dotted-key prefixes NOT covered by an explicit header.
    implicit_tables: Vec<(Vec<String>, usize)>,
}

/// Walk spans in order, tracking current table scope.
fn walk_spans(doc: &FlatDoc) -> SpanWalk {
    let mut table_headers: Vec<Vec<String>> = Vec::new();
    let mut entries: Vec<(Vec<String>, usize)> = Vec::new();
    let mut implicit_tables: Vec<(Vec<String>, usize)> = Vec::new();
    let mut current_table: Vec<String> = Vec::new();
    let mut i = 0;

    while i < doc.spans.len() {
        let span = doc.spans[i];

        match span.kind {
            SpanKind::ArrayOpen | SpanKind::ArrayTableOpen => {
                let mut path: Vec<String> = Vec::with_capacity(4);
                i += 1;
                while i < doc.spans.len() {
                    match doc.spans[i].kind {
                        SpanKind::BareKey | SpanKind::BasicString | SpanKind::LiteralString => {
                            let key = edit::clean_key(&doc.source, &doc.spans[i]);
                            path.push(key.to_string());
                            i += 1;
                        }
                        SpanKind::Dot => {
                            i += 1;
                        }
                        SpanKind::ArrayClose | SpanKind::ArrayTableClose => {
                            i += 1;
                            break;
                        }
                        _ => {
                            i += 1;
                            break;
                        }
                    }
                }
                if !path.is_empty() {
                    table_headers.push(path.clone());
                    current_table = path;
                }
                continue;
            }

            SpanKind::BareKey | SpanKind::BasicString | SpanKind::LiteralString => {
                let mut key_parts: Vec<String> = Vec::with_capacity(4);
                key_parts.push(edit::clean_key(&doc.source, &span).to_string());
                let key_start = i;
                let mut j = i + 1;

                loop {
                    if j >= doc.spans.len() {
                        break;
                    }
                    match doc.spans[j].kind {
                        SpanKind::Whitespace | SpanKind::Newline | SpanKind::Comment => {
                            j += 1;
                        }
                        SpanKind::Dot => {
                            j += 1;
                        }
                        SpanKind::BareKey | SpanKind::BasicString | SpanKind::LiteralString => {
                            key_parts.push(edit::clean_key(&doc.source, &doc.spans[j]).to_string());
                            j += 1;
                        }
                        SpanKind::Equals => {
                            let mut full_path: Vec<String> =
                                Vec::with_capacity(current_table.len() + key_parts.len());
                            full_path.extend_from_slice(&current_table);
                            full_path.extend(key_parts.clone());

                            entries.push((full_path.clone(), key_start));

                            // Track implicit tables from dotted keys.
                            // A prefix is implicit if there are multiple key parts
                            // AND the prefix is not an explicit table header.
                            if key_parts.len() > 1 {
                                let prefix_base_len = current_table.len();
                                for prefix_extra in 1..key_parts.len() {
                                    let prefix_len = prefix_base_len + prefix_extra;
                                    let prefix: Vec<String> = full_path[..prefix_len].to_vec();
                                    if !table_headers.contains(&prefix)
                                        && !implicit_tables.iter().any(|(p, _)| p == &prefix)
                                    {
                                        implicit_tables.push((prefix, key_start));
                                    }
                                }
                            }

                            // Skip past the value
                            j += 1;
                            let mut k = j;
                            while k < doc.spans.len() {
                                if is_value_kind(doc.spans[k].kind) {
                                    i = k;
                                    break;
                                }
                                match doc.spans[k].kind {
                                    SpanKind::Whitespace
                                    | SpanKind::Newline
                                    | SpanKind::Comment => {
                                        k += 1;
                                    }
                                    _ => {
                                        break;
                                    }
                                }
                            }
                            break;
                        }
                        _ => {
                            break;
                        }
                    }
                }
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    SpanWalk {
        table_headers,
        entries,
        implicit_tables,
    }
}

fn is_value_kind(k: SpanKind) -> bool {
    matches!(
        k,
        SpanKind::Integer
            | SpanKind::Float
            | SpanKind::Boolean
            | SpanKind::Datetime
            | SpanKind::BasicString
            | SpanKind::LiteralString
            | SpanKind::MlBasicString
            | SpanKind::MlLiteralString
            | SpanKind::InlineTableOpen
            | SpanKind::ArrayOpen
    )
}

// ---------------------------------------------------------------------------
// check_duplicate_keys
// ---------------------------------------------------------------------------

fn check_duplicate_keys(
    doc: &FlatDoc,
    index: &[(Vec<String>, edit::Entry)],
    errors: &mut Vec<ValidationError>,
) {
    let walk = walk_spans(doc);

    // 1. Duplicate key-value entries (from index)
    let mut seen: BTreeMap<&[String], usize> = BTreeMap::new();
    for (i, (path, entry)) in index.iter().enumerate() {
        if let Some(prev_idx) = seen.get(path.as_slice()) {
            let prev_entry = &index[*prev_idx].1;
            errors.push(ValidationError {
                pos: entry.key_start,
                kind: ValidationErrorKind::DuplicateKey,
                msg: format!(
                    "duplicate key `{}` (first defined at span index {})",
                    path.join("."),
                    prev_entry.key_start,
                ),
            });
        } else {
            seen.insert(path.as_slice(), i);
        }
    }

    // 2. Duplicate explicit table/AOT headers
    let mut seen_tables: BTreeMap<&[String], usize> = BTreeMap::new();
    for (seq_idx, path) in walk.table_headers.iter().enumerate() {
        if let Some(prev_seq) = seen_tables.get(path.as_slice()) {
            errors.push(ValidationError {
                pos: 0,
                kind: ValidationErrorKind::DuplicateKey,
                msg: format!(
                    "duplicate table `[{}]` (first defined as occurrence #{})",
                    path.join("."),
                    prev_seq + 1,
                ),
            });
        } else {
            seen_tables.insert(path.as_slice(), seq_idx);
        }
    }
}

// ---------------------------------------------------------------------------
// check_table_conflicts
// ---------------------------------------------------------------------------

/// Detect table conflicts:
/// 1. Scalar-then-subtable: `a = 1` then `[a.b]`
/// 2. Implicit-then-explicit: `a.b = 1` then `[a]`
/// 3. Same path as both scalar and table
fn check_table_conflicts(
    doc: &FlatDoc,
    index: &[(Vec<String>, edit::Entry)],
    errors: &mut Vec<ValidationError>,
) {
    let walk = walk_spans(doc);

    // Conflict: scalar path vs table header path
    for (scalar_path, entry) in index {
        for table_path in &walk.table_headers {
            if scalar_path == table_path {
                errors.push(ValidationError {
                    pos: entry.key_start,
                    kind: ValidationErrorKind::TableConflict,
                    msg: format!(
                        "key `{}` is defined as both a value and a table",
                        scalar_path.join("."),
                    ),
                });
            } else if table_path.len() > scalar_path.len()
                && table_path.starts_with(scalar_path.as_slice())
            {
                errors.push(ValidationError {
                    pos: entry.key_start,
                    kind: ValidationErrorKind::TableConflict,
                    msg: format!(
                        "key `{}` used as a value conflicts with table `{}`",
                        scalar_path.join("."),
                        table_path.join("."),
                    ),
                });
            }
        }
    }

    // Conflict: implicit table from dotted key vs explicit table header.
    // e.g. `a.b = 1` then `[a]` — dotted key creates implicit table "a",
    // then explicit table header redefines it.
    for (imp_path, key_start) in &walk.implicit_tables {
        if walk.table_headers.iter().any(|th| th == imp_path) {
            // Find the dotted-key entry that created this implicit table
            if let Some((entry_path, _)) = walk
                .entries
                .iter()
                .find(|(ep, _)| ep.starts_with(imp_path.as_slice()) && ep.len() > imp_path.len())
            {
                errors.push(ValidationError {
                    pos: *key_start,
                    kind: ValidationErrorKind::TableConflict,
                    msg: format!(
                        "table `[{}]` conflicts with implicit table from dotted key `{}`",
                        imp_path.join("."),
                        entry_path.join("."),
                    ),
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// check_aot_ordering
// ---------------------------------------------------------------------------

fn check_aot_ordering(doc: &FlatDoc, errors: &mut Vec<ValidationError>) {
    let mut aots: Vec<(usize, Vec<String>)> = Vec::new();
    let mut i = 0;
    while i < doc.spans.len() {
        if doc.spans[i].kind == SpanKind::ArrayTableOpen {
            let mut path: Vec<String> = Vec::with_capacity(4);
            i += 1;
            while i < doc.spans.len() {
                match doc.spans[i].kind {
                    SpanKind::BareKey | SpanKind::BasicString | SpanKind::LiteralString => {
                        let key = edit::clean_key(&doc.source, &doc.spans[i]);
                        path.push(key.to_string());
                        i += 1;
                    }
                    SpanKind::Dot => {
                        i += 1;
                    }
                    SpanKind::ArrayTableClose => {
                        i += 1;
                        break;
                    }
                    _ => {
                        i += 1;
                        break;
                    }
                }
            }
            aots.push((aots.len(), path));
            continue;
        }
        i += 1;
    }

    let mut groups: BTreeMap<&[String], Vec<usize>> = BTreeMap::new();
    for (seq_idx, path) in &aots {
        groups.entry(path.as_slice()).or_default().push(*seq_idx);
    }
    for (path, indices) in &groups {
        if indices.len() <= 1 {
            continue;
        }
        for w in indices.windows(2) {
            if w[0] + 1 != w[1] {
                errors.push(ValidationError {
                    pos: 0,
                    kind: ValidationErrorKind::AotOrdering,
                    msg: format!(
                        "array-of-tables `[[{}]]` entries are not consecutive",
                        path.join("."),
                    ),
                });
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// check_control_characters
// ---------------------------------------------------------------------------

fn check_control_characters(doc: &FlatDoc, errors: &mut Vec<ValidationError>) {
    let source = doc.source.as_bytes();
    for (i, &b) in source.iter().enumerate() {
        if b < 0x20 && b != b'\t' && b != b'\n' && b != b'\r' {
            errors.push(ValidationError {
                pos: i,
                kind: ValidationErrorKind::ControlCharacter,
                msg: format!("control character U+{:04X} is not valid in TOML", b),
            });
        } else if b == 0x7F {
            errors.push(ValidationError {
                pos: i,
                kind: ValidationErrorKind::ControlCharacter,
                msg: "control character U+007F is not valid in TOML".to_string(),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// check_key_syntax
// ---------------------------------------------------------------------------

fn check_key_syntax(doc: &FlatDoc, errors: &mut Vec<ValidationError>) {
    for span in &doc.spans {
        if span.kind != SpanKind::BareKey {
            continue;
        }
        let text = &doc.source[span.start as usize..span.end as usize];
        if text.is_empty() {
            errors.push(ValidationError {
                pos: span.start as usize,
                kind: ValidationErrorKind::InvalidKey,
                msg: "bare key must not be empty".to_string(),
            });
        } else if !text
            .bytes()
            .all(|b| matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_'))
        {
            errors.push(ValidationError {
                pos: span.start as usize,
                kind: ValidationErrorKind::InvalidKey,
                msg: format!("bare key `{}` contains invalid characters", text),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// check_missing_values
// ---------------------------------------------------------------------------

fn check_missing_values(doc: &FlatDoc, errors: &mut Vec<ValidationError>) {
    let mut i = 0;
    while i < doc.spans.len() {
        if doc.spans[i].kind == SpanKind::Equals {
            let mut j = i + 1;
            while j < doc.spans.len() {
                match doc.spans[j].kind {
                    SpanKind::Whitespace | SpanKind::Newline | SpanKind::Comment => {
                        j += 1;
                    }
                    _ => break,
                }
            }
            if j >= doc.spans.len() {
                errors.push(ValidationError {
                    pos: doc.spans[i].start as usize,
                    kind: ValidationErrorKind::MissingValue,
                    msg: "key has no value".to_string(),
                });
            } else {
                let next_kind = doc.spans[j].kind;
                let is_value = matches!(
                    next_kind,
                    SpanKind::Integer
                        | SpanKind::Float
                        | SpanKind::Boolean
                        | SpanKind::Datetime
                        | SpanKind::BasicString
                        | SpanKind::LiteralString
                        | SpanKind::MlBasicString
                        | SpanKind::MlLiteralString
                        | SpanKind::InlineTableOpen
                        | SpanKind::ArrayOpen
                );
                if !is_value {
                    errors.push(ValidationError {
                        pos: doc.spans[i].start as usize,
                        kind: ValidationErrorKind::MissingValue,
                        msg: "key has no value".to_string(),
                    });
                }
            }
        }
        i += 1;
    }
}
