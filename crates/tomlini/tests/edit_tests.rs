//! Tests for SAX edit operations on FlatDoc.

use tomlini::{EditError, parse};

#[test]
fn set_simple_value() {
    let mut doc = parse("port = 8080\n").unwrap();
    doc.set(&["port"], "9090").unwrap();
    assert_eq!(doc.to_string(), "port = 9090\n");
}

#[test]
fn set_value_in_table() {
    let mut doc = parse("[server]\nport = 8080\nhost = \"localhost\"\n").unwrap();
    doc.set(&["server", "port"], "9090").unwrap();
    assert_eq!(
        doc.to_string(),
        "[server]\nport = 9090\nhost = \"localhost\"\n"
    );
}

#[test]
fn set_preserves_comments() {
    let mut doc = parse("# before\nport = 8080 # inline\n").unwrap();
    doc.set(&["port"], "9090").unwrap();
    // The comment before "port" and the inline comment survive because
    // we only replace the value span
    assert!(doc.to_string().contains("# before"));
}

#[test]
fn set_not_found() {
    let mut doc = parse("port = 8080\n").unwrap();
    assert!(matches!(
        doc.set(&["nonexistent"], "1"),
        Err(EditError::NotFound)
    ));
}

#[test]
fn insert_new_key_root() {
    let mut doc = parse("name = \"hello\"\n").unwrap();
    doc.insert(&[], "version", "1.0").unwrap();
    let out = doc.to_string();
    assert!(out.contains("name = \"hello\""));
    assert!(out.contains("version = 1.0"));
}

#[test]
fn insert_new_key_in_table() {
    let mut doc = parse("[server]\nhost = \"localhost\"\n").unwrap();
    doc.insert(&["server"], "port", "8080").unwrap();
    let out = doc.to_string();
    assert!(out.contains("host = \"localhost\""));
    assert!(out.contains("port = 8080"));
}

#[test]
fn remove_key() {
    let mut doc = parse("name = \"hello\"\nversion = \"1.0\"\n").unwrap();
    doc.remove(&["version"]).unwrap();
    let out = doc.to_string();
    assert!(!out.contains("version"));
    assert!(out.contains("name"));
}

#[test]
fn remove_key_from_table() {
    let mut doc = parse("[server]\nhost = \"localhost\"\nport = 8080\n").unwrap();
    doc.remove(&["server", "port"]).unwrap();
    let out = doc.to_string();
    assert!(!out.contains("port"));
    assert!(out.contains("host"));
}

#[test]
fn remove_not_found() {
    let mut doc = parse("port = 8080\n").unwrap();
    assert!(matches!(
        doc.remove(&["nonexistent"]),
        Err(EditError::NotFound)
    ));
}

#[test]
fn set_then_insert_then_remove() {
    let mut doc = parse("name = \"hello\"\n").unwrap();

    doc.set(&["name"], "\"world\"").unwrap();
    assert!(doc.to_string().contains("\"world\""));

    doc.insert(&[], "version", "2.0").unwrap();
    assert!(doc.to_string().contains("version = 2.0"));
    assert!(doc.to_string().contains("\"world\""));

    doc.remove(&["name"]).unwrap();
    let out = doc.to_string();
    assert!(!out.contains("name"));
    assert!(!out.contains("world"));
    assert!(out.contains("version"));
}

#[test]
fn set_preserves_surrounding_structure() {
    let input =
        "[package]\nname = \"my-app\"\nversion = \"1.0\"\n\n[dependencies]\nserde = \"1.0\"\n";
    let mut doc = parse(input).unwrap();

    doc.set(&["package", "version"], "\"2.0\"").unwrap();
    let out = doc.to_string();

    // All structural elements preserved
    assert!(out.contains("[package]"));
    assert!(out.contains("name = \"my-app\""));
    assert!(out.contains("version = \"2.0\""));
    assert!(out.contains("[dependencies]"));
    assert!(out.contains("serde = \"1.0\""));
}

#[test]
fn idempotent_after_edit() {
    let mut doc = parse("port = 8080\n").unwrap();
    doc.set(&["port"], "9090").unwrap();
    // Parsing the output should produce a valid document
    let reparsed = parse(&doc.to_string()).unwrap();
    assert!(!reparsed.spans.is_empty());
}
