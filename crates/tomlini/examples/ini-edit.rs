//! # INI file editing — semicolon comments and values.
//!
//! tomlini parses INI `;` comments natively and treats `key = value`
//! pairs exactly like TOML scalars.
//!
//! Run:  cargo run --example ini-edit

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = "\
; Server configuration
[server]
host = \"0.0.0.0\"
port = 8080

[database]
url = \"localhost:5432\"
pool_size = 10
tls = false
";

    let mut doc = tomlini::parse(input)?;

    doc.edit()
        .set("server.port", "9090")
        .set("database.pool_size", "20")
        .set("database.tls", "true")
        .commit()?;

    println!("{}", doc);
    Ok(())
}
