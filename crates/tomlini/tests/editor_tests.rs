//! Tests for the batch editor.

use tomlini::{parse, EditError, editor::Editor, editor::BringAlong};

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

// ============================================================
// reorder_root
// ============================================================

#[test]
fn test_reorder_root_scalars_before_tables() {
    // Scalars MUST come before tables in valid TOML
    let input = "root = 1\nbase = \"my-base\"\nprofiles = [\"a\", \"b\"]\n[meta]\nkind = \"leaf\"\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.reorder_root(&["meta", "base", "profiles", "root"]).commit(&mut doc).unwrap();
    let out = doc.to_string();
    let meta_pos = out.find("[meta]").unwrap();
    let base_pos = out.find("base =").unwrap();
    let prof_pos = out.find("profiles =").unwrap();
    assert!(meta_pos < base_pos, "[meta] should be before base: {out}");
    assert!(base_pos < prof_pos, "base should be before profiles: {out}");
}

#[test]
fn test_reorder_root_preserves_comments() {
    let input = "# top comment\nbase = \"my-base\"\n[meta]\nkind = \"leaf\"\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.reorder_root(&["meta", "base"]).commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("# top comment"), "top comment lost: {out}");
}

#[test]
fn test_reorder_root_noop() {
    let input = "base = \"my-base\"\n[mypackage]\nname = \"mypackage\"\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    // Already in this order — should be a no-op
    editor.reorder_root(&["base", "mypackage"]).commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("base = \"my-base\""));
    assert!(out.contains("[mypackage]"));
}

#[test]
fn test_reorder_root_fluent() {
    let input = "base = \"my-base\"\n[meta]\nkind = \"leaf\"\n";
    let mut doc = parse(input).unwrap();
    doc.edit().reorder_root(&["meta", "base"]).commit().unwrap();
    let out = doc.to_string();
    let meta_pos = out.find("[meta]").unwrap();
    let base_pos = out.find("base =").unwrap();
    assert!(meta_pos < base_pos, "[meta] should be before base: {out}");
}


// ── reorder_root edge cases ───────────────────────────────────

#[test]
fn test_reorder_root_empty_document() {
    let mut doc = tomlini::FlatDoc::new();
    let mut editor = tomlini::editor::Editor::new();
    editor.reorder_root(&["anything"]).commit(&mut doc).unwrap();
    assert_eq!(doc.to_string(), "");
}

#[test]
fn test_reorder_root_entry_not_in_doc_is_skipped() {
    let input = "base = \"my-base\"\n[meta]\nkind = \"leaf\"\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    // "ghost" is not in the document — silently skipped. Listed entries keep.
    editor.reorder_root(&["ghost", "base", "meta"]).commit(&mut doc).unwrap();
    let out = doc.to_string();
    let base_pos = out.find("base =").unwrap();
    let meta_pos = out.find("[meta]").unwrap();
    assert!(base_pos < meta_pos, "base should be before [meta]: {out}");
}

#[test]
fn test_reorder_root_entry_in_doc_not_in_order_is_dropped() {
    let input = "a = 1\nb = 2\nc = 3\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    // Only list "c" and "a" — "b" is omitted and should disappear
    editor.reorder_root(&["c", "a"]).commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(!out.contains("b ="), "\"b\" should be dropped when not in order list: {out}");
    assert!(out.contains("c ="), "\"c\" should be present: {out}");
    assert!(out.contains("a ="), "\"a\" should be present: {out}");
}

#[test]
fn test_reorder_root_table_only_document() {
    let input = "[z]\nk = 1\n[a]\nk = 2\n[m]\nk = 3\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.reorder_root(&["a", "m", "z"]).commit(&mut doc).unwrap();
    let out = doc.to_string();
    let a_pos = out.find("[a]").unwrap();
    let m_pos = out.find("[m]").unwrap();
    let z_pos = out.find("[z]").unwrap();
    assert!(a_pos < m_pos, "[a] before [m]: {out}");
    assert!(m_pos < z_pos, "[m] before [z]: {out}");
}

