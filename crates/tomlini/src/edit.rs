//! SAX-style edit operations on `FlatDoc`.
//!
//! Operates directly on the source string and span index — no DOM, no tree.
//! All edits are O(log n) span lookup + O(s) string splice + O(n) offset fixup.

use crate::{FlatDoc, Span, SpanKind};

#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

/// An entry in the key-value index: points to spans in the document.
#[derive(Debug, Clone)]
pub(crate) struct Entry {
    /// Index into `spans` where the key (or dotted key chain) starts.
    pub key_start: usize,
    pub value_idx: usize,
}

/// Slice the source to get a clean key string (no copy).
/// Trims whitespace and strips surrounding quotes.
#[inline]
pub(crate) fn clean_key<'s>(source: &'s str, span: &Span) -> &'s str {
    let bytes = source.as_bytes();
    let mut s = span.start as usize;
    let mut e = span.end as usize;
    // trim leading whitespace
    while s < e && matches!(bytes[s], b' ' | b'\t') { s += 1; }
    // trim trailing whitespace
    while e > s && matches!(bytes[e - 1], b' ' | b'\t') { e -= 1; }
    // strip surrounding quotes
    if e > s + 1 && matches!(bytes[s], b'"' | b'\'') && bytes[s] == bytes[e - 1] {
        s += 1;
        e -= 1;
    }
    &source[s..e]
}



/// Convenience: clean a key referenced by a span index.
#[inline]
pub(crate) fn clean_key_span<'s>(source: &'s str, span: &Span) -> &'s str {
    clean_key(source, span)
}

pub(crate) fn build_index(doc: &FlatDoc) -> Vec<(Vec<String>, Entry)> {
    let mut index = Vec::with_capacity(doc.spans.len() / 8);
    let mut current_table: Vec<&str> = Vec::new();
    let mut i = 0;

    while i < doc.spans.len() {
        let span = doc.spans[i];

        match span.kind {
            SpanKind::ArrayOpen | SpanKind::ArrayTableOpen => {
                let is_aot = span.kind == SpanKind::ArrayTableOpen;
                let mut path: Vec<&str> = Vec::with_capacity(4);
                i += 1;
                while i < doc.spans.len() {
                    match doc.spans[i].kind {
                        SpanKind::BareKey | SpanKind::BasicString | SpanKind::LiteralString => {
                            path.push(clean_key(&doc.source, &doc.spans[i]));
                            i += 1;
                        }
                        SpanKind::Dot => { i += 1; }
                        SpanKind::ArrayClose => {
                            if !is_aot { current_table = path; }
                            i += 1; break;
                        }
                        SpanKind::ArrayTableClose => { i += 1; break; }
                        _ => { i += 1; break; }
                    }
                }
                continue;
            }

            SpanKind::BareKey | SpanKind::BasicString | SpanKind::LiteralString => {
                let mut key_parts: Vec<&str> = Vec::with_capacity(4);
                key_parts.push(clean_key(&doc.source, &span));
                let key_start = i;
                let mut j = i + 1;

                loop {
                    if j >= doc.spans.len() { break; }
                    match doc.spans[j].kind {
                        SpanKind::Whitespace | SpanKind::Newline | SpanKind::Comment => { j += 1; }
                        SpanKind::Dot => { j += 1; }
                        SpanKind::BareKey | SpanKind::BasicString | SpanKind::LiteralString => {
                            key_parts.push(clean_key(&doc.source, &doc.spans[j]));
                            j += 1;
                        }
                        SpanKind::Equals => {
                            j += 1;
                            let mut k = j;
                            while k < doc.spans.len() {
                                if is_value_kind(doc.spans[k].kind) {
                                    let path: Vec<String> = current_table.iter()
                                        .chain(&key_parts)
                                        .map(|s| s.to_string())
                                        .collect();
                                    index.push((path, Entry { key_start, value_idx: k }));
                                    i = k;
                                    break;
                                }
                                match doc.spans[k].kind {
                                    SpanKind::Whitespace | SpanKind::Newline | SpanKind::Comment => { k += 1; }
                                    _ => { break; }
                                }
                            }
                            break;
                        }
                        _ => { break; }
                    }
                }
                i += 1;
            }
            _ => { i += 1; }
        }
    }

    index
}

pub(crate) fn adjust_spans(spans: &mut [Span], pos: u32, delta: i32) {
    if delta == 0 { return; }
    let first = match spans.binary_search_by_key(&pos, |s| s.start) {
        Ok(idx) => idx,
        Err(idx) => idx,
    };
    for span in &mut spans[first..] {
        span.start = (span.start as i32 + delta) as u32;
        span.end = (span.end as i32 + delta) as u32;
    }
}

fn is_value_kind(k: SpanKind) -> bool {
    matches!(k,
        SpanKind::Integer | SpanKind::Float | SpanKind::Boolean
        | SpanKind::Datetime | SpanKind::BasicString | SpanKind::LiteralString
        | SpanKind::MlBasicString | SpanKind::MlLiteralString
        | SpanKind::InlineTableOpen | SpanKind::ArrayOpen)
}

// ============================================================
// Edit operations
// ============================================================

#[derive(Debug)]
pub enum EditError {
    NotFound,
    InvalidPath,
    SectionExists,
    TableMismatch,
}

#[cfg(feature = "alloc")]
impl core::fmt::Display for EditError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            EditError::NotFound => write!(f, "key not found"),
            EditError::InvalidPath => write!(f, "invalid path or out of bounds"),
            EditError::SectionExists => write!(f, "target section already exists"),
            EditError::TableMismatch => write!(f, "source and destination tables must match"),
        }
    }
}

