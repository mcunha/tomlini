//! Tests for the batch editor.

use tomlini::{parse, EditError};

// ============================================================
// Read accessors
// ============================================================

#[test]
fn test_has_existing_key() {
    let mut doc = parse("port = 8080\n").unwrap();
    assert!(doc.has("port"));
}

#[test]
fn test_has_missing_key() {
    let mut doc = parse("port = 8080\n").unwrap();
    assert!(!doc.has("nonexistent"));
}

#[test]
fn test_has_in_table() {
    let mut doc = parse("[server]\nport = 8080\n").unwrap();
    assert!(doc.has("server.port"));
}

#[test]
fn test_get_value() {
    let mut doc = parse("count = 42\n").unwrap();
    assert_eq!(doc.get("count"), Some("42"));
}

#[test]
fn test_get_quoted_value() {
    let mut doc = parse("name = \"hello\"\n").unwrap();
    assert_eq!(doc.get("name"), Some("\"hello\""));
}

// ============================================================
// Set operations
// ============================================================

#[test]
fn editor_set_value() {
    let mut doc = parse("port = 8080\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.set("port", "9090").commit(&mut doc).unwrap();
    assert_eq!(doc.to_string(), "port = 9090\n");
}

#[test]
fn editor_set_value_in_table() {
    let mut doc = parse("[server]\nport = 8080\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.set("server.port", "9090").commit(&mut doc).unwrap();
    assert!(doc.to_string().contains("port = 9090"));
}

#[test]
fn test_set_creates_valid_output() {
    let mut doc = parse("key = \"old\"\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.set("key", "\"new\"").commit(&mut doc).unwrap();
    // The output should parse successfully.
    let reparsed = parse(&doc.to_string());
    assert!(reparsed.is_ok(), "set output should be valid TOML: {:?}", reparsed.err());
}

#[test]
fn test_set_nonexistent_key() {
    let mut doc = parse("a = 1\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.set("missing", "2");
    let result = editor.commit(&mut doc);
    assert!(matches!(result, Err(EditError::NotFound)));
}

// ============================================================
// Insert operations
// ============================================================

#[test]
fn editor_insert_key() {
    let mut doc = parse("name = \"hello\"\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.insert("", "version", "1.0").commit(&mut doc).unwrap();
    assert!(doc.to_string().contains("version = 1.0"));
}

#[test]
fn test_insert_into_empty_doc() {
    let mut doc = tomlini::FlatDoc::new();
    let mut editor = tomlini::editor::Editor::new();
    editor.insert("", "key", "\"val\"").commit(&mut doc).unwrap();
    assert!(doc.to_string().contains("key = \"val\""));
}

#[test]
fn test_insert_multiple() {
    // Batch-insert several keys into a document that already has content.
    // The insert ops all resolve to the end of the root table in one pass.
    let mut doc = parse("z = 999\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.insert("", "a", "1");
    editor.insert("", "b", "2");
    editor.insert("", "c", "3");
    editor.commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("a = 1"));
    assert!(out.contains("b = 2"));
    assert!(out.contains("c = 3"));
    assert!(out.contains("z = 999"));
}

#[test]
fn editor_insert_with_comment() {
    let mut doc = parse("name = \"hello\"\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.insert("", "version", "1.0")
        .with_above_comment("The app version");
    editor.commit(&mut doc).unwrap();
    assert!(doc.to_string().contains("# The app version"));
}

#[test]
fn test_insert_with_block_comment() {
    let mut doc = parse("name = \"hello\"\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.insert("", "version", "1.0")
        .with_block_comment(&["Copyright 2024", "All rights reserved"]);
    editor.commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("# Copyright 2024"));
    assert!(out.contains("# All rights reserved"));
}

#[test]
fn test_insert_nonexistent_table() {
    let mut doc = parse("[real]\na = 1\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.insert("nonexistent", "key", "val");
    let result = editor.commit(&mut doc);
    assert!(matches!(result, Err(EditError::NotFound)));
}

// ============================================================
// Remove operations
// ============================================================

#[test]
fn editor_remove_key() {
    let mut doc = parse("name = \"hello\"\nversion = \"1.0\"\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.remove("version").commit(&mut doc).unwrap();
    assert!(!doc.to_string().contains("version"));
}

#[test]
fn test_remove_last_key() {
    let mut doc = parse("only = \"me\"\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.remove("only").commit(&mut doc).unwrap();
    // The entire line is removed. The document may be empty.
    assert!(!doc.to_string().contains("only"));
}

#[test]
fn test_remove_nonexistent_key() {
    let mut doc = parse("a = 1\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.remove("missing");
    let result = editor.commit(&mut doc);
    assert!(matches!(result, Err(EditError::NotFound)));
}

#[test]
fn test_remove_from_nonexistent_table() {
    let mut doc = parse("[real]\na = 1\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.remove("fake.key");
    let result = editor.commit(&mut doc);
    assert!(matches!(result, Err(EditError::NotFound)));
}

// ============================================================
// Chained operations
// ============================================================

#[test]
fn editor_chained_ops() {
    let mut doc = parse("[server]\nhost = \"localhost\"\nport = 8080\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor
        .set("server.port", "9090")
        .insert("server", "timeout", "30")
        .with_above_comment("Connection timeout")
        .remove("server.host");
    editor.commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("port = 9090"));
    assert!(out.contains("timeout = 30"));
    assert!(out.contains("# Connection timeout"));
    assert!(!out.contains("host"));
}

#[test]
fn test_chain_set_and_remove_same_key() {
    // Both set and remove targeting the same key in one commit:
    // remove wins because it removes the entire line.
    let mut doc = parse("x = 1\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.set("x", "2").remove("x");
    editor.commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(!out.contains("x"), "key should be removed, but got: {out}");
}

#[test]
fn test_chain_ten_ops() {
    let mut doc = tomlini::FlatDoc::new();
    let mut editor = tomlini::editor::Editor::new();
    for i in 0..10 {
        let key = format!("k{i}");
        let val = format!("{i}");
        editor.insert("", &key, &val);
    }
    editor.commit(&mut doc).unwrap();
    let out = doc.to_string();
    for i in 0..10 {
        assert!(out.contains(&format!("k{i} = {i}")));
    }
}

// ============================================================
// Empty operations
// ============================================================

#[test]
fn test_empty_commit() {
    let mut doc = parse("key = 1\n").unwrap();
    let original = doc.to_string();
    let mut editor = tomlini::editor::Editor::new();
    editor.commit(&mut doc).unwrap();
    assert_eq!(doc.to_string(), original);
}

#[test]
fn test_document_new_is_empty() {
    let doc = tomlini::FlatDoc::new();
    assert!(doc.to_string().is_empty());
}

// ============================================================
// Formatting preservation
// ============================================================

#[test]
fn editor_set_preserves_comment() {
    let mut doc = parse("port = 8080 # default\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.set("port", "9090").commit(&mut doc).unwrap();
    assert!(doc.to_string().contains("# default"));
}

#[test]
fn test_set_preserves_comment_before_key() {
    let mut doc = parse("# the port number\nport = 8080\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.set("port", "9090").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("# the port number"), "comment above key must survive: {out}");
    assert!(out.contains("port = 9090"));
}

#[test]
fn test_insert_copies_neighbor_indent() {
    let mut doc = parse("[server]\n  host = \"localhost\"\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.insert("server", "port", "8080").commit(&mut doc).unwrap();
    let out = doc.to_string();
    // The inserted line should copy the 2-space indent.
    assert!(out.contains("  port = 8080"), "expected 2-space indent on new key: {out}");
}

#[test]
fn test_remove_preserves_adjacent_formatting() {
    let mut doc = parse("key1 = \"a\"\nkey2 = \"b\"\nkey3 = \"c\"\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.remove("key2").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("key1 = \"a\""));
    assert!(!out.contains("key2"));
    assert!(out.contains("key3 = \"c\""));
}

#[test]
fn get_decoded_basic_string() {
    let mut doc = parse(r#"key = "hello\nworld""#).unwrap();
    let decoded = doc.get_decoded("key").unwrap();
    assert_eq!(decoded, "hello\nworld");
}

#[test]
fn get_decoded_literal_string() {
    let mut doc = parse(r"key = 'hello\nworld'").unwrap();
    let decoded = doc.get_decoded("key").unwrap();
    assert_eq!(decoded, r"hello\nworld");
}

#[test]
fn get_decoded_integer_returns_raw() {
    let mut doc = parse("key = 42").unwrap();
    let decoded = doc.get_decoded("key").unwrap();
    assert_eq!(decoded, "42");
}

#[test]
fn doc_keys_root() {
    let mut doc = parse("name = \"app\"\nversion = \"1.0\"\n[server]\nport = 8080\n").unwrap();
    let keys = doc.keys();
    assert!(keys.contains(&"name".to_string()));
    assert!(keys.contains(&"version".to_string()));
    assert!(keys.contains(&"server".to_string()));
}

#[test]
fn doc_is_table_true() {
    let mut doc = parse("[server]\nport = 8080\n").unwrap();
    assert!(doc.is_table("server"));
}

#[test]
fn doc_is_table_false_for_scalar() {
    let mut doc = parse("port = 8080\n").unwrap();
    assert!(!doc.is_table("port"));
}

#[test]
fn doc_is_table_inline() {
    let mut doc = parse("colors = { red = \"#ff0000\" }\n").unwrap();
    assert!(doc.is_table("colors"));
}


// ============================================================
// Array insert / remove
// ============================================================

#[test]
fn test_array_insert() {
    let mut doc = parse("arr = [1, 2, 3]\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.array_insert("arr", 1, "99").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("99"), "inserted value not found in: {out}");
    assert!(out.contains("1"));
    assert!(out.contains("2"));
    assert!(out.contains("3"));
}

#[test]
fn test_array_insert_at_end() {
    let mut doc = parse("arr = [1, 2, 3]\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.array_insert("arr", 3, "4").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("4"));
    assert!(out.contains("3"));
}

#[test]
fn test_array_insert_into_empty() {
    let mut doc = parse("arr = []\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.array_insert("arr", 0, "1").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("1"));
}

#[test]
fn test_array_remove() {
    let mut doc = parse("arr = [1, 2, 3]\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.array_remove("arr", 1).commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("1"));
    assert!(!out.contains("2"));
    assert!(out.contains("3"));
}

#[test]
fn test_array_remove_first() {
    let mut doc = parse("arr = [1, 2, 3]\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.array_remove("arr", 0).commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(!out.contains('1'));
    assert!(out.contains('2'));
    assert!(out.contains('3'));
}

#[test]
fn test_array_remove_last() {
    let mut doc = parse("arr = [1, 2, 3]\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.array_remove("arr", 2).commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("1"));
    assert!(out.contains("2"));
    assert!(!out.contains('3'));
}

// ============================================================
// Section operations
// ============================================================

#[test]
fn test_replace_section() {
    let mut doc = parse("[server]\nhost = \"old\"\nport = 80\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.replace_section("server", &[("host", "\"new\""), ("timeout", "30")])
        .commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("[server]"), "header must survive: {out}");
    assert!(out.contains("host = \"new\""), "new host not found: {out}");
    assert!(out.contains("timeout = 30"), "new key not found: {out}");
    assert!(!out.contains("\"old\""), "old host should be gone: {out}");
    assert!(!out.contains("port"), "port should be gone: {out}");
}

#[test]
fn test_replace_section_new_section() {
    let mut doc = parse("root_key = 1\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.replace_section("new_sec", &[("key", "\"val\"")])
        .commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("[new_sec]"), "new section header missing: {out}");
    assert!(out.contains("key = \"val\""), "new key missing: {out}");
}

#[test]
fn test_clear_section() {
    let mut doc = parse("[server]\nhost = \"x\"\nport = 80\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.clear_section("server").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("[server]"), "header must survive: {out}");
    assert!(!out.contains("host"), "host should be removed: {out}");
    assert!(!out.contains("port"), "port should be removed: {out}");
}

#[test]
fn test_rename_section() {
    let mut doc = parse("[old]\nkey = 1\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.rename_section("old", "new").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("[new]"), "new name missing: {out}");
    assert!(!out.contains("[old]"), "old name should be gone: {out}");
    assert!(out.contains("key = 1"), "content should survive: {out}");
}

#[test]
fn test_rename_section_conflict() {
    let mut doc = parse("[a]\nk = 1\n[b]\nk = 2\n").unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.rename_section("a", "b");
    let result = editor.commit(&mut doc);
    assert!(matches!(result, Err(EditError::SectionExists)));
}

// ============================================================
// AOT operations
// ============================================================

#[test]
fn test_aot_set() {
    let input = "[[products]]\nname = \"apple\"\nprice = 5\n\n[[products]]\nname = \"banana\"\nprice = 3\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.aot_set("products", 0, "price", "10").commit(&mut doc).unwrap();
    let out = doc.to_string();
    // First product's price should be 10, second should remain 3
    assert!(out.contains("price = 10"), "first price not updated: {out}");
    assert!(out.contains("price = 3"), "second price should remain: {out}");
    assert!(out.contains("name = \"apple\""));
    assert!(out.contains("name = \"banana\""));
}

#[test]
fn test_aot_set_second_entry() {
    let input = "[[products]]\nname = \"apple\"\nprice = 5\n\n[[products]]\nname = \"banana\"\nprice = 3\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.aot_set("products", 1, "name", "\"cherry\"").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("name = \"apple\""), "first name should survive: {out}");
    assert!(out.contains("name = \"cherry\""), "second name not updated: {out}");
    assert!(!out.contains("\"banana\""));
}

#[test]
fn test_aot_set_nonexistent_key() {
    let input = "[[products]]\nname = \"apple\"\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.aot_set("products", 0, "missing", "99");
    let result = editor.commit(&mut doc);
    assert!(matches!(result, Err(EditError::NotFound)));
}

#[test]
fn test_aot_set_bad_index() {
    let input = "[[products]]\nname = \"apple\"\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.aot_set("products", 5, "name", "\"x\"");
    let result = editor.commit(&mut doc);
    assert!(matches!(result, Err(EditError::InvalidPath)));
}

// ============================================================
// Inline table operations
// ============================================================

#[test]
fn test_inline_insert() {
    let input = "key = {a = 1, b = 2}\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.inline_insert("key", "c", "3").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("key = {a = 1, b = 2, c = 3}"), "unexpected output: {out}");
}

#[test]
fn test_inline_insert_empty() {
    let input = "key = {}\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.inline_insert("key", "a", "1").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("key = {a = 1}"), "unexpected output: {out}");
}

#[test]
fn test_inline_remove() {
    let input = "key = {a = 1, b = 2, c = 3}\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.inline_remove("key", "b").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("key = {a = 1,c = 3}"), "unexpected output: {out}");
}

#[test]
fn test_inline_remove_last() {
    let input = "key = {a = 1}\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.inline_remove("key", "a").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("{}"), "unexpected output: {out}");
}

// ============================================================
// promote_key
// ============================================================

#[test]
fn test_promote_key_simple() {
    let input = "[meta]\nname = \"Test\"\nbase = \"my-base\"\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.promote_key("meta.base").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("base = \"my-base\""), "promoted key missing: {out}");
    assert!(!out.contains("meta.base"), "dotted path still present: {out}");
}

#[test]
fn test_promote_key_preserves_formatting() {
    let input = "[meta]\n# above comment\nbase = \"val\"  # inline\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.promote_key("meta.base").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("# above comment"), "above comment lost: {out}");
    assert!(out.contains("# inline"), "inline comment lost: {out}");
}

#[test]
fn test_promote_key_last_in_table() {
    // When the promoted key is the last in the table, the table may become empty
    let input = "[meta]\nbase = \"my-base\"\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.promote_key("meta.base").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("base = \"my-base\""), "promoted key missing: {out}");
}

#[test]
fn test_promote_key_with_fluent_handle() {
    let input = "[meta]\nbase = \"my-base\"\n";
    let mut doc = parse(input).unwrap();
    doc.edit().promote_key("meta.base").commit().unwrap();
    let out = doc.to_string();
    assert!(out.contains("base = \"my-base\""), "promoted key missing: {out}");
}

#[test]
fn test_promote_key_from_deep_table() {
    let input = "[a.b.c]\nkey = 42\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.promote_key("a.b.c.key").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("key = 42"), "promoted key missing: {out}");
}

// ============================================================
// move_key_create
// ============================================================

#[test]
fn test_move_key_create_new_section() {
    let input = "[meta]\nname = \"Test\"\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.move_key_create("meta.name", "game.name").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("[game]"), "new section header missing: {out}");
    assert!(out.contains("name = \"Test\""), "moved key missing: {out}");
}

#[test]
fn test_move_key_create_existing_section() {
    // When destination exists, behaves like regular move_key
    let input = "[meta]\nname = \"Test\"\n[game]\nversion = 1\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.move_key_create("meta.name", "game.name").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("[game]"), "section missing: {out}");
    assert!(out.contains("name = \"Test\""), "moved key missing: {out}");
    assert!(out.contains("version = 1"), "existing key lost: {out}");
}

#[test]
fn test_move_key_create_to_root() {
    let input = "[meta]\nname = \"Test\"\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.move_key_create("meta.name", "name").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("name = \"Test\""), "moved key missing: {out}");
}

#[test]
fn test_move_key_create_preserves_formatting() {
    let input = "[meta]\n# comment\nname = \"Test\"  # inline\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.move_key_create("meta.name", "game.name").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("# comment"), "above comment lost: {out}");
    assert!(out.contains("# inline"), "inline comment lost: {out}");
}

#[test]
fn test_move_key_create_with_fluent_handle() {
    let input = "[meta]\nname = \"Test\"\n";
    let mut doc = parse(input).unwrap();
    doc.edit().move_key_create("meta.name", "game.name").commit().unwrap();
    let out = doc.to_string();
    assert!(out.contains("[game]"), "new section header missing: {out}");
    assert!(out.contains("name = \"Test\""), "moved key missing: {out}");
}