//! Negative-path editor tests — error handling and edge cases.

use tomlini::{parse, editor::Editor, EditError};

// ---- set() errors ----

#[test] fn set_nonexistent_root_key() {
    let mut doc = parse("name = \"test\"\n").unwrap();
    let mut e = Editor::new(); e.set("nonexistent", "42");
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::NotFound));
}
#[test] fn set_nonexistent_nested_key() {
    let mut doc = parse("[server]\nport = 8080\n").unwrap();
    let mut e = Editor::new(); e.set("server.nonexistent", "42");
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::NotFound));
}
#[test] fn set_deeply_nonexistent_path() {
    let mut doc = parse("a = 1\n").unwrap();
    let mut e = Editor::new(); e.set("a.b.c.d", "42");
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::NotFound));
}

// ---- insert() errors ----

#[test] fn insert_into_nonexistent_table() {
    let mut doc = parse("name = \"test\"\n").unwrap();
    let mut e = Editor::new(); e.insert("nonexistent", "key", "val");
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::NotFound));
}
#[test] fn insert_into_deeply_nonexistent_table() {
    let mut doc = parse("[a]\nk = 1\n").unwrap();
    let mut e = Editor::new(); e.insert("a.b.c", "key", "val");
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::NotFound));
}

// ---- remove() errors ----

#[test] fn remove_nonexistent_root_key() {
    let mut doc = parse("name = \"test\"\n").unwrap();
    let mut e = Editor::new(); e.remove("nonexistent");
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::NotFound));
}
#[test] fn remove_nonexistent_nested_key() {
    let mut doc = parse("[server]\nport = 8080\n").unwrap();
    let mut e = Editor::new(); e.remove("server.nonexistent");
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::NotFound));
}

// ---- array operation ----

#[test] fn array_set_out_of_bounds() {
    let mut doc = parse("arr = [1, 2]\n").unwrap();
    let mut e = Editor::new(); e.array_set("arr", 10, "99");
    assert!(e.commit(&mut doc).is_err()); // InvalidPath
}
#[test] fn array_set_empty_array() {
    let mut doc = parse("arr = []\n").unwrap();
    let mut e = Editor::new(); e.array_set("arr", 0, "99");
    assert!(e.commit(&mut doc).is_err()); // InvalidPath
}
#[test] fn array_insert_out_of_bounds() {
    let mut doc = parse("arr = [1]\n").unwrap();
    let mut e = Editor::new(); e.array_insert("arr", 5, "99");
    e.commit(&mut doc).unwrap(); // NOTE: lenient — silently no-ops
}
#[test] fn array_remove_out_of_bounds() {
    let mut doc = parse("arr = [1]\n").unwrap();
    let mut e = Editor::new(); e.array_remove("arr", 5);
    assert!(e.commit(&mut doc).is_err()); // InvalidPath
}
#[test] fn array_push_nonexistent_array() {
    let mut doc = parse("name = \"test\"\n").unwrap();
    let mut e = Editor::new(); e.array_push("nonexistent", "42");
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::NotFound));
}
#[test] fn array_insert_nonexistent_array() {
    let mut doc = parse("name = \"test\"\n").unwrap();
    let mut e = Editor::new(); e.array_insert("nonexistent", 0, "42");
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::NotFound));
}

// ---- AOT operation ----

#[test] fn aot_set_nonexistent() {
    let mut doc = parse("name = \"test\"\n").unwrap();
    let mut e = Editor::new(); e.aot_set("bin", 0, "name", "replaced");
    assert!(e.commit(&mut doc).is_err()); // InvalidPath
}
#[test] fn aot_push_on_scalar() {
    let mut doc = parse("bin = \"not-an-aot\"\n").unwrap();
    let mut e = Editor::new(); e.aot_push("bin", &[("name", "alpha"), ("path", "src/a.rs")]);
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::NotFound));
}
#[test] fn aot_set_out_of_bounds() {
    let mut doc = parse("[[bin]]\nname = \"alpha\"\n").unwrap();
    let mut e = Editor::new(); e.aot_set("bin", 5, "name", "replaced");
    assert!(e.commit(&mut doc).is_err()); // InvalidPath
}

// ---- inline table errors ----

