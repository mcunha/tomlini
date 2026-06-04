# tomlini — SAX TOML/INI parser and editor
# Cross-platform justfile (no shell-dependent syntax)

# ---- Build ----------------------------------------------------------------

build:
    cargo build -p tomlini

release:
    cargo build -p tomlini --release

check:
    cargo check -p tomlini

# ---- Test ----------------------------------------------------------------

test:
    cargo test -p tomlini

test-ci: test

# ---- Coverage -------------------------------------------------------------

cov:
    cargo llvm-cov -p tomlini

cov-html:
    cargo llvm-cov -p tomlini --html --open

cov-ci: cov

# ---- CI -------------------------------------------------------------------

ci: check test cov-ci

# ---- Benchmarks -----------------------------------------------------------

bench-edit:
    cargo bench -p toml_benchmarks --bench 3-edit

bench-ini:
    cargo bench -p toml_benchmarks --bench 7-ini

bench-real:
    cargo bench -p toml_benchmarks --bench 8-realworld

bench-serde:
    cargo bench -p toml_benchmarks --bench 6-serde

# ---- Lint -----------------------------------------------------------------

clippy:
    cargo clippy -p tomlini -- -D warnings

fmt-check:
    cargo fmt -p tomlini -- --check

fmt:
    cargo fmt -p tomlini

# ---- no_std ---------------------------------------------------------------

core:
    cargo build -p tomlini --no-default-features

alloc:
    cargo build -p tomlini --no-default-features --features alloc

alloc-test:
    cargo test -p tomlini --no-default-features --features alloc

# ---- WASM -----------------------------------------------------------------

wasm:
    cargo build -p tomlini --target wasm32-unknown-unknown --no-default-features --features alloc


# ---- Mutation testing (nightly CI) -----------------------------------------

mutants:
    cargo mutants -p tomlini --timeout 30 --iterate

mutants-quick:
    cargo mutants -p tomlini --timeout 15 --iterate --re "build_index|commit|clean_key"


# ---- Fuzzing (nightly CI) ---------------------------------------------------

fuzz:
    # Run proptest unbounded — stops only on failure or Ctrl-C.
    # Uses PROPTEST_CASES=0 for "run until failure" mode.
    PROPTEST_CASES=0 cargo test --test proptest -- --nocapture

fuzz-editor:
    # Editor-specific fuzz targets only.
    PROPTEST_CASES=0 cargo test --test proptest editor_no_panic editor_survives_garbage -- --nocapture

fuzz-parser:
    # Parser fuzz targets only.
    PROPTEST_CASES=0 cargo test --test proptest span_integrity span_coverage idempotent -- --nocapture

fuzz-errors:
    PROPTEST_CASES=0 cargo test --test proptest_errors -- --nocapture
mutants-list:
    cargo mutants -p tomlini --list

mutants-shard N: SHARDS:
    cargo mutants -p tomlini --timeout 30 --iterate --shard {{N}}/{{SHARDS}}
# ---- Dev ------------------------------------------------------------------

watch:
    cargo watch -x "test -p tomlini"

# ----
