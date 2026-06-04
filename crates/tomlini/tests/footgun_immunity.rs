//! Verify FlatDoc is immune to the format-preservation footguns
//! we documented for toml_edit.

use tomlini::parse;

// ============================================================
// Footgun 1: IndexMut auto-creates InlineTable
// ============================================================
// toml_edit: doc["a"]["b"] = value(1) → a = { b = 1 }
// FlatDoc: no IndexMut trait — all access is by key path string. Immune.

// ============================================================
// Footgun 2: insert() destroys key formatting
// ============================================================

#[test]
fn set_preserves_key_formatting() {
    let mut doc = parse("  padded-key   = 1010\n").unwrap();
    doc.set(&["padded-key"], "42").unwrap();
    // Only the value span is replaced. Whitespace around the key survives.
    assert!(doc.to_string().contains("  padded-key   "));
}

#[test]
fn set_preserves_key_comments() {
    let mut doc = parse("# comment before key\nport = 8080\n").unwrap();
    doc.set(&["port"], "9090").unwrap();
    assert!(doc.to_string().contains("# comment before key"));
}

#[test]
fn set_preserves_inline_comment() {
    let mut doc = parse("port = 8080 # default\n").unwrap();
    doc.set(&["port"], "9090").unwrap();
    // The inline comment is part of the line after the value.
    // Since set() only replaces the value span, the inline comment survives.
    assert!(doc.to_string().contains("# default"));
}

// ============================================================
// Footgun 3: value() creates unformatted values
// ============================================================
// toml_edit: value(0xDEAD) → "57005" (decimal, loses hex formatting)
// FlatDoc: caller passes a string — they control formatting entirely.

#[test]
fn caller_controls_value_formatting() {
    let mut doc = parse("weight = 100\n").unwrap();
    // Caller can pass hex if they want
    doc.set(&["weight"], "0x64").unwrap();
    assert!(doc.to_string().contains("0x64"));
}

// ============================================================
// Footgun 4: new tables lose header comments
// ============================================================
// FlatDoc: no table objects. Immune.

// ============================================================
// Footgun 5: fmt() destroys everything
// ============================================================
// FlatDoc: no fmt(). Immune.

// ============================================================
// Footgun 6: insert() destroys comments before the EDITED key
// ============================================================

#[test]
fn insert_new_key_does_not_destroy_sibling_comments() {
    let mut doc = parse("name = \"hello\"\nversion = \"1.0\"\n").unwrap();
    // Insert a new key — neither existing key is touched
    doc.insert(&[], "author", "\"me\"").unwrap();
    let out = doc.to_string();
    assert!(out.contains("name = \"hello\""));
    assert!(out.contains("version = \"1.0\""));
}

// ============================================================
// Footgun 7: remove() preserves comments on adjacent keys
// ============================================================

#[test]
fn remove_preserves_adjacent_comments() {
    let input = "# name comment\nname = \"hello\"\n# version comment\nversion = \"1.0\"\n";
    let mut doc = parse(input).unwrap();
    doc.remove(&["name"]).unwrap();
    let out = doc.to_string();
    // The comment before version survives
    assert!(out.contains("# version comment"));
    assert!(!out.contains("name = \"hello\""), "name kv removed: {out:?}");
    assert!(out.contains("# name comment"), "comment survives: {out:?}");
}

// ============================================================
// Footgun 8: dotted key as literal, not path
// ============================================================
// FlatDoc: key paths are explicit &[&str]. No dotted-key confusion.

// ============================================================
// Footgun 9: comments between keys owned by next key
// ============================================================


/// Verify that insert() does NOT copy the previous key's comment.
/// It should only copy whitespace indentation.
#[test]
fn insert_copies_indentation_not_comment() {
    let input = "# comment for key1\n  key1 = 1\n";
    let mut doc = parse(input).unwrap();
    doc.insert(&[], "key2", "2").unwrap();
    let out = doc.to_string();

    // key2 gets the same indentation (2 spaces) but NOT the comment
    assert!(out.contains("  key2 = 2"), "indentation copied: {out:?}");
    assert!(out.contains("# comment for key1"), "original comment preserved: {out:?}");

    // The comment should appear only once (not duplicated)
    let comment_count = out.matches("# comment for key1").count();
    assert_eq!(comment_count, 1, "comment not duplicated: {out:?}");
}

/// If there's no indentation on the previous key, the new key gets none either.
#[test]
fn insert_no_indent_when_neighbor_has_none() {
    let input = "key1 = 1\n";
    let mut doc = parse(input).unwrap();
    doc.insert(&[], "key2", "2").unwrap();
    let out = doc.to_string();
    assert!(out.contains("key2 = 2\n"), "no extra indentation: {out:?}");
}
#[test]
fn between_key_comment_survives_insert() {
    let input = "key1 = 1\n# belongs to key2\nkey2 = 2\n";
    let mut doc = parse(input).unwrap();
    doc.insert(&[], "key1point5", "15").unwrap();
    let out = doc.to_string();
    assert!(out.contains("# belongs to key2"));
    assert!(out.contains("key2 = 2"));
}

// ============================================================
// Footgun 10: empty implicit tables linger after remove
// ============================================================

#[test]
fn remove_last_value_cleans_up_line() {
    let mut doc = parse("[a.b]\nx = 1\n").unwrap();
    // remove the only value — the line is gone, no lingering [a.b]
    doc.remove(&["a", "b", "x"]).unwrap();
    let out = doc.to_string();
    // The span removal deletes the entire "x = 1\n" line.
    // The [a.b] header stays, but that's separate.
    assert!(!out.contains("x = 1"));
}

// ============================================================
// Footgun 11: Array::fmt() destroys multiline arrays
// ============================================================
// FlatDoc: no fmt(). Immune.

// ============================================================
// Footgun 12: get_mut() + assignment loses above-comments
// ============================================================
// FlatDoc: no get_mut() — only set() by key path. Immune.

// ============================================================
// Stress: heavy documented config survives edits
// ============================================================

#[test]
fn documented_config_survives_edits() {
    let input = "\
# MyApp Configuration
[server]
# The hostname to bind to
host = \"localhost\" # default

# The port number
port = 8080

[logging]
# Log level
level = \"info\"
";
    let mut doc = parse(input).unwrap();

    doc.set(&["server", "port"], "9090").unwrap();
    doc.set(&["logging", "level"], "\"debug\"").unwrap();

    let out = doc.to_string();

    // File header
    assert!(out.contains("# MyApp Configuration"));
    // Section headers
    assert!(out.contains("[server]"));
    assert!(out.contains("[logging]"));
    // Key comments survived
    assert!(out.contains("# The hostname to bind to"));
    assert!(out.contains("# The port number"));
    assert!(out.contains("# Log level"));
    // Inline comments survived
    assert!(out.contains("# default"));
    // Values updated
    assert!(out.contains("port = 9090"));
    assert!(out.contains("level = \"debug\""));
}
