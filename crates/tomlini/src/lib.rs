//! # tomlini — SAX TOML/INI parser and editor
//!
//! A zero-dependency, three-tier (`core`/`alloc`/`std`) TOML implementation
//! that parses into a flat span index instead of a DOM tree. Edits are
//! byte-range splices on the source string — no decor model, no footguns.
//!
//! ## Quick start
//!
//! ```ignore
//! use tomlini::Editor;
//!
//! let mut doc = tomlini::parse("[server]\nport = 8080\n")?;
//!
//! // Read values
//! assert!(doc.has("server.port"));
//! let raw = doc.get("server.port").unwrap();        // "8080"
//! let val = doc.get_decoded("server.port").unwrap(); // "8080"
//!
//! // Edit with the batch editor
//! doc.edit()
//!     .set("server.port", "9090")
//!     .insert("server", "host", "\"0.0.0.0\"")
//!         .with_above_comment("Bind address")
//!     .commit()?;
//!
//! let output = doc.to_string();
//! ```
//!
//! ## Feature tiers
//!
//! | Feature  | What you get |
//! |----------|-------------|
//! | *(none)* | `parse_into()` — classified spans via callback, zero allocations |
//! | `alloc`  | `FlatDoc`, `parse()`, full editing API |
//! | `std`    | (default) `std::error::Error` impls |
//!
//! ## Key types
//!
//! - [`FlatDoc`] — parsed document: source string + flat span index
//! - [`Editor`] — batch mutation accumulator, commit applies all ops at once
//! - [`BringAlong`] — composable flags for what adjacent text to carry when relocating
//! - [`Span`], [`SpanKind`] — classified byte ranges in the source
//! - [`ParseError`] — parse error with byte position
//! - [`ValidationMode`] — lenient/relaxed/strict validation levels
//! - [`EditError`] — editor error variants (`NotFound`, `InvalidPath`, `SectionExists`, `TableMismatch`)
//! - [`SpanSink`] — core-only callback trait for span emission
//!
//! ## Performance
//!
//! Parse is **18× faster** than `toml_edit` on a 94-line Cargo.toml (3.3 µs vs
//! 59.5 µs). Batch edits are **2–3× faster** than equivalent `toml_edit`
//! operations. See `crates/benchmarks/` for details.
//!
//! ## Format preservation
//!
//! Every edit operation preserves comments, whitespace, and formatting
//! unless explicitly overridden.  We test for these invariants across
//! all 22 edit operations:
//!
//! - Comments between keys survive insertions and removals
//! - Inline comments on modified lines stay in place
//! - Key formatting (quoted vs bare, dotted vs flat) is never altered
//! - Value formatting (hex integers, multi-line strings, literal vs basic)
//!   passes through unchanged
//! - Blank-line separators between sections are maintained
//! - Indentation of new keys copies the neighbor's indentation
//! - Removing the last key in a section cleans up trailing whitespace
//! - Dotted-key headers like `[profiles.dev]` survive reordering intact
//! - Comments above keys move with the key when `BringAlong` flags are used
//!
//! These invariants are verified by **11 footgun immunity tests**
//! and **8 proptest fuzzers** that generate random documents and
//! random edit sequences, asserting that the editor never panics and
//! that successful commits produce re-parseable output.
//!
//! ## Movement & comment control
//!
//! [`BringAlong`] bitflags let you control what adjacent text travels
//! with a key or section when it moves:
//!
//! ```ignore
//! use tomlini::editor::BringAlong;
//!
//! // Move a key, bringing the comment above it
//! doc.edit().move_key_bring("a.k", "b.k", BringAlong::COMMENTS_ABOVE).commit()?;
//!
//! // Promote to root, bringing comments on both sides
//! doc.edit().promote_key_bring("meta.base",
//!     BringAlong::COMMENTS_ABOVE | BringAlong::COMMENTS_BELOW).commit()?;
//!
//! // Reorder root entries, keeping section-preceding comments with their section
//! doc.edit().reorder_root_bring(&["base", "meta"], BringAlong::COMMENTS_ABOVE).commit()?;
//! ```
//!
//! ## Acknowledgments
//!
//! Built on the excellent work of the [toml-rs](https://github.com/toml-rs/toml) project:
//! [`toml_edit`](https://crates.io/crates/toml_edit),
//! [`toml_datetime`](https://crates.io/crates/toml_datetime), and
//! [`toml-test`](https://github.com/toml-lang/toml-test).
//!
//! ## INI files
//!
//! `tomlini` parses INI-style configs out of the box — `;` comments, bare
//! values, `=` separators. No special mode needed: `tomlini::parse(ini_str)`.
//!
//! ## Validation modes
//!
//! ```ignore
//! use tomlini::ValidationMode;
//!
//! doc.validate(ValidationMode::Lenient);   // everything accepted
//! doc.validate(ValidationMode::Relaxed);   // structural TOML + INI extensions
//! doc.validate(ValidationMode::Strict);    // full TOML 1.1.0 spec
//! ```
//!
//! ## Container editing
//!
//! Arrays, inline tables, and array-of-tables are first-class edit targets:
//!
//! ```ignore
//! doc.edit()
//!     .array_push("hosts", "\"10.0.0.3\"")
//!     .inline_set("headers", "content-type", "\"text/html\"")
//!     .aot_push("backend", &[("host", "\"10.0.0.4\""), ("port", "9000")])
//!     .aot_remove("backend", 0)
//!     .commit()?;
//! ```
//!
//! ## Core-only usage
//!
//! Without the `alloc` feature, the parser emits spans through a callback:
//!
//! ```ignore
//! tomlini::parse_into(input, &mut |kind, start, end| {
//!     // Called for every classified span
//! });
//! ```
//!
//! [`FlatDoc`]: crate::FlatDoc
//! [`Editor`]: crate::editor::Editor
//! [`BringAlong`]: crate::editor::BringAlong
//! [`EditError`]: crate::edit::EditError
//! [`Span`]: crate::Span
//! [`SpanKind`]: crate::SpanKind
//! [`SpanSink`]: crate::SpanSink
//! [`ParseError`]: crate::ParseError
//! [`ValidationMode`]: crate::ValidationMode
//! [`toml_edit`]: https://crates.io/crates/toml_edit
//! [`toml_datetime`]: https://crates.io/crates/toml_datetime
//! [`toml-test`]: https://github.com/toml-lang/toml-test
#![cfg_attr(all(not(feature = "std"), not(test)), no_std)]

#[cfg(feature = "alloc")]
#[macro_use]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;
#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::fmt;
#[cfg(feature = "std")]
use std::fmt;

#[cfg(feature = "alloc")]
mod edit;
#[cfg(feature = "alloc")]
pub mod editor;
#[cfg(feature = "alloc")]
pub use edit::EditError;
#[cfg(feature = "alloc")]
mod validate;
#[cfg(feature = "alloc")]
pub use validate::{ValidationError, ValidationErrorKind, ValidationMode};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub kind: SpanKind,
    pub start: u32,
    pub end: u32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SpanKind {
    Whitespace = 0,
    Newline = 1,
    Comment = 2,
    BareKey = 3,
    BasicString = 4,
    LiteralString = 5,
    MlBasicString = 6,
    MlLiteralString = 7,
    Integer = 8,
    Float = 9,
    Boolean = 10,
    Datetime = 11,
    ArrayOpen = 12,
    ArrayClose = 13,
    ArrayTableOpen = 14,
    ArrayTableClose = 15,
    InlineTableOpen = 16,
    InlineTableClose = 17,
    Equals = 18,
    Dot = 19,
    Comma = 20,
}

/// Sink trait for receiving classified spans without allocation.
pub trait SpanSink {
    fn emit(&mut self, kind: SpanKind, start: u32, end: u32);
}

#[derive(Debug)]
pub struct ParseError {
    /// Byte position of the error.  Consistent with [`Span::start`] (both `u32`).
    pub pos: u32,
    /// Human-readable error description.
    pub msg: &'static str,
}

#[cfg(feature = "std")]
impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parse error at byte {}: {}", self.pos, self.msg)
    }
}

