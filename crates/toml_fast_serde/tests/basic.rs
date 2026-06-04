use serde::{Deserialize, Serialize};
use toml_fast_serde;

#[derive(Debug, PartialEq, Deserialize)]
struct Config {
    port: u16,
    host: String,
}

#[derive(Debug, PartialEq, Deserialize)]
struct Nested {
    server: Config,
}

#[test]
fn deserialize_flat() {
    let doc = toml_fast::parse("port = 8080\nhost = \"localhost\"\n").unwrap();
    let config: Config = toml_fast_serde::from_doc(&doc).unwrap();
    assert_eq!(config.port, 8080);
    assert_eq!(config.host, "localhost");
}

#[test]
fn deserialize_nested() {
    let input = "[server]\nport = 8080\nhost = \"localhost\"\n";
    let doc = toml_fast::parse(input).unwrap();
    let config: Nested = toml_fast_serde::from_doc(&doc).unwrap();
    assert_eq!(config.server.port, 8080);
    assert_eq!(config.server.host, "localhost");
}

#[test]
fn deserialize_bool() {
    #[derive(Debug, PartialEq, Deserialize)]
    struct C { enabled: bool }
    let doc = toml_fast::parse("enabled = true\n").unwrap();
    let c: C = toml_fast_serde::from_doc(&doc).unwrap();
    assert!(c.enabled);
}

#[test]
fn deserialize_float() {
    #[derive(Debug, PartialEq, Deserialize)]
    struct C { value: f64 }
    let doc = toml_fast::parse("value = 3.14\n").unwrap();
    let c: C = toml_fast_serde::from_doc(&doc).unwrap();
    assert!((c.value - 3.14).abs() < 0.001);
}

#[test]
fn deserialize_hex_int() {
    #[derive(Debug, PartialEq, Deserialize)]
    struct C { weight: u64 }
    let doc = toml_fast::parse("weight = 0x64\n").unwrap();
    let c: C = toml_fast_serde::from_doc(&doc).unwrap();
    assert_eq!(c.weight, 100);
}

#[test]
fn deserialize_array() {
    #[derive(Debug, PartialEq, Deserialize)]
    struct C { values: Vec<i64> }
    let doc = toml_fast::parse("values = [1, 2, 3]\n").unwrap();
    let c: C = toml_fast_serde::from_doc(&doc).unwrap();
    assert_eq!(c.values, vec![1, 2, 3]);
}
#[test]
fn deserialize_datetime() {
    #[derive(Debug, PartialEq, Deserialize)]
    struct C { timestamp: toml_datetime::Datetime }
    let doc = toml_fast::parse("timestamp = 1979-05-27T07:32:00Z\n").unwrap();
    let c: C = toml_fast_serde::from_doc(&doc).unwrap();
    assert!(c.timestamp.to_string().contains("1979-05-27"));
}

#[test]
fn deserialize_datetime_date_only() {
    #[derive(Debug, PartialEq, Deserialize)]
    struct C { dt: toml_datetime::Datetime }
    let doc = toml_fast::parse("dt = 1979-05-27\n").unwrap();
    let c: C = toml_fast_serde::from_doc(&doc).unwrap();
    assert!(c.dt.to_string().contains("1979-05-27"));
}


#[test]
fn serialize_roundtrip() {
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct C { port: u16, host: String }
    let input = C { port: 8080, host: "localhost".into() };
    let doc = toml_fast_serde::to_doc(&input).unwrap();
    let output: C = toml_fast_serde::from_doc(&doc).unwrap();
    assert_eq!(input.port, output.port);
    assert_eq!(input.host, output.host);
}

#[test]
fn serialize_to_string() {
    #[derive(Debug, Serialize)]
    struct C { port: u16, host: String }
    let c = C { port: 8080, host: "localhost".into() };
    let s = toml_fast_serde::to_string(&c).unwrap();
    assert!(s.contains("port = 8080"));
    assert!(s.contains("host = \"localhost\""));
}
#[test]
fn deserialize_inline_table() {
    #[derive(Debug, PartialEq, Deserialize)]
    struct Point { x: i64, y: i64 }
    #[derive(Debug, PartialEq, Deserialize)]
    struct C { location: Point }
    let doc = toml_fast::parse("location = { x = 1, y = 2 }\n").unwrap();
    let c: C = toml_fast_serde::from_doc(&doc).unwrap();
    assert_eq!(c.location.x, 1);
    assert_eq!(c.location.y, 2);
}

#[test]
fn deserialize_aot() {
    #[derive(Debug, PartialEq, Deserialize)]
    struct Bin { name: String, path: String }
    #[derive(Debug, PartialEq, Deserialize)]
    struct Config { bin: Vec<Bin> }
    let input = "[[bin]]\nname = \"alpha\"\npath = \"src/a.rs\"\n\n[[bin]]\nname = \"beta\"\npath = \"src/b.rs\"\n";
    let doc = toml_fast::parse(input).unwrap();
    let c: Config = toml_fast_serde::from_doc(&doc).unwrap();
    assert_eq!(c.bin.len(), 2);
    assert_eq!(c.bin[0].name, "alpha");
    assert_eq!(c.bin[1].name, "beta");
}

#[test]
fn deserialize_flatten() {
    #[derive(Debug, PartialEq, Deserialize)]
    struct Common { name: String, version: String }
    #[derive(Debug, PartialEq, Deserialize)]
    struct Config {
        #[serde(flatten)]
        common: Common,
        port: u16,
    }
    let input = "name = \"app\"\nversion = \"1.0\"\nport = 8080\n";
    let doc = toml_fast::parse(input).unwrap();
    let c: Config = toml_fast_serde::from_doc(&doc).unwrap();
    assert_eq!(c.common.name, "app");
    assert_eq!(c.port, 8080);
}