#[test]
fn test_reorder_root_single_entry_is_noop() {
    let input = "[only]\nk = 1\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.reorder_root(&["only"]).commit(&mut doc).unwrap();
    assert!(doc.to_string().contains("[only]"));
}

#[test]
fn test_reorder_root_preserves_inter_entry_comments() {
    // Comments BETWEEN root entries must survive reorder_root
    let input = "base = \"my-base\"\n# comment before meta\n[meta]\nkind = \"leaf\"\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.reorder_root(&["meta", "base"]).commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("# comment before meta"),
        "comment between entries was dropped during reorder_root: {out}");
}

#[test]
fn test_reorder_root_following_anchor_moves_comment_with_section() {
    // Comment preceding a section header moves with the section in Following mode
    let input = "base = \"my-base\"\n# comment for meta\n[meta]\nkind = \"leaf\"\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.reorder_root_bring(&["meta", "base"], tomlini::editor::BringAlong::COMMENTS_ABOVE).commit(&mut doc).unwrap();
    let out = doc.to_string();
    let comment_pos = out.find("# comment for meta").unwrap();
    let meta_pos = out.find("[meta]").unwrap();
    assert!(comment_pos < meta_pos,
        "comment should precede [meta] with Following anchor: {out}");
}

// ============================================================
#[test]
fn test_pont_pipeline_reorder_tables() {
    // Simulates the pont fmt pipeline: parse a profile-style TOML config,
    // reorder root-level entries so that [base] comes first, then [meta],
    // then [profiles].
    let input = "\
# My project profiles
[profiles.dev]
env = { FOO = \"dev\" }
build = \"cargo build\"

[meta]
name = \"my-project\"

[base]
extends = []
build = \"echo hi\"
";
    let mut doc = parse(input).unwrap();

    // Step 1: compute desired order from doc.keys() + policy
    // Verify root entries before reordering
    let _keys = doc.keys(); // verify index is populated
    assert_eq!(_keys.len(), 3, "expected 3 root entries: {_keys:?}");
    // Policy: [base], [meta], [profiles]
    let order = &["base", "meta", "profiles"];

    // Step 2: reorder
    doc.edit().reorder_root(order).commit().unwrap();

    // Step 3: verify
    let out = doc.to_string();
    let base_pos = out.find("[base]").unwrap();
    let meta_pos = out.find("[meta]").unwrap();
    // Dotted headers keep their full name as written
    let profiles_pos = out.find("[profiles.dev]").unwrap();
    assert!(base_pos < meta_pos, "[base] before [meta]: {out}");
    assert!(meta_pos < profiles_pos, "[meta] before [profiles.dev]: {out}");
    assert!(out.contains("name = \"my-project\""));
    assert!(out.contains("FOO = \"dev\""));
    assert!(out.contains("# My project profiles"), "top comment lost: {out}");
}

#[test]
fn test_pont_pipeline_promote_then_reorder() {
    // Simulates the pont fmt pipeline when scalars have been absorbed
    // into [meta] (TOML spec footgun).  Promote them back to root,
    // then reorder scalars before tables.
    let input = "\
# project config
[meta]
name = \"my-project\"

[meta]
base = \"my-base\"
kind = \"leaf\"
";
    let mut doc = parse(input).unwrap();

    // Step 1: promote absorbed scalars to root
    // (base and kind were absorbed into the second [meta] per TOML spec)
    let _keys = doc.keys(); // verify index
    doc.edit()
        .promote_key("meta.base")
        .promote_key("meta.kind")
        .commit().unwrap();

    // Step 2: verify promoted keys are at root
    assert!(doc.has("base"), "base should be at root after promote");
    assert!(doc.has("kind"), "kind should be at root after promote");

    // Step 3: reorder — scalars before tables, alphabetically
    doc.edit().reorder_root(&["base", "kind", "meta"]).commit().unwrap();

    // Step 4: verify order
    let out = doc.to_string();
    let base_pos = out.find("base =").unwrap();
    let kind_pos = out.find("kind =").unwrap();
    let meta_pos = out.find("[meta]").unwrap();
    assert!(base_pos < kind_pos, "base before kind: {out}");
    assert!(kind_pos < meta_pos, "kind before [meta]: {out}");

    // Step 5: content and comments preserved
    assert!(out.contains("name = \"my-project\""));
    assert!(out.contains("# project config"));
}

// ============================================================
// move_key
// ============================================================

#[test]
fn test_move_key_cross_table() {
    let input = "[a]\nkey = 1\n[b]\nother = 2\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.move_key("a.key", "b.key").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("[b]"), "target section missing: {out}");
    assert!(out.contains("key = 1"), "moved key missing from [b]: {out}");
    assert!(out.contains("other = 2"), "existing key lost: {out}");
}

#[test]
fn test_move_key_to_root() {
    let input = "[server]\nport = 8080\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.move_key("server.port", "port").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("port = 8080"), "moved key missing: {out}");
}

#[test]
fn test_move_key_preserves_formatting() {
    let input = "[a]\n# above\nkey = \"val\"  # inline\n[b]\nother = 2\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.move_key("a.key", "b.key").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("# above"), "above comment lost: {out}");
    assert!(out.contains("# inline"), "inline comment lost: {out}");
}

// ============================================================
// rename_key
// ============================================================

#[test]
fn test_rename_key_simple() {
    let input = "old_name = 42\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.rename_key("old_name", "new_name").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(!out.contains("old_name"), "old key still present: {out}");
    assert!(out.contains("new_name"), "new key missing: {out}");
    assert!(out.contains("42"), "value lost: {out}");
}

#[test]
fn test_rename_key_in_table() {
    let input = "[server]\nhostname = \"old\"\nport = 8080\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.rename_key("server.hostname", "server.host").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(!out.contains("hostname"), "old key still present: {out}");
    assert!(out.contains("host"), "new key missing: {out}");
    assert!(out.contains("port = 8080"), "sibling key lost: {out}");
}

#[test]
fn test_rename_key_preserves_value_formatting() {
    let input = "old_name = \"val\" # inline\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.rename_key("old_name", "new_name").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("# inline"), "inline comment lost: {out}");
}

#[test]
fn test_rename_key_fluent() {
    let input = "old_name = 42\n";
    let mut doc = parse(input).unwrap();
    doc.edit().rename_key("old_name", "new_name").commit().unwrap();
    let out = doc.to_string();
    assert!(out.contains("new_name"), "new key missing: {out}");
}

// ============================================================
// insert_section
// ============================================================

#[test]
fn test_insert_section_creates_header() {
    let input = "root = 1\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.insert_section("mysection").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("[mysection]"), "section header missing: {out}");
    assert!(out.contains("root = 1"), "existing key lost: {out}");
}

#[test]
fn test_insert_section_idempotent() {
    let input = "[mysection]\nkey = 1\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.insert_section("mysection").commit(&mut doc).unwrap();
    // Should be a no-op — section already exists
    let out = doc.to_string();
    assert_eq!(out.matches("[mysection]").count(), 1, "duplicate header: {out}");
}

#[test]
fn test_insert_section_into_empty() {
    let mut doc = tomlini::FlatDoc::new();
    let mut editor = tomlini::editor::Editor::new();
    editor.insert_section("mysection").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert_eq!(out, "[mysection]\n");
}

// ============================================================
// array_push multiline
// ============================================================

#[test]
fn test_array_push_multiline() {
    let input = "hosts = [\n  \"a\",\n  \"b\",\n]\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.array_push("hosts", "\"c\"").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("\"c\""), "new element missing: {out}");
    assert!(out.contains("\"a\""), "existing element lost: {out}");
    assert!(out.contains("\"b\""), "existing element lost: {out}");
}