#[cfg(all(feature = "alloc", not(feature = "std")))]
impl alloc::fmt::Display for ParseError {
    fn fmt(&self, f: &mut alloc::fmt::Formatter<'_>) -> alloc::fmt::Result {
        write!(f, "parse error at byte {}: {}", self.pos, self.msg)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ParseError {}

// ---------------------------------------------------------------------------
// Parser: lex helpers — generic over SpanSink
// ---------------------------------------------------------------------------

/// TOML 1.1.0: backslash at end of line trims newline + leading whitespace.
fn skip_trim(bytes: &[u8], pos: usize, len: usize) -> usize {
    let mut p = pos;
    if p < len && bytes[p] == b'\r' {
        p += 1;
    }
    if p < len && bytes[p] == b'\n' {
        p += 1;
    }
    while p < len && matches!(bytes[p], b' ' | b'\t') {
        p += 1;
    }
    p
}

fn lex_string<S: SpanSink>(
    bytes: &[u8],
    mut pos: usize,
    len: usize,
    sink: &mut S,
    start: usize,
    error: &mut Option<ParseError>,
) -> usize {
    if pos + 2 < len && bytes[pos + 1] == b'"' && bytes[pos + 2] == b'"' {
        pos += 3;
        if pos < len && bytes[pos] == b'\n' {
            pos += 1;
        } else if pos + 1 < len && bytes[pos] == b'\r' && bytes[pos + 1] == b'\n' {
            pos += 2;
        }
        loop {
            if pos >= len {
                *error = Some(ParseError {
                    pos: start as u32,
                    msg: "unterminated multi-line basic string",
                });
                return pos;
            }
            if bytes[pos] == b'"'
                && pos + 1 < len
                && bytes[pos + 1] == b'"'
                && pos + 2 < len
                && bytes[pos + 2] == b'"'
            {
                // Count consecutive quotes. 3 = closing. 4-5 = leading ones
                // are content, last 3 close. 6+ = invalid (content would need
                // escaping); first 3 close, trailing quotes cause parse error.
                let mut n = 3;
                while pos + n < len && bytes[pos + n] == b'"' {
                    n += 1;
                }
                if n >= 6 {
                    pos += 3;
                    break;
                }
                if n > 3 {
                    pos += n - 3;
                    continue;
                }
                pos += 3;
                break;
            }
            if bytes[pos] == b'\\' && pos + 1 < len {
                pos += 1;
                let prev = pos;
                pos = skip_trim(bytes, pos, len);
                if pos == prev {
                    pos += 1;
                }
            } else {
                pos += 1;
            }
        }
        sink.emit(SpanKind::MlBasicString, start as u32, pos as u32);
    } else {
        pos += 1;
        while pos < len && bytes[pos] != b'"' {
            if bytes[pos] == b'\\' && pos + 1 < len {
                pos += 1;
                let prev = pos;
                pos = skip_trim(bytes, pos, len);
                if pos == prev {
                    pos += 1;
                }
            } else {
                pos += 1;
            }
        }
        if pos >= len {
            *error = Some(ParseError {
                pos: start as u32,
                msg: "unterminated basic string",
            });
            return pos;
        }
        pos += 1;
        sink.emit(SpanKind::BasicString, start as u32, pos as u32);
    }
    pos
}

fn lex_literal_string<S: SpanSink>(
    bytes: &[u8],
    mut pos: usize,
    len: usize,
    sink: &mut S,
    start: usize,
    error: &mut Option<ParseError>,
) -> usize {
    if pos + 2 < len && bytes[pos + 1] == b'\'' && bytes[pos + 2] == b'\'' {
        pos += 3;
        if pos < len && bytes[pos] == b'\n' {
            pos += 1;
        } else if pos + 1 < len && bytes[pos] == b'\r' && bytes[pos + 1] == b'\n' {
            pos += 2;
        }
        loop {
            if pos >= len {
                *error = Some(ParseError {
                    pos: start as u32,
                    msg: "unterminated multi-line literal string",
                });
                return pos;
            }
            if bytes[pos] == b'\''
                && pos + 1 < len
                && bytes[pos + 1] == b'\''
                && pos + 2 < len
                && bytes[pos + 2] == b'\''
            {
                // Count consecutive quotes. 3 = closing. 4-5 = leading ones
                // are content, last 3 close. 6+ = invalid (content would need
                // escaping); first 3 close, trailing quotes cause parse error.
                let mut n = 3;
                while pos + n < len && bytes[pos + n] == b'\'' {
                    n += 1;
                }
                if n >= 6 {
                    pos += 3;
                    break;
                }
                if n > 3 {
                    pos += n - 3;
                    continue;
                }
                pos += 3;
                break;
            }
            pos += 1;
        }
        sink.emit(SpanKind::MlLiteralString, start as u32, pos as u32);
    } else {
        pos += 1;
        while pos < len && bytes[pos] != b'\'' {
            if bytes[pos] == b'\n' || bytes[pos] == b'\r' {
                *error = Some(ParseError {
                    pos: start as u32,
                    msg: "newline in literal string",
                });
                return pos;
            }
            pos += 1;
        }
        if pos >= len {
            *error = Some(ParseError {
                pos: start as u32,
                msg: "unterminated literal string",
            });
            return pos;
        }
        pos += 1; // closing '
        sink.emit(SpanKind::LiteralString, start as u32, pos as u32);
    }
    pos
}

fn lex_bare_key(bytes: &[u8], mut pos: usize, len: usize) -> usize {
    while pos < len && is_bare_key_char(bytes[pos]) {
        pos += 1;
    }
    pos
}

fn is_bare_key_lead(b: u8) -> bool {
    matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'_' | b'-')
}

fn is_bare_key_char(b: u8) -> bool {
    matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_')
}

fn lex_number_or_datetime<S: SpanSink>(
    bytes: &[u8],
    mut pos: usize,
    len: usize,
    sink: &mut S,
    start: usize,
) -> usize {
    // Try datetime first (more specific pattern)
    if let Some(end) = try_datetime(bytes, pos, len) {
        pos = end;
        sink.emit(SpanKind::Datetime, start as u32, pos as u32);
        return pos;
    }
    // Try number (integer or float)
    if let Some(end) = try_number(bytes, pos, len) {
        pos = end;
        let kind = if is_float(bytes, start, pos) {
            SpanKind::Float
        } else {
            SpanKind::Integer
        };
        sink.emit(kind, start as u32, pos as u32);
        return pos;
    }
    // Fallback: bare key. If the character isn't a bare key char (e.g. +, -),
    // advance by at least one to avoid an infinite loop.
    let old = pos;
    pos = lex_bare_key(bytes, pos, len);
    if pos == old {
        pos += 1;
    }
    sink.emit(SpanKind::BareKey, start as u32, pos as u32);
    pos
}

fn try_number(bytes: &[u8], mut pos: usize, len: usize) -> Option<usize> {
    let start = pos;
    // Optional sign
    if pos < len && matches!(bytes[pos], b'+' | b'-') {
        pos += 1;
    }
    if pos >= len {
        return None;
    }

    let remains = &bytes[pos..];
    if remains.starts_with(b"inf") {
        return Some(pos + 3);
    }
    if remains.starts_with(b"nan") {
        return Some(pos + 3);
    }

    if pos < len && bytes[pos] == b'0' && pos + 1 < len {
        match bytes[pos + 1] {
            b'x' | b'o' | b'b' => {
                let prefix = bytes[pos + 1];
                pos += 2;
                if pos >= len || !is_radix_digit(bytes[pos], prefix) {
                    return None;
                }
                while pos < len && (is_radix_digit(bytes[pos], prefix) || bytes[pos] == b'_') {
                    pos += 1;
                }
                return Some(pos);
            }
            _ => {}
        }
    }

    // Decimal digits
    if pos < len && !bytes[pos].is_ascii_digit() {
        return None;
    }
    while pos < len && (bytes[pos].is_ascii_digit() || bytes[pos] == b'_') {
        pos += 1;
    }
    // Optional fractional part
    if pos < len && bytes[pos] == b'.' {
        pos += 1;
        while pos < len && (bytes[pos].is_ascii_digit() || bytes[pos] == b'_') {
            pos += 1;
        }
    }
    // Optional exponent
    if pos < len && matches!(bytes[pos], b'e' | b'E') {
        pos += 1;
        if pos < len && matches!(bytes[pos], b'+' | b'-') {
            pos += 1;
        }
        while pos < len && (bytes[pos].is_ascii_digit() || bytes[pos] == b'_') {
            pos += 1;
        }
    }
    if pos == start || (pos == start + 1 && matches!(bytes[start], b'+' | b'-')) {
        return None;
    }
    Some(pos)
}

fn is_float(bytes: &[u8], start: usize, end: usize) -> bool {
    // Hex, octal, binary literals are never floats
    if start + 1 < end && bytes[start] == b'0' && matches!(bytes[start + 1], b'x' | b'o' | b'b') {
        return false;
    }
    // inf and nan are always floats per TOML spec
    let len = end - start;
    if len >= 3 {
        let tail = &bytes[end - 3..end];
        if tail == b"inf" || tail == b"nan" {
            return true;
        }
    }
    bytes[start..end]
        .iter()
        .any(|&b| b == b'.' || b == b'e' || b == b'E')
}

fn is_radix_digit(b: u8, prefix: u8) -> bool {
    match prefix {
        b'x' => b.is_ascii_hexdigit(),
        b'o' => matches!(b, b'0'..=b'7'),
        b'b' => matches!(b, b'0' | b'1'),
        _ => false,
    }
}

fn try_datetime(bytes: &[u8], mut pos: usize, len: usize) -> Option<usize> {
    if pos >= len || !bytes[pos].is_ascii_digit() {
        return None;
    }
    let start = pos;

    // Try local time: HH:MM:SS
    if pos + 8 <= len
        && bytes[pos + 2] == b':'
        && bytes[pos + 5] == b':'
        && bytes[pos..pos + 2].iter().all(|b| b.is_ascii_digit())
        && bytes[pos + 3..pos + 5].iter().all(|b| b.is_ascii_digit())
        && bytes[pos + 6..pos + 8].iter().all(|b| b.is_ascii_digit())
    {
        pos += 8;
        if pos < len && bytes[pos] == b'.' {
            pos += 1;
            while pos < len && bytes[pos].is_ascii_digit() {
                pos += 1;
            }
        }
        return Some(pos);
    }
    // Try local time: HH:MM (without seconds)
    if pos + 5 <= len
        && bytes[pos + 2] == b':'
        && bytes[pos..pos + 2].iter().all(|b| b.is_ascii_digit())
        && bytes[pos + 3..pos + 5].iter().all(|b| b.is_ascii_digit())
    {
        pos += 5;
        return Some(pos);
    }

    // Try full date: YYYY-MM-DD
    if pos + 10 > len {
        return None;
    }
    if bytes[pos + 4] != b'-' || bytes[pos + 7] != b'-' {
        return None;
    }
    if !bytes[pos..pos + 4].iter().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if !bytes[pos + 5..pos + 7].iter().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if !bytes[pos + 8..pos + 10].iter().all(|b| b.is_ascii_digit()) {
        return None;
    }
    pos += 10;

    // Optional time
    if pos < len && (bytes[pos] == b'T' || bytes[pos] == b't' || bytes[pos] == b' ') {
        pos += 1;
        let time_start = pos;
        // Try HH:MM:SS
        if pos + 8 <= len
            && bytes[pos + 2] == b':'
            && bytes[pos + 5] == b':'
            && bytes[pos..pos + 2].iter().all(|b| b.is_ascii_digit())
            && bytes[pos + 3..pos + 5].iter().all(|b| b.is_ascii_digit())
            && bytes[pos + 6..pos + 8].iter().all(|b| b.is_ascii_digit())
        {
            pos += 8;
        } else if pos + 5 <= len
            && bytes[pos + 2] == b':'
            && bytes[pos..pos + 2].iter().all(|b| b.is_ascii_digit())
            && bytes[pos + 3..pos + 5].iter().all(|b| b.is_ascii_digit())
        {
            // HH:MM (without seconds)
            pos += 5;
        } else {
            return Some(time_start);
        }
        if pos < len && bytes[pos] == b'.' {
            pos += 1;
            while pos < len && bytes[pos].is_ascii_digit() {
                pos += 1;
            }
        }
        if pos < len {
            if bytes[pos] == b'Z' || bytes[pos] == b'z' {
                pos += 1;
            } else if pos + 6 <= len && matches!(bytes[pos], b'+' | b'-') && bytes[pos + 3] == b':'
            {
                pos += 6;
            }
        }
        return Some(pos);
    }

    if pos == start { None } else { Some(pos) }
}

// ---------------------------------------------------------------------------
// Public parse functions
// ---------------------------------------------------------------------------

pub fn parse_into(input: &str, sink: &mut impl SpanSink) -> Result<(), ParseError> {
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut pos = 0usize;
    let mut error = None;

    // Track how many ArrayTableOpen are unclosed (for matching ArrayTableClose).
    // Track how many ArrayTableOpen are unclosed (for matching ArrayTableClose).
    let mut aot_depth: usize = 0;

    // Returns true if `p` is at line start (preceded only by whitespace/newlines).
    fn is_line_start(bytes: &[u8], p: usize) -> bool {
        if p == 0 {
            return true;
        }
        let mut i = p;
        while i > 0 {
            i -= 1;
            match bytes[i] {
                b' ' | b'\t' => continue,
                b'\n' | b'\r' => return true,
                _ => return false,
            }
        }
        true
    }

    while pos < len {
        let start = pos;
        match bytes[pos] {
            b' ' | b'\t' => {
                while pos < len && matches!(bytes[pos], b' ' | b'\t') {
                    pos += 1;
                }
                sink.emit(SpanKind::Whitespace, start as u32, pos as u32);
            }
            b'\n' => {
                pos += 1;
                sink.emit(SpanKind::Newline, start as u32, pos as u32);
            }
            b'\r' => {
                pos += 1;
                if pos < len && bytes[pos] == b'\n' {
                    pos += 1;
                }
                sink.emit(SpanKind::Newline, start as u32, pos as u32);
            }
            b'#' | b';' => {
                while pos < len && bytes[pos] != b'\n' && bytes[pos] != b'\r' {
                    pos += 1;
                }
                sink.emit(SpanKind::Comment, start as u32, pos as u32);
            }
            b'[' => {
                pos += 1;
                if pos < len && bytes[pos] == b'[' && is_line_start(bytes, start) {
                    pos += 1;
                    sink.emit(SpanKind::ArrayTableOpen, start as u32, pos as u32);
                    aot_depth += 1;
                } else if pos < len && bytes[pos] == b'[' {
                    // Two `[` not at line start: emit two separate ArrayOpen spans
                    pos += 1;
                    sink.emit(SpanKind::ArrayOpen, start as u32, (start + 1) as u32);
                    sink.emit(SpanKind::ArrayOpen, (start + 1) as u32, pos as u32);
                } else {
                    sink.emit(SpanKind::ArrayOpen, start as u32, pos as u32);
                }
            }
            b']' => {
                pos += 1;
                if pos < len && bytes[pos] == b']' && aot_depth > 0 {
                    pos += 1;
                    sink.emit(SpanKind::ArrayTableClose, start as u32, pos as u32);
                    aot_depth -= 1;
                } else {
                    sink.emit(SpanKind::ArrayClose, start as u32, pos as u32);
                }
            }
            b'{' => {
                pos += 1;
                sink.emit(SpanKind::InlineTableOpen, start as u32, pos as u32);
            }
            b'}' => {
                pos += 1;
                sink.emit(SpanKind::InlineTableClose, start as u32, pos as u32);
            }
            b'=' => {
                pos += 1;
                sink.emit(SpanKind::Equals, start as u32, pos as u32);
            }
            b'.' => {
                pos += 1;
                sink.emit(SpanKind::Dot, start as u32, pos as u32);
            }
            b',' => {
                pos += 1;
                sink.emit(SpanKind::Comma, start as u32, pos as u32);
            }
            b'"' => {
                pos = lex_string(bytes, pos, len, sink, start, &mut error);
            }
            b'\'' => {
                pos = lex_literal_string(bytes, pos, len, sink, start, &mut error);
            }
            b't' | b'f' => {
                let remains = &bytes[pos..];
                if remains.starts_with(b"true") {
                    pos += 4;
                    sink.emit(SpanKind::Boolean, start as u32, pos as u32);
                } else if remains.starts_with(b"false") {
                    pos += 5;
                    sink.emit(SpanKind::Boolean, start as u32, pos as u32);
                } else {
                    pos = lex_bare_key(bytes, pos, len);
                    sink.emit(SpanKind::BareKey, start as u32, pos as u32);
                }
            }
            b'+' | b'-' | b'0'..=b'9' | b'i' | b'n' => {
                pos = lex_number_or_datetime(bytes, pos, len, sink, start);
            }
            _ => {
                if is_bare_key_lead(bytes[pos]) {
                    pos = lex_bare_key(bytes, pos, len);
                    sink.emit(SpanKind::BareKey, start as u32, pos as u32);
                } else {
                    pos += 1;
                    sink.emit(SpanKind::BareKey, start as u32, pos as u32);
                }
            }
        }
        if error.is_some() {
            break;
        }
    }

    match error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

// ===========================================================================
// alloc-gated: FlatDoc, parse(), decode_toml_string, editing
// ===========================================================================

#[cfg(feature = "alloc")]
use alloc::string::{String, ToString};
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

#[cfg(feature = "alloc")]
pub struct FlatDoc {
    pub source: String,
    pub spans: Vec<Span>,
    pub(crate) index: Option<Vec<(Vec<String>, edit::Entry)>>,
}

#[cfg(feature = "alloc")]
/// Parse a TOML document into a [`FlatDoc`] for editing.
pub fn parse(input: &str) -> Result<FlatDoc, ParseError> {
    struct VecSink {
        spans: Vec<Span>,
    }
    impl SpanSink for VecSink {
        fn emit(&mut self, kind: SpanKind, start: u32, end: u32) {
            self.spans.push(Span { kind, start, end });
        }
    }
    let mut sink = VecSink {
        spans: Vec::with_capacity(input.len() / 6),
    };
    parse_into(input, &mut sink)?;
    Ok(FlatDoc {
        source: input.to_string(),
        spans: sink.spans,
        index: None,
    })
}

#[cfg(feature = "alloc")]
impl Default for FlatDoc {
    fn default() -> Self {
        FlatDoc::new()
    }
}

#[cfg(feature = "alloc")]
/// Decode a TOML string value: resolve escape sequences.
fn decode_toml_string(raw: &str, kind: SpanKind) -> String {
    // Literal strings and multi-line literals — no escape processing
    if matches!(kind, SpanKind::LiteralString | SpanKind::MlLiteralString) {
        return raw.trim_matches('\'').to_string();
    }
    let inner = match kind {
        SpanKind::BasicString => &raw[1..raw.len() - 1],
        SpanKind::MlBasicString => {
            let s = raw.find('\n').map(|i| i + 1).unwrap_or(3);
            let e = raw.rfind("\"\"\"").unwrap_or(raw.len());
            &raw[s..e]
        }
        _ => return raw.to_string(),
    };
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some('"') => out.push('"'),
            Some('b') => out.push('\x08'),
            Some('f') => out.push('\x0C'),
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                    out.push(char::from_u32(cp).unwrap_or('\u{FFFD}'));
                }
            }
            Some('U') => {
                let hex: String = chars.by_ref().take(8).collect();
                if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                    out.push(char::from_u32(cp).unwrap_or('\u{FFFD}'));
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(feature = "alloc")]
impl FlatDoc {
    pub(crate) fn build_index_if_needed(&mut self) {
        if self.index.is_none() {
            self.index = Some(edit::build_index(self));
        }
    }
}

#[cfg(feature = "alloc")]
impl FlatDoc {
    /// Create an empty document suitable for programmatic construction.
    pub fn new() -> Self {
        FlatDoc {
            source: String::new(),
            spans: Vec::new(),
            index: None,
        }
    }

