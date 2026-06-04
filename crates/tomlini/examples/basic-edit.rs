//! # Editing a TOML config file — the basic workflow.
//!
//! Parse, edit, serialize.  All comments and formatting are preserved.
//!
//! Run:  cargo run --example basic-edit

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = r#"
# Database configuration
[database]
host = "localhost"
port = 5432       # default Postgres port
max_connections = 100
"#;

    // ── parse ──────────────────────────────────────────────
    let mut doc = tomlini::parse(input)?;

    // ── edit ───────────────────────────────────────────────
    doc.edit()
        .set("database.host", "\"db.example.com\"") // change host
        .set("database.port", "6432") // change port
        .insert("database", "ssl", "true") // add new key
        .commit()?;

    // ── output ─────────────────────────────────────────────
    println!("{}", doc);

    Ok(())
}