impl FlatDoc {
    /// Return the source text for a span.
    ///
    /// The slice borrows from the document's source buffer.
    ///
    /// # Panics
    ///
    /// Panics if `span.start` or `span.end` are out of bounds for the
    /// current source.
    pub fn span_text(&self, span: &Span) -> &str {
        &self.source[span.start as usize..span.end as usize]
    }

    /// Replace the value at `path` with `new_value`.
    ///
    /// Preserves surrounding formatting (comments, whitespace) by only
    /// replacing the value span itself.
    pub fn set(&mut self, path: &[&str], new_value: &str) -> Result<(), EditError> {
        let index = build_index(self);
        let entry = index.iter().find(|(p, _)| p.iter().map(|s| s.as_str()).collect::<Vec<_>>() == path)
            .map(|(_, e)| e)
            .ok_or(EditError::NotFound)?;

        let value_span = self.spans[entry.value_idx];
        let old_len = (value_span.end - value_span.start) as usize;
        let new_bytes = new_value.as_bytes();
        let delta = new_bytes.len() as i32 - old_len as i32;

        // Splice the source string
        let start = value_span.start as usize;
        let end = value_span.end as usize;
        self.source.replace_range(start..end, new_value);

        // Adjust downstream spans
        adjust_spans(&mut self.spans, value_span.end, delta);

        Ok(())
    }

    /// Insert a new key-value pair into the table at `path`.
    /// The new key is appended after the last key in that table.
    pub fn insert(&mut self, table_path: &[&str], key: &str, value: &str) -> Result<(), EditError> {
        let index = build_index(self);

        // Find the last entry in the target table
        let entries_in_table: Vec<_> = index.iter()
            .filter(|(p, _)| p.len() == table_path.len() + 1
                && p[..table_path.len()].iter().map(|s| s.as_str()).collect::<Vec<_>>() == table_path)
            .collect();

        let (insertion_point, indentation) = if let Some((_, last)) = entries_in_table.last() {
            let last_span = self.spans[last.key_start];

            // Walk back to find the start of this line
            let mut line_start = last_span.start as usize;
            while line_start > 0 && self.source.as_bytes()[line_start - 1] != b'\n' {
                line_start -= 1;
            }
            // Scan forward through only whitespace to get the indentation
            // (skip any comment on the previous line — we don't copy comment text)
            let mut indent_end = line_start;
            while indent_end < last_span.start as usize
                && matches!(self.source.as_bytes()[indent_end], b' ' | b'\t')
            {
                indent_end += 1;
            }
            let indent = &self.source[line_start..indent_end];

            // Find the end of this entry (after the newline following the value)
            let mut end = last_span.end as usize;
            while end < self.source.len() && self.source.as_bytes()[end] != b'\n' {
                end += 1;
            }
            if end < self.source.len() { end += 1; }

            (end as u32, format!("{indent}{key} = {value}\n"))
        } else {
            // Empty table — insert after the table header
            // Find the table header
            let header_start = if table_path.is_empty() {
                0 // root table — insert at beginning
            } else {
                // Find [table_path] header span
                let mut found = None;
                let mut i = 0;
                while i < self.spans.len() {
                    if self.spans[i].kind == SpanKind::ArrayOpen {
                        let mut j = i + 1;
                        let mut hdr = Vec::new();
                        while j < self.spans.len() {
                            match self.spans[j].kind {
                                SpanKind::BareKey | SpanKind::BasicString | SpanKind::LiteralString => {
                                    hdr.push(clean_key_span(&self.source, &self.spans[j]).to_string());
                                    j += 1;
                                }
                                SpanKind::Dot => { j += 1; }
                                SpanKind::ArrayClose => {
                                    if hdr == table_path {
                                        found = Some(self.spans[j].end);
                                    }
                                    break;
                                }
                                _ => { break; }
                            }
                        }
                        if found.is_some() { break; }
                    }
                    i += 1;
                }
                found.unwrap_or(0)
            };
            // Find the newline after the header
            let mut pos = header_start as usize;
            while pos < self.source.len() && self.source.as_bytes()[pos] != b'\n' {
                pos += 1;
            }
            if pos < self.source.len() { pos += 1; }
            (pos as u32, format!("{key} = {value}\n"))
        };
        let delta = indentation.as_bytes().len() as i32;
        self.source.insert_str(insertion_point as usize, &indentation);
        adjust_spans(&mut self.spans, insertion_point, delta);
        Ok(())
    }

    /// Remove the key-value pair at `path` from the document.
    pub fn remove(&mut self, path: &[&str]) -> Result<(), EditError> {
        let index = build_index(self);
        let entry = index.iter().find(|(p, _)| p.iter().map(|s| s.as_str()).collect::<Vec<_>>() == path)
            .map(|(_, e)| e)
            .ok_or(EditError::NotFound)?;

        // Find the range to remove: from the key's start (including preceding
        // indentation) to the end of the line containing the value.
        let key_span = self.spans[entry.key_start];
        let value_span = self.spans[entry.value_idx];

        // Walk back from key to the start of its line
        let mut remove_start = key_span.start as usize;
        while remove_start > 0 && self.source.as_bytes()[remove_start - 1] != b'\n' {
            remove_start -= 1;
        }

        // Walk forward from value to end of its line
        let mut remove_end = value_span.end as usize;
        while remove_end < self.source.len() && self.source.as_bytes()[remove_end] != b'\n' {
            remove_end += 1;
        }
        if remove_end < self.source.len() { remove_end += 1; } // include the newline

        let old_len = (remove_end - remove_start) as i32;
        self.source.replace_range(remove_start..remove_end, "");
        adjust_spans(&mut self.spans, remove_start as u32, -old_len);

        Ok(())
    }
}