#[test] fn inline_set_nonexistent_table() {
    let mut doc = parse("name = \"test\"\n").unwrap();
    let mut e = Editor::new(); e.inline_set("nonexistent", "key", "val");
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::NotFound));
}
#[test] fn inline_set_nonexistent_key_in_table() {
    let mut doc = parse("pt = { x = 1, y = 2 }\n").unwrap();
    let mut e = Editor::new(); e.inline_set("pt", "z", "3");
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::NotFound));
}
#[test] fn inline_insert_nonexistent_table() {
    let mut doc = parse("name = \"test\"\n").unwrap();
    let mut e = Editor::new(); e.inline_insert("nonexistent", "key", "val");
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::NotFound));
}
#[test] fn inline_remove_nonexistent_table() {
    let mut doc = parse("name = \"test\"\n").unwrap();
    let mut e = Editor::new(); e.inline_remove("nonexistent", "key");
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::NotFound));
}

// ---- section operation ----

#[test] fn rename_section_to_existing_name() {
    let mut doc = parse("[a]\nk=1\n[b]\nk=2\n").unwrap();
    let mut e = Editor::new(); e.rename_section("a", "b");
    assert!(e.commit(&mut doc).is_err()); // SectionExists
}
#[test] fn rename_nonexistent_section() {
    let mut doc = parse("[a]\nk=1\n").unwrap();
    let mut e = Editor::new(); e.rename_section("nonexistent", "new");
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::NotFound));
}
#[test] fn replace_nonexistent_section() {
    let mut doc = parse("[a]\nk=1\n").unwrap();
    let mut e = Editor::new(); e.replace_section("nonexistent", &[("k", "v")]);
    e.commit(&mut doc).unwrap(); // NOTE: lenient — creates the section
}
#[test] fn clear_nonexistent_section() {
    let mut doc = parse("[a]\nk=1\n").unwrap();
    let mut e = Editor::new(); e.clear_section("nonexistent");
    e.commit(&mut doc).unwrap(); // NOTE: lenient — silently no-ops
}

// ---- empty document ----

#[test] fn set_on_empty_document() {
    let mut doc = parse("").unwrap();
    let mut e = Editor::new(); e.set("key", "val");
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::NotFound));
}
#[test] fn remove_on_empty_document() {
    let mut doc = parse("").unwrap();
    let mut e = Editor::new(); e.remove("key");
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::NotFound));
}
#[test] fn commit_empty_is_noop() {
    let mut doc = parse("k = 1\n").unwrap();
    let orig = doc.to_string();
    Editor::new().commit(&mut doc).unwrap();
    assert_eq!(doc.to_string(), orig);
}

// ---- dup ops ----

#[test] fn set_then_remove_same_key() {
    let mut doc = parse("port = 8080\nname = \"test\"\n").unwrap();
    let mut e = Editor::new(); e.set("port", "9090"); e.remove("port");
    e.commit(&mut doc).unwrap();
    assert!(!doc.to_string().contains("port"));
    assert!(doc.to_string().contains("name"));
}
#[test] fn insert_then_set_new_key() {
    let mut doc = parse("existing = 1\n").unwrap();
    let mut e = Editor::new(); e.insert("", "new-key", "2"); e.set("new-key", "3");
    assert!(e.commit(&mut doc).is_err()); // NotFound: insert+set same key races
}

// ---- chained commit after error ----

#[test] fn commit_stops_at_first_error_source_unchanged() {
    let mut doc = parse("name = \"test\"\n").unwrap();
    let original = doc.to_string();
    let mut e = Editor::new(); e.set("name", "new-name"); e.set("nonexistent", "val");
    assert!(e.commit(&mut doc).is_err());
    // After error, doc is in undefined state — ops before error may have applied
}

// ---- promote_key errors ----

#[test] fn promote_nonexistent_key() {
    let mut doc = parse("[meta]\nname = \"Test\"\n").unwrap();
    let mut e = Editor::new(); e.promote_key("meta.nope");
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::NotFound));
}

#[test] fn promote_nonexistent_table() {
    let mut doc = parse("x = 1\n").unwrap();
    let mut e = Editor::new(); e.promote_key("nonexistent.key");
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::NotFound));
}

#[test]
#[should_panic(expected = "promote_key requires a dotted path")]
fn promote_root_key_panics() {
    let _e = Editor::new();
    let mut e = Editor::new();
    e.promote_key("root_key"); // no dot → should panic
}

// ---- move_key_create errors ----