// ============================================================
// array_set
// ============================================================

#[test]
fn test_array_set_first_element() {
    let input = "ports = [80, 443, 8080]\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.array_set("ports", 0, "22").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("22"), "new value missing: {out}");
    assert!(!out.contains("80,"), "old value '80,' still present: {out}");
}

#[test]
fn test_array_set_last_element() {
    let input = "ports = [80, 443, 8080]\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.array_set("ports", 2, "9090").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("9090"), "new value missing: {out}");
}
// ============================================================
// aot_push
// ============================================================

#[test]
fn test_aot_push_adds_entry() {
    let input = "[[server]]\nhost = \"a\"\nport = 1\n[[server]]\nhost = \"b\"\nport = 2\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.aot_push("server", &[("host", "\"c\""), ("port", "3")]).commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("host = \"c\""), "new entry host missing: {out}");
    assert!(out.contains("port = 3"), "new entry port missing: {out}");
}

#[test]
fn test_aot_push_first_entry() {
    let input = "[[server]]\nhost = \"a\"\nport = 1\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.aot_push("server", &[("host", "\"b\""), ("port", "2")]).commit(&mut doc).unwrap();
    let out = doc.to_string();
    let count = out.matches("[[server]]").count();
    assert_eq!(count, 2, "expected 2 [[server]] entries, got {count}: {out}");
}