    /// Check whether the given dotted path exists in the document.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if doc.has("package.version") { /* ... */ }
    /// ```
    pub fn has(&mut self, path: &str) -> bool {
        self.build_index_if_needed();
        let idx = self.index.as_ref().unwrap();
        let (table, key) = editor::split_path(path);
        let target: Vec<&str> = table
            .iter()
            .map(|s| s.as_str())
            .chain(core::iter::once(key.as_str()))
            .collect();
        idx.iter().any(|(p, _)| editor::path_eq(p, &target))
    }

    /// Get the raw text of the value at the given dotted path.
    ///
    /// Returns `None` if the path does not exist.  The returned slice
    /// borrows from the document's source buffer.
    ///
    /// # Example
    ///
    /// ```ignore
    /// assert_eq!(doc.get("package.version"), Some("\"0.1.0\""));
    /// ```
    pub fn get(&mut self, path: &str) -> Option<&str> {
        self.build_index_if_needed();
        let idx = self.index.as_ref().unwrap();
        let (table, key) = editor::split_path(path);
        let target: Vec<&str> = table
            .iter()
            .map(|s| s.as_str())
            .chain(core::iter::once(key.as_str()))
            .collect();
        let entry = idx.iter().find(|(p, _)| editor::path_eq(p, &target))?;
        let value_span = self.spans[entry.1.value_idx];
        Some(&self.source[value_span.start as usize..value_span.end as usize])
    }

