//! INI format benchmarks for `tomlini`.
//!
//! Covers common INI patterns: classic bare values, `;` comments,
//! Windows-style CRLF, Apache-style configs, and section-heavy files.

#![allow(elided_lifetimes_in_paths)]

use std::sync::LazyLock;

// ---- INI test documents ----

/// Classic INI: bare keys, `=` separator, no quotes on values
const CLASSIC_INI: &str = "\
; Application settings
[server]
host = localhost
port = 8080
debug = true

[database]
host = db.example.com
name = myapp
pool_size = 10
";

/// Windows-style INI: CRLF line endings, spaces around `=`
const WINDOWS_INI: &str = "\
; System configuration\r\n\
[network]\r\n\
  adapter = eth0 \r\n\
  ip = 10.0.0.1 \r\n\
  gateway = 10.0.0.254\r\n\
\r\n\
[users]\r\n\
  max_connections = 500  \r\n\
  timeout = 30\r\n\
";

/// Apache-style config: many key-value pairs, mixed comment styles
const APACHE_STYLE: &str = "\
# Apache-like configuration
ServerRoot /etc/httpd
Listen 80
Listen 443

# Module loading
LoadModule auth_module modules/mod_auth.so
LoadModule ssl_module modules/mod_ssl.so

# Virtual hosts
<VirtualHost *:80>
    ServerName www.example.com
    DocumentRoot /var/www/html
    ErrorLog /var/log/httpd/error.log
    CustomLog /var/log/httpd/access.log combined
</VirtualHost>
";

/// Docker daemon.json converted to INI style
const DOCKER_INI: &str = "\
# Docker daemon configuration
[registry-mirrors]
mirror1 = https://mirror.gcr.io
mirror2 = https://registry-1.docker.io

[log]
driver = json-file
max-size = 10m
max-file = 3

[storage]
driver = overlay2

[dns]
servers = 8.8.8.8,8.8.4.4
search = example.com
";

/// Large INI: many sections with many keys
static LARGE_INI: LazyLock<String> = LazyLock::new(|| {
    let mut s = String::with_capacity(40000);
    for i in 0..100 {
        s.push_str(&format!("; Section {i}\n[{i}]\n"));
        for j in 0..50 {
            s.push_str(&format!("key_{j} = value_{i}_{j}\n"));
        }
        s.push('\n');
    }
    s
});

// ---- Benchmarks ----

#[divan::bench]
fn parse_classic_ini() -> tomlini::FlatDoc {
    tomlini::parse(CLASSIC_INI).unwrap()
}

#[divan::bench]
fn parse_windows_ini() -> tomlini::FlatDoc {
    tomlini::parse(WINDOWS_INI).unwrap()
}

#[divan::bench]
fn parse_apache_style() -> tomlini::FlatDoc {
    tomlini::parse(APACHE_STYLE).unwrap()
}

#[divan::bench]
fn parse_docker_ini() -> tomlini::FlatDoc {
    tomlini::parse(DOCKER_INI).unwrap()
}

#[divan::bench]
fn parse_large_ini() -> tomlini::FlatDoc {
    tomlini::parse(&LARGE_INI).unwrap()
}

#[divan::bench]
fn edit_ini_set_value() -> String {
    let mut doc = tomlini::parse(CLASSIC_INI).unwrap();
    doc.edit()
        .set("server.port", "9090")
        .set("server.host", "0.0.0.0")
        .commit()
        .unwrap();
    doc.to_string()
}

#[divan::bench]
fn edit_ini_insert_key() -> String {
    let mut doc = tomlini::parse(CLASSIC_INI).unwrap();
    doc.edit()
        .insert("server", "max_connections", "100")
        .with_above_comment("Max concurrent connections")
        .commit()
        .unwrap();
    doc.to_string()
}

#[divan::bench]
fn edit_large_ini_set_value() -> String {
    let mut doc = tomlini::parse(&LARGE_INI).unwrap();
    doc.edit()
        .set("50.key_25", "modified")
        .commit()
        .unwrap();
    doc.to_string()
}

#[divan::bench]
fn validate_classic_ini_relaxed() -> usize {
    let mut doc = tomlini::parse(CLASSIC_INI).unwrap();
    doc.validate(tomlini::ValidationMode::Relaxed).len()
}

#[divan::bench]
fn validate_classic_ini_strict() -> usize {
    let mut doc = tomlini::parse(CLASSIC_INI).unwrap();
    doc.validate(tomlini::ValidationMode::Strict).len()
}

#[divan::bench]
fn roundtrip_classic_ini() -> String {
    let mut doc = tomlini::parse(CLASSIC_INI).unwrap();
    doc.to_string()
}

fn main() { divan::main(); }