// ============================================================
// aot_set
// ============================================================

#[test]
fn test_aot_set_modifies_entry() {
    let input = "[[server]]\nhost = \"a\"\nport = 1\n[[server]]\nhost = \"b\"\nport = 2\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.aot_set("server", 0, "port", "9000").commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("port = 9000"), "modified value missing: {out}");
    assert!(out.contains("port = 2"), "second entry lost: {out}");
}

// ============================================================
// ── aot_remove ───────────────────────────────────────────────

#[test]
fn test_aot_remove_first_entry() {
    let input = "[[server]]\nhost = \"a\"\nport = 1\n[[server]]\nhost = \"b\"\nport = 2\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.aot_remove("server", 0).commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(!out.contains("host = \"a\""), "removed entry 'a' still present: {out}");
    assert!(out.contains("host = \"b\""), "remaining entry 'b' lost: {out}");
    assert_eq!(out.matches("[[server]]").count(), 1, "should have 1 [[server]] entry left: {out}");
}

#[test]
fn test_aot_remove_last_entry() {
    let input = "[[server]]\nhost = \"a\"\nport = 1\n[[server]]\nhost = \"b\"\nport = 2\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.aot_remove("server", 1).commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("host = \"a\""), "remaining entry 'a' lost: {out}");
    assert!(!out.contains("host = \"b\""), "removed entry 'b' still present: {out}");
}

#[test]
fn test_aot_remove_only_entry() {
    let input = "[[server]]\nhost = \"a\"\nport = 1\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor.aot_remove("server", 0).commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(!out.contains("[[server]]"), "removed entry still present: {out}");
}

#[test]
fn test_aot_remove_out_of_bounds() {
    let input = "[[server]]\nhost = \"a\"\n";
    let mut doc = parse(input).unwrap();
    let mut e = tomlini::editor::Editor::new(); e.aot_remove("server", 99);
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::InvalidPath));
}

#[test]
fn test_aot_remove_nonexistent() {
    let input = "x = 1\n";
    let mut doc = parse(input).unwrap();
    let mut e = tomlini::editor::Editor::new(); e.aot_remove("nonexistent", 0);
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::InvalidPath));
}

#[test]
fn test_aot_remove_fluent() {
    let input = "[[server]]\nhost = \"a\"\n[[server]]\nhost = \"b\"\n";
    let mut doc = parse(input).unwrap();
    doc.edit().aot_remove("server", 1).commit().unwrap();
    let out = doc.to_string();
    assert_eq!(out.matches("[[server]]").count(), 1);
}

// ============================================================
#[test]
fn test_chain_set_remove_insert_rename() {
    let input = "[package]\nname = \"old\"\nversion = \"1.0\"\n";
    let mut doc = parse(input).unwrap();
    let mut editor = tomlini::editor::Editor::new();
    editor
        .set("package.version", "\"2.0\"")
        .remove("package.name")
        .insert("package", "license", "\"MIT\"")
        .rename_key("package.version", "package.ver")
        .commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(!out.contains("version"), "renamed key still present: {out}");
    assert!(out.contains("ver = \"2.0\""), "renamed+set value wrong: {out}");
    assert!(out.contains("license = \"MIT\""), "inserted key missing: {out}");
}