    /// Get the decoded string value at `path`.
    ///
    /// For string values (basic, literal, multi-line), returns the decoded
    /// content with escape sequences resolved. For non-string values, returns
    /// the raw source text.
    pub fn get_decoded(&mut self, path: &str) -> Option<String> {
        self.build_index_if_needed();
        let idx = self.index.as_ref().unwrap();
        let (table, key) = editor::split_path(path);
        let target: Vec<&str> = table
            .iter()
            .map(|s| s.as_str())
            .chain(core::iter::once(key.as_str()))
            .collect();
        let entry = idx.iter().find(|(p, _)| editor::path_eq(p, &target))?;
        let span = self.spans[entry.1.value_idx];
        let raw = &self.source[span.start as usize..span.end as usize];
        Some(decode_toml_string(raw, span.kind))
    }

    /// List all top-level keys in the document.
    ///
    /// Returns keys of the root table: both scalar key-value pairs and
    /// sub-table headers.
    pub fn keys(&mut self) -> Vec<String> {
        self.build_index_if_needed();
        let idx = self.index.as_ref().unwrap();
        let mut keys: Vec<String> = idx
            .iter()
            .filter(|(p, _)| !p.is_empty())
            .map(|(p, _)| p[0].clone())
            .collect();
        keys.sort();
        keys.dedup();
        keys
    }

