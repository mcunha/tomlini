//! Edit operation benchmarks for `tomlini`.

#![allow(elided_lifetimes_in_paths)]

use toml_benchmarks::{Data, MANIFESTS};

// ---- Batch editor ----

#[divan::bench(args = MANIFESTS)]
fn editor_set_value(bencher: divan::Bencher) {
    bencher.bench(|| -> String {
        let mut doc = tomlini::parse(MANIFESTS[0].content()).unwrap();
        let mut editor = tomlini::editor::Editor::new();
        editor.set("package.version", "2.0.0");
        editor.commit(&mut doc).unwrap();
        doc.to_string()
    });
}

#[divan::bench(args = MANIFESTS)]
fn editor_insert_key(bencher: divan::Bencher) {
    bencher.bench(|| -> String {
        let mut doc = tomlini::parse(MANIFESTS[0].content()).unwrap();
        let mut editor = tomlini::editor::Editor::new();
        editor.insert("package", "bench-key", "42");
        editor.commit(&mut doc).unwrap();
        doc.to_string()
    });
}

#[divan::bench(args = MANIFESTS)]
fn editor_remove_key(bencher: divan::Bencher) {
    bencher.bench(|| -> String {
        let mut doc = tomlini::parse(MANIFESTS[0].content()).unwrap();
        let mut editor = tomlini::editor::Editor::new();
        editor.remove("package.version");
        editor.commit(&mut doc).unwrap();
        doc.to_string()
    });
}

#[divan::bench(args = MANIFESTS)]
fn editor_chained_ops(bencher: divan::Bencher) {
    bencher.bench(|| -> String {
        let mut doc = tomlini::parse(MANIFESTS[0].content()).unwrap();
        let mut editor = tomlini::editor::Editor::new();
        editor.set("package.version", "2.0.0");
        editor.insert("package", "bench-key", "42");
        editor.remove("package.edition");
        editor.commit(&mut doc).unwrap();
        doc.to_string()
    });
}

fn main() {
    divan::main();
}
