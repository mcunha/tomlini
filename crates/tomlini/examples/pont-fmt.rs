//! # Pont fmt — self-healing and section reordering.
//!
//! When scalars appear after a `[table]` header, TOML absorbs them into
//! that table per the spec.  `promote_key` extracts them back to root,
//! then `reorder_root` places scalars before tables for cleaner output.
//!
//! Run:  cargo run --example pont-fmt

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Input: scalars inside [meta] per valid TOML placement
    let input = "\
# My project
[meta]
name = \"my-project\"
base = \"my-base\"
kind = \"leaf\"
";

    let mut doc = tomlini::parse(input)?;

    // Step 1: promote scalars from [meta] to document root
    doc.edit()
        .promote_key("meta.base")
        .promote_key("meta.kind")
        .commit()?;

    // Step 2: reorder — scalars before tables
    doc.edit()
        .reorder_root(&["base", "kind", "meta"])
        .commit()?;

    println!("{}", doc);
    // Now `base` and `kind` are root-level scalars before `[meta]`,
    // comments and formatting preserved.

    Ok(())
}