#[test]
fn test_chain_promote_then_reorder() {
    let input = "[meta]\nbase = \"my-base\"\nkind = \"leaf\"\n";
    let mut doc = parse(input).unwrap();
    // Verify promote_key actually moves base to root
    doc.edit().promote_key("meta.base").commit().unwrap();
    let promoted = doc.to_string();
    assert!(doc.has("base"), "promote_key failed: base not at root. Doc: {promoted}");
    assert!(!doc.has("meta.base"), "promote_key failed: meta.base still exists. Doc: {promoted}");
    // Then reorder
    doc.edit().reorder_root(&["base", "meta"]).commit().unwrap();
    let out = doc.to_string();
    let base_pos = out.find("base =").unwrap();
    let meta_pos = out.find("[meta]").unwrap();
    assert!(base_pos < meta_pos, "base should be before [meta]: {out}");
}

// ── build_index coverage: dotted table headers ───────────────

#[test]
fn test_index_dotted_table_header() {
    // Exercise build_index's Dot handling for [a.b.c] headers.
    let input = "[profiles.dev]\nenv = \"dev\"\nport = 8000\n";
    let mut doc = parse(input).unwrap();
    // After parse: index is None. Trigger build via accessor.
    assert!(doc.has("profiles.dev.env"), "should find profiles.dev.env");
    assert!(doc.has("profiles.dev.port"), "should find profiles.dev.port");
    assert!(!doc.has("profiles.env"), "should not find wrong path");
    // is_table on the first segment
    assert!(doc.is_table("profiles"), "profiles should be a table");
}

#[test]
fn test_index_aot_and_dotted_mixed() {
    // Exercise ArrayTableClose + Dot in build_index.
    let input = "[[server]]\nhost = \"a\"\nport = 1\n[server.http]\nenabled = true\n";
    let mut doc = parse(input).unwrap();
    // AOT key is server (first segment), and sub-keys exist
    let keys = doc.keys();
    assert!(keys.contains(&"server".to_string()), "server should be in keys: {keys:?}");
    assert!(doc.has("server.http.enabled"), "server.http.enabled should exist");
    assert!(doc.is_table("server"), "server should be a table");
}

// ============================================================
// BringAlong coverage — each flag x each _bring method
// ============================================================

// ── move_key_bring ──────────────────────────────────────────

#[test]
fn test_move_key_bring_comments_above() {
    // Comment above the key moves with the key to the new table.
    let input = "[a]\n# this describes k\nk = 1\nx = 2\n[b]\ny = 3\n";
    let mut doc = parse(input).unwrap();
    let mut editor = Editor::new();
    editor.move_key_bring("a.k", "b.k", BringAlong::COMMENTS_ABOVE).commit(&mut doc).unwrap();
    let out = doc.to_string();
    let comment_pos = out.find("# this describes k").unwrap();
    let b_start = out.find("[b]").unwrap();
    assert!(b_start < comment_pos, "comment should be inside [b] section after move: {out}");
}

#[test]
fn test_move_key_bring_comments_below() {
    let input = "[a]\nk = 1\n# comment after k\nx = 2\n[b]\ny = 3\n";
    let mut doc = parse(input).unwrap();
    let mut editor = Editor::new();
    editor.move_key_bring("a.k", "b.k", BringAlong::COMMENTS_BELOW).commit(&mut doc).unwrap();
    let out = doc.to_string();
    let comment_pos = out.find("# comment after k").unwrap();
    let b_start = out.find("[b]").unwrap();
    assert!(b_start < comment_pos, "comment below key should move to [b]: {out}");
}

