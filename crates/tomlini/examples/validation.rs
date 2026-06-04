//! # Validation — lenient, relaxed, and strict modes.
//!
//! Validate a document after edits to catch structural TOML errors.
//!
//! Run:  cargo run --example validation

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = r#"
[server]
port = 8080
host = "localhost"

[server]    # duplicate table!
host = "0.0.0.0"
"#;

    let mut doc = tomlini::parse(input)?;

    // Strict catches duplicate tables and keys
    let errors = doc.validate(tomlini::ValidationMode::Strict);
    for e in &errors {
        println!("✗ {}", e.msg);
    }
    println!("Strict: {} error(s)", errors.len());

    // Lenient relaxes key-name rules but still catches structural issues
    let errors = doc.validate(tomlini::ValidationMode::Lenient);
    println!("Lenient: {} error(s)", errors.len());

    Ok(())
}
