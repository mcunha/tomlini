# toml_fast_serde

Serde integration for `toml_fast`. Deserializes TOML directly from the flat span index — no DOM construction.

## Features

- **Deserialize** any `#[derive(Deserialize)]` struct from TOML
- **Serialize** to TOML string or editable `FlatDoc`
- **Array-of-tables** (`[[bin]]` → `Vec<T>`)
- **Flatten** (`#[serde(flatten)]`)
- **Datetime** support via `toml_datetime::Datetime`
- All TOML value types: strings, integers (hex/oct/bin), floats, booleans, datetimes

## Quick start

```rust
use serde::Deserialize;

#[derive(Deserialize)]
struct Config {
    server: Server,
}

#[derive(Deserialize)]
struct Server {
    port: u16,
    host: String,
}

let doc = toml_fast::parse("[server]\nport = 8080\nhost = \"localhost\"\n")?;
let config: Config = toml_fast_serde::from_doc(&doc)?;
```

## Serialize back to editable document

```rust
let doc = toml_fast_serde::to_doc(&config)?;
doc.edit().set("server.port", "9090").commit()?;
```

## License

MIT OR Apache-2.0