#[test] fn move_key_create_nonexistent_source() {
    let mut doc = parse("[meta]\nname = \"Test\"\n[game]\nversion = 1\n").unwrap();
    let mut e = Editor::new(); e.move_key_create("meta.nope", "game.nope");
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::NotFound));
}

#[test] fn move_key_create_from_nonexistent_table() {
    let mut doc = parse("x = 1\n").unwrap();
    let mut e = Editor::new(); e.move_key_create("nope.key", "x");
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::NotFound));
}
// ============================================================
// Additional gap-filling tests — error conditions not previously covered
// ============================================================

// ── array ops on scalar values ────────────────────────────────

#[test] fn array_push_on_scalar() {
    let mut doc = parse("x = 1\n").unwrap();
    let mut e = Editor::new(); e.array_push("x", "2");
    // resolve_array sees scalar kind != ArrayOpen → InvalidPath
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::InvalidPath),
        "array_push on scalar returns InvalidPath");
}

#[test] fn array_set_on_scalar() {
    let mut doc = parse("x = 1\n").unwrap();
    let mut e = Editor::new(); e.array_set("x", 0, "2");
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::InvalidPath),
        "array_set on scalar returns InvalidPath");
}

#[test] fn array_insert_on_scalar() {
    let mut doc = parse("x = 1\n").unwrap();
    let mut e = Editor::new(); e.array_insert("x", 0, "2");
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::InvalidPath),
        "array_insert on scalar returns InvalidPath");
}

#[test] fn array_remove_on_scalar() {
    let mut doc = parse("x = 1\n").unwrap();
    let mut e = Editor::new(); e.array_remove("x", 0);
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::InvalidPath),
        "array_remove on scalar returns InvalidPath");
}

#[test] fn aot_push_nonexistent() {
    let mut doc = parse("x = 1\n").unwrap();
    let mut e = Editor::new(); e.aot_push("nonexistent", &[("k", "v")]);
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::NotFound),
        "aot_push on nonexistent path returns NotFound");
}

// ── inline ops on non-inline tables ───────────────────────────

#[test] fn inline_set_on_non_inline() {
    let mut doc = parse("[stdtable]\nk = 1\n").unwrap();
    let mut e = Editor::new(); e.inline_set("stdtable", "k", "2");
    // resolve_inline_table returns NotFound when target is not { inline }
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::NotFound),
        "inline_set on regular [table] returns NotFound");
}

#[test] fn inline_insert_on_non_inline() {
    let mut doc = parse("[stdtable]\nk = 1\n").unwrap();
    let mut e = Editor::new(); e.inline_insert("stdtable", "k2", "2");
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::NotFound),
        "inline_insert on regular [table] returns NotFound");
}

#[test] fn inline_remove_on_non_inline() {
    let mut doc = parse("[stdtable]\nk = 1\n").unwrap();
    let mut e = Editor::new(); e.inline_remove("stdtable", "k");
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::NotFound),
        "inline_remove on regular [table] returns NotFound");
}

// ── rename_key cross-table ────────────────────────────────────

#[test] fn rename_key_cross_table() {
    let mut doc = parse("[a]\nk = 1\n[b]\nk = 2\n").unwrap();
    let mut e = Editor::new(); e.rename_key("a.k", "b.k");
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::TableMismatch),
        "rename_key cross-table returns TableMismatch");
}

#[test] fn rename_key_nonexistent_source() {
    let mut doc = parse("[a]\nk = 1\n").unwrap();
    let mut e = Editor::new(); e.rename_key("a.nonexistent", "a.new");
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::NotFound),
        "rename_key on nonexistent source returns NotFound");
}

// ── move_key nonexistent source ───────────────────────────────

#[test] fn move_key_nonexistent_source() {
    let mut doc = parse("[a]\nk = 1\n").unwrap();
    let mut e = Editor::new(); e.move_key("a.nonexistent", "b.k");
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::NotFound),
        "move_key with nonexistent source returns NotFound");
}

#[test] fn move_key_nonexistent_dest_table() {
    let mut doc = parse("[a]\nk = 1\n").unwrap();
    let mut e = Editor::new(); e.move_key("a.k", "nonexistent.k");
    assert!(matches!(e.commit(&mut doc).unwrap_err(), EditError::NotFound),
        "move_key to nonexistent dest table returns NotFound (use move_key_create)");
}