    /// Check whether a key refers to a sub-table.
    ///
    /// Returns `true` if the key exists and its value is a table
    /// (either an explicit `[table]` section or an inline table `{...}`).
    pub fn is_table(&mut self, key: &str) -> bool {
        self.build_index_if_needed();
        let idx = self.index.as_ref().unwrap();
        // A sub-table exists if any entry has this key as a path prefix
        // (meaning there are dotted keys or section keys under it)
        idx.iter().any(|(p, _)| p.len() >= 2 && p[0] == key)
            || idx
                .iter()
                .any(|(p, _)| p.len() == 1 && p[0] == key && self.is_value_table(key))
    }

    fn is_value_table(&self, key: &str) -> bool {
        self.index
            .as_ref()
            .unwrap()
            .iter()
            .filter(|(p, _)| p.len() == 1 && p[0] == key)
            .any(|(_, e)| {
                let span = self.spans[e.value_idx];
                matches!(span.kind, SpanKind::InlineTableOpen)
            })
    }

    /// Begin a batch editing session.
    pub fn edit(&mut self) -> editor::EditorHandle<'_> {
        editor::EditorHandle {
            doc: self,
            editor: editor::Editor::new(),
        }
    }

    /// Validate the document against a strictness mode.
    ///
    /// Returns a list of validation problems. An empty vector means
    /// the document passes validation.
    ///
    /// # Modes
    ///
    /// - [`ValidationMode::Lenient`] — always returns no errors
    /// - [`ValidationMode::Relaxed`] — checks duplicates, table conflicts, AOT ordering
    /// - [`ValidationMode::Strict`] — full TOML 1.1.0 spec compliance
    pub fn validate(&mut self, mode: ValidationMode) -> Vec<ValidationError> {
        validate::validate(self, mode)
    }
}

#[cfg(feature = "alloc")]
impl fmt::Display for FlatDoc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.source)
    }
}

// Gate the std::error::Error impl on std
#[cfg(feature = "std")]
impl std::error::Error for EditError {}
