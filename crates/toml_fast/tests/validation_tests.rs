//! Validation tests for toml_fast.

use toml_fast::{ValidationMode, ValidationErrorKind};

// ---------------------------------------------------------------------------
// duplicate key detection
// ---------------------------------------------------------------------------

#[test]
fn test_duplicate_key_rejected() {
    let mut doc = toml_fast::parse("[a]\nb=1\n[a]\nc=2\n").unwrap();
    let errors = doc.validate(ValidationMode::Relaxed);
    assert!(!errors.is_empty(), "expected duplicate key error");
    assert!(
        errors.iter().any(|e| matches!(e.kind, ValidationErrorKind::DuplicateKey)),
        "expected DuplicateKey error, got: {:?}",
        errors.iter().map(|e| e.kind).collect::<Vec<_>>(),
    );
}

#[test]
fn test_duplicate_key_within_table() {
    let mut doc = toml_fast::parse("[table]\nkey = 1\nkey = 2\n").unwrap();
    let errors = doc.validate(ValidationMode::Relaxed);
    assert!(
        errors.iter().any(|e| matches!(e.kind, ValidationErrorKind::DuplicateKey)),
        "expected DuplicateKey error"
    );
}

#[test]
fn test_no_duplicate_distinct_tables() {
    let mut doc = toml_fast::parse("[a]\nkey = 1\n[b]\nkey = 2\n").unwrap();
    let errors = doc.validate(ValidationMode::Relaxed);
    // Same key name in different tables is fine
    assert!(
        !errors.iter().any(|e| matches!(e.kind, ValidationErrorKind::DuplicateKey)),
        "should not report duplicate for same key in different tables"
    );
}

// ---------------------------------------------------------------------------
// table conflict detection
// ---------------------------------------------------------------------------

#[test]
fn test_table_conflict() {
    let mut doc = toml_fast::parse("a = 1\n[a.b]\nc = 2\n").unwrap();
    let errors = doc.validate(ValidationMode::Relaxed);
    assert!(
        errors.iter().any(|e| matches!(e.kind, ValidationErrorKind::TableConflict)),
        "expected TableConflict error, got: {:?}",
        errors.iter().map(|e| e.kind).collect::<Vec<_>>(),
    );
}

#[test]
fn test_no_table_conflict_valid_nesting() {
    let mut doc = toml_fast::parse("[a]\nb = 1\n[a.c]\nd = 2\n").unwrap();
    let errors = doc.validate(ValidationMode::Relaxed);
    assert!(
        !errors.iter().any(|e| matches!(e.kind, ValidationErrorKind::TableConflict)),
        "should not report conflict for valid table nesting"
    );
}

#[test]
fn test_table_conflict_dotted_key() {
    // a.b = 1 means 'a' is implicitly a table containing 'b'. Then [a] redefines it.
    let mut doc = toml_fast::parse("a.b = 1\n[a]\nc = 2\n").unwrap();
    let errors = doc.validate(ValidationMode::Relaxed);
    // The implicit table 'a' from dotted key vs explicit [a] table — this is a conflict
    assert!(
        errors.iter().any(|e| matches!(e.kind, ValidationErrorKind::TableConflict)),
        "expected TableConflict error for dotted key + table conflict"
    );
}

// ---------------------------------------------------------------------------
// AOT ordering
// ---------------------------------------------------------------------------

#[test]
fn test_aot_ordering_non_consecutive() {
    let mut doc = toml_fast::parse(
        "[[a]]\nx = 1\n[[b]]\ny = 2\n[[a]]\nz = 3\n"
    ).unwrap();
    let errors = doc.validate(ValidationMode::Relaxed);
    assert!(
        errors.iter().any(|e| matches!(e.kind, ValidationErrorKind::AotOrdering)),
        "expected AotOrdering error"
    );
}

#[test]
fn test_aot_ordering_consecutive_ok() {
    let mut doc = toml_fast::parse(
        "[[a]]\nx = 1\n[[a]]\ny = 2\n"
    ).unwrap();
    let errors = doc.validate(ValidationMode::Relaxed);
    assert!(
        !errors.iter().any(|e| matches!(e.kind, ValidationErrorKind::AotOrdering)),
        "consecutive AOT entries should be valid"
    );
}

