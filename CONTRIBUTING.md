# Contributing to tomlini

## Setup

```bash
git clone https://github.com/mcunha/tomlini
cd tomlini
just setup-git-hooks   # installs pre-commit (zizmor, fmt, clippy)
```

## Dev workflow

```bash
just build             # cargo build -p tomlini
just test              # cargo test -p tomlini (298 tests)
just check             # cargo check -p tomlini
just clippy            # cargo clippy -p tomlini -- -D warnings
just fmt               # cargo fmt -p tomlini
just lint              # clippy + fmt-check + zizmor
just cov               # cargo llvm-cov -p tomlini (coverage report)
just cov-html          # open coverage report in browser
just bench-edit        # run edit benchmarks
just bench-compare     # tomlini vs toml_edit comparison
```

## Before opening a PR

- [ ] `just test` passes
- [ ] `just lint` passes
- [ ] `cargo doc --no-deps -p tomlini` produces no warnings
- [ ] `CHANGELOG.md` is updated if the change is notable

## Commit conventions

tomlini uses [Conventional Commits](https://www.conventionalcommits.org/).
`release-plz` reads commit messages to determine the semver bump:

- `fix:` → patch
- `feat:` → minor
- `feat!:` or `fix!: ` → major

## Architecture

- `lib.rs` — SAX parser, FlatDoc, span index, query API, validation
- `editor.rs` — Editor, EditorHandle, OpKind enum (22 variants), commit handler
- `edit.rs` — index builder, EditError
- `validate.rs` — three-mode validation

The editor works by accumulating `Op` structs, resolving them against the index
in `commit()`, then performing byte-range splices on the source string.
Comments, whitespace, and formatting are preserved by construction — they're
never parsed into a tree, only measured as spans.
