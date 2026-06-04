//! # Batch migration — set, remove, insert, rename in one atomic commit.
//!
//! All operations resolve against the same index.  The document is updated
//! once when `commit()` returns — no intermediate states.
//!
//! Run:  cargo run --example batch-migrate

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = r#"
[package]
name = "old-name"
version = "0.1.0"
edition = "2024"
[dependencies]
serde = "1"
"#;

    let mut doc = tomlini::parse(input)?;

    doc.edit()
        .set("package.version", "\"1.0.0\"") // bump version
        .rename_key("package.name", "package.lib") // rename field
        .remove("dependencies.serde") // drop dep
        .insert("dependencies", "tokio", "1") // add dep
        .commit()?;

    println!("{}", doc);
    Ok(())
}