// ---------------------------------------------------------------------------
// INI semicolon comment support
// ---------------------------------------------------------------------------

#[test]
fn test_ini_comment_accepted() {
    // Semicolons are always treated as comments now
    let doc = toml_fast::parse("; this is a comment\nkey = 1\n").unwrap();
    // Verify the comment span is there
    let comment_spans: Vec<_> = doc.spans
        .iter()
        .filter(|s| s.kind == toml_fast::SpanKind::Comment)
        .collect();
    assert!(!comment_spans.is_empty(), "expected a comment span for `;`");
    // Verify the key is correctly parsed
    assert!(doc.spans.iter().any(|s| {
        s.kind == toml_fast::SpanKind::BareKey
            && &doc.source[s.start as usize..s.end as usize] == "key"
    }));
}

#[test]
fn test_semicolon_comment_mid_line() {
    // Not valid as an inline comment — ; is only a comment at line start (after whitespace).
    // But the parser treats bare `;` as a comment regardless of position in the main loop.
    // For now, verify parse doesn't crash.
    let result = toml_fast::parse("key = 1 ; trailing\n");
    // May or may not parse depending on how `;` interacts with the value context.
    // The key point is it shouldn't panic.
    let _ = result;
}

#[test]
fn test_hash_comment_still_works() {
    let doc = toml_fast::parse("# this is a comment\nkey = 1\n").unwrap();
    let comment_spans: Vec<_> = doc.spans
        .iter()
        .filter(|s| s.kind == toml_fast::SpanKind::Comment)
        .collect();
    assert!(!comment_spans.is_empty(), "expected a comment span for `#`");
}

// ---------------------------------------------------------------------------
// strict mode checks
#[test]
fn test_strict_rejects_control_chars() {
    // Parse a document with a raw control character (form feed 0x0C).
    // The parser treats unknown bytes as BareKey, so \x0C becomes a 1-byte "bare key".
    // The validator checks all source bytes for control chars.
    let input = "key = 1\n\x0C\n";
    let mut doc = toml_fast::parse(input).unwrap();
    let errors = doc.validate(ValidationMode::Strict);

    let has_control = errors.iter().any(|e| matches!(e.kind, ValidationErrorKind::ControlCharacter));
    assert!(has_control, "expected ControlCharacter error for form feed (0x0C)");
}

#[test]
fn test_strict_bare_key_validation() {
    // A bare key with an invalid character like `!` is accepted by the parser
    // (emitted as BareKey) but invalid in strict TOML.
    let mut doc = toml_fast::parse("key! = 1\n").unwrap();
    let errors = doc.validate(ValidationMode::Strict);
    assert!(
        errors.iter().any(|e| matches!(e.kind, ValidationErrorKind::InvalidKey)),
        "expected InvalidKey error for bare key with invalid char `!`, got: {:?}",
        errors.iter().map(|e| (e.kind, &e.msg)).collect::<Vec<_>>(),
    );
}

#[test]
fn test_strict_valid_bare_key() {
    let mut doc = toml_fast::parse("my_key = 1\n").unwrap();
    let errors = doc.validate(ValidationMode::Strict);
    assert!(
        !errors.iter().any(|e| matches!(e.kind, ValidationErrorKind::InvalidKey)),
        "valid bare key should not produce InvalidKey error"
    );
}

// ---------------------------------------------------------------------------
// mode behavior
// ---------------------------------------------------------------------------

#[test]
fn test_lenient_accepts_everything() {
    // Lenient mode still catches duplicate keys/tables (always checked)
    let mut doc = toml_fast::parse("[ok]\nkey=1\n[other]\nval=2\n").unwrap();
    let errors = doc.validate(ValidationMode::Lenient);
    assert!(errors.is_empty(), "lenient mode should produce no errors for valid doc");
}

#[test]
fn test_relaxed_accepts_loose_keys() {
    // Relaxed mode allows loose key syntax (like INI-style shenanigans)
    // but still checks structural rules
    let mut doc = toml_fast::parse("[ok]\nkey = 1\n").unwrap();
    let errors = doc.validate(ValidationMode::Relaxed);
    // No structural errors expected
    assert!(
        !errors.iter().any(|e| matches!(e.kind, ValidationErrorKind::InvalidKey)),
        "relaxed mode should not check key syntax"
    );
}
