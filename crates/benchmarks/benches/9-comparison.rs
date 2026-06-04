//! Comparison benchmarks: tomlini vs toml_edit on real-world edit patterns.
//!
//! Run:  cargo bench -p toml_benchmarks --bench 9-comparison
//!
//! Each benchmark pair uses the same input and performs the same semantic
//! edit through each crate's native API.  Timings include parse + edit +
//! serialize (document-level `to_string()`) so every crate pays its own
//! serialization cost.

#![allow(elided_lifetimes_in_paths)]

// ── fixtures ──────────────────────────────────────────────────

const CARGO_MID: &str = "\
[package]
name = \"my-cli\"
version = \"2.1.0\"
edition = \"2024\"
description = \"A small command-line tool\"
license = \"MIT OR Apache-2.0\"

[dependencies]
clap = { version = \"4\", features = [\"derive\"] }
serde = { version = \"1\", features = [\"derive\"] }
serde_json = \"1\"
tokio = { version = \"1\", features = [\"full\"], optional = true }

[features]
default = [\"tokio\"]
";

const WITH_TABLES: &str = "\
[server]
host = \"localhost\"
port = 8080

[database]
url = \"postgres://localhost:5432\"
pool_size = 10
tls = true
";

// ── tomlini ───────────────────────────────────────────────────

fn tomlini_parse(src: &str) -> tomlini::FlatDoc { tomlini::parse(src).unwrap() }

fn tomlini_set(doc: &mut tomlini::FlatDoc, path: &str, val: &str) {
    doc.edit().set(path, val).commit().unwrap();
}
fn tomlini_insert(doc: &mut tomlini::FlatDoc, table: &str, key: &str, val: &str) {
    doc.edit().insert(table, key, val).commit().unwrap();
}
fn tomlini_remove(doc: &mut tomlini::FlatDoc, path: &str) {
    doc.edit().remove(path).commit().unwrap();
}
fn tomlini_rename_section(doc: &mut tomlini::FlatDoc, from: &str, to: &str) {
    doc.edit().rename_section(from, to).commit().unwrap();
}

// ── toml_edit ─────────────────────────────────────────────────

fn toml_edit_parse(src: &str) -> toml_edit::DocumentMut { src.parse().unwrap() }

fn toml_edit_set(doc: &mut toml_edit::DocumentMut, path: &str, val: &str) {
    doc[path] = toml_edit::value(val);
}
fn toml_edit_insert(doc: &mut toml_edit::DocumentMut, table: &str, key: &str, val: &str) {
    doc[table][key] = toml_edit::value(val);
}
fn toml_edit_remove(doc: &mut toml_edit::DocumentMut, path: &str) {
    doc.remove(path);
}
fn toml_edit_rename_section(doc: &mut toml_edit::DocumentMut, from: &str, to: &str) {
    if let Some(t) = doc.remove(from) { doc.insert(to, t); }
}

// ── parse-only ────────────────────────────────────────────────

#[divan::bench]
fn parse_cargo_mid(bencher: divan::Bencher) {
    bencher
        .with_inputs(|| CARGO_MID)
        .bench_values(|src| { let _ = tomlini_parse(src); });
}

#[divan::bench]
fn parse_cargo_mid_toml_edit(bencher: divan::Bencher) {
    bencher
        .with_inputs(|| CARGO_MID)
        .bench_values(|src| { let _ = toml_edit_parse(src); });
}

// ── set (change a value) ─────────────────────────────────────

#[divan::bench]
fn set_value(bencher: divan::Bencher) {
    bencher.bench(|| -> String {
        let mut doc = tomlini_parse(CARGO_MID);
        tomlini_set(&mut doc, "package.version", "\"999.0.0\"");
        doc.to_string()
    });
}

#[divan::bench]
fn set_value_toml_edit(bencher: divan::Bencher) {
    bencher.bench(|| -> String {
        let mut doc = toml_edit_parse(CARGO_MID);
        toml_edit_set(&mut doc, "package.version", "999.0.0");
        doc.to_string()
    });
}

// ── insert (add new key) ──────────────────────────────────────

#[divan::bench]
fn insert_key(bencher: divan::Bencher) {
    bencher.bench(|| -> String {
        let mut doc = tomlini_parse(CARGO_MID);
        tomlini_insert(&mut doc, "package", "homepage", "\"https://example.com\"");
        doc.to_string()
    });
}

#[divan::bench]
fn insert_key_toml_edit(bencher: divan::Bencher) {
    bencher.bench(|| -> String {
        let mut doc = toml_edit_parse(CARGO_MID);
        toml_edit_insert(&mut doc, "package", "homepage", "\"https://example.com\"");
        doc.to_string()
    });
}

// ── remove ────────────────────────────────────────────────────

#[divan::bench]
fn remove_key(bencher: divan::Bencher) {
    bencher.bench(|| -> String {
        let mut doc = tomlini_parse(CARGO_MID);
        tomlini_remove(&mut doc, "dependencies.serde_json");
        doc.to_string()
    });
}

#[divan::bench]
fn remove_key_toml_edit(bencher: divan::Bencher) {
    bencher.bench(|| -> String {
        let mut doc = toml_edit_parse(CARGO_MID);
        toml_edit_remove(&mut doc, "dependencies.serde_json");
        doc.to_string()
    });
}

// ── rename section ────────────────────────────────────────────

#[divan::bench]
fn rename_section(bencher: divan::Bencher) {
    bencher.bench(|| -> String {
        let mut doc = tomlini_parse(WITH_TABLES);
        tomlini_rename_section(&mut doc, "database", "db");
        doc.to_string()
    });
}

#[divan::bench]
fn rename_section_toml_edit(bencher: divan::Bencher) {
    bencher.bench(|| -> String {
        let mut doc = toml_edit_parse(WITH_TABLES);
        toml_edit_rename_section(&mut doc, "database", "db");
        doc.to_string()
    });
}

// ── chain: set + insert + remove ──────────────────────────────

#[divan::bench]
fn chain_set_insert_remove(bencher: divan::Bencher) {
    bencher.bench(|| -> String {
        let mut doc = tomlini_parse(CARGO_MID);
        doc.edit()
            .set("package.version", "\"999.0.0\"")
            .insert("package", "homepage", "\"https://example.com\"")
            .remove("dependencies.serde_json")
            .commit().unwrap();
        doc.to_string()
    });
}

#[divan::bench]
fn chain_set_insert_remove_toml_edit(bencher: divan::Bencher) {
    bencher.bench(|| -> String {
        let mut doc = toml_edit_parse(CARGO_MID);
        doc["package.version"] = toml_edit::value("999.0.0");
        doc["package"]["homepage"] = toml_edit::value("\"https://example.com\"");
        doc.remove("dependencies.serde_json");
        doc.to_string()
    });
}

fn main() { divan::main() }
