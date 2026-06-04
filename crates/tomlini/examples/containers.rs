//! # Container editing — arrays, inline tables, and array-of-tables.
//!
//! TOML containers are first-class edit targets.  This example shows
//! `array_push`, `inline_set`, and `aot_push`.
//!
//! Run:  cargo run --example containers

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = r#"
# Dev server configuration
allowed-hosts = ["localhost", "127.0.0.1"]

[defaults]
headers = { x-powered-by = "false", content-type = "text/html" }

[[backend]]
host = "10.0.0.1"
port = 8000
"#;

    let mut doc = tomlini::parse(input)?;

    doc.edit()
        // Push to an array
        .array_push("allowed-hosts", "\"10.0.0.3\"")
        // Set a key inside an inline table
        .inline_set("defaults.headers", "content-type", "\"application/json\"")
        // Push a new entry to an array-of-tables
        .aot_push("backend", &[("host", "\"10.0.0.4\""), ("port", "9000")])
        .commit()?;

    println!("{}", doc);
    Ok(())
}
