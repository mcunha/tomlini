//! Serde benchmarks for `toml_fast_serde`.

#![allow(elided_lifetimes_in_paths)]

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Pkg { name: String, version: String, edition: String }

#[derive(Debug, Deserialize)]
struct Config { package: Pkg }

const T: &str = "[package]\nname = \"bar\"\nversion = \"0.1.0\"\nedition = \"2018\"\n\n[dependencies]\n";

#[divan::bench]
fn fast_serde() -> Config {
    toml_fast_serde::from_str(T).unwrap().1
}

fn main() { divan::main(); }