#[test]
fn test_move_key_bring_everything_above() {
    // Everything above the key (including blank lines) moves.
    let input = "[a]\nx = 2\n\n# comment for k\nk = 1\n[b]\ny = 3\n";
    let mut doc = parse(input).unwrap();
    let mut editor = Editor::new();
    editor.move_key_bring("a.k", "b.k", BringAlong::EVERYTHING_ABOVE).commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("# comment for k"), "comment lost: {out}");
    // The blank line above the comment should also have moved — verify the comment is in [b]
    let comment_pos = out.find("# comment for k").unwrap();
    let b_pos = out.find("[b]").unwrap();
    assert!(b_pos < comment_pos, "everything above should be in [b] section: {out}");
}

#[test]
fn test_move_key_bring_combo_above_and_below() {
    let input = "[a]\nx = 2\n# above k\nk = 1\n# below k\ny = 3\n[b]\nz = 4\n";
    let mut doc = parse(input).unwrap();
    let mut editor = Editor::new();
    let bring = BringAlong::COMMENTS_ABOVE | BringAlong::COMMENTS_BELOW;
    editor.move_key_bring("a.k", "b.k", bring).commit(&mut doc).unwrap();
    let out = doc.to_string();
    let above_pos = out.find("# above k").unwrap();
    let below_pos = out.find("# below k").unwrap();
    let b_pos = out.find("[b]").unwrap();
    assert!(b_pos < above_pos, "# above k should be in [b]: {out}");
    assert!(b_pos < below_pos, "# below k should be in [b]: {out}");
}

// ── promote_key_bring ───────────────────────────────────────

#[test]
fn test_promote_key_bring_comments_above() {
    let input = "[meta]\nname = \"proj\"\n# base configuration\nbase = \"my-base\"\n";
    let mut doc = parse(input).unwrap();
    let mut editor = Editor::new();
    editor.promote_key_bring("meta.base", BringAlong::COMMENTS_ABOVE).commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("base = \"my-base\""), "promoted key missing: {out}");
    assert!(out.contains("# base configuration"), "comment lost: {out}");
    // Comment should be at root level, near base
    let comment_pos = out.find("# base configuration").unwrap();
    let base_pos = out.find("base =").unwrap();
    assert!(comment_pos < base_pos, "comment should precede base at root: {out}");
}

#[test]
fn test_promote_key_bring_comments_below() {
    let input = "[meta]\nname = \"proj\"\nbase = \"my-base\"\n# note about base\n";
    let mut doc = parse(input).unwrap();
    let mut editor = Editor::new();
    editor.promote_key_bring("meta.base", BringAlong::COMMENTS_BELOW).commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("# note about base"), "comment lost: {out}");
    let comment_pos = out.find("# note about base").unwrap();
    let base_pos = out.find("base =").unwrap();
    assert!(base_pos < comment_pos, "comment should follow base at root: {out}");
}

// ── move_key_create_bring ───────────────────────────────────

#[test]
fn test_move_key_create_bring_comments_above() {
    let input = "[a]\n# license info\nk = \"MIT\"\n";
    let mut doc = parse(input).unwrap();
    let mut editor = Editor::new();
    editor.move_key_create_bring("a.k", "game.k", BringAlong::COMMENTS_ABOVE).commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("[game]"), "new section not created: {out}");
    assert!(out.contains("# license info"), "comment lost: {out}");
    let comment_pos = out.find("# license info").unwrap();
    let game_pos = out.find("[game]").unwrap();
    assert!(game_pos < comment_pos, "comment should be inside [game]: {out}");
}

// ── reorder_root_bring combinations ─────────────────────────

#[test]
fn test_reorder_root_bring_combo() {
    let input = "base = \"my-base\"\n# comment for meta\n[meta]\nkind = \"leaf\"\n";
    let mut doc = parse(input).unwrap();
    let mut editor = Editor::new();
    let bring = BringAlong::COMMENTS_ABOVE | BringAlong::COMMENTS_BELOW;
    editor.reorder_root_bring(&["meta", "base"], bring).commit(&mut doc).unwrap();
    let out = doc.to_string();
    assert!(out.contains("# comment for meta"), "comment lost: {out}");
    let comment_pos = out.find("# comment for meta").unwrap();
    let meta_pos = out.find("[meta]").unwrap();
    assert!(comment_pos < meta_pos, "comment should precede [meta]: {out}");
}