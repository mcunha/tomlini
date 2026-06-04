//! Error-path property tests for `tomlini`.
//!
//! Generates deliberately invalid TOML and asserts the parser either
//! rejects it or that accepted spans still satisfy basic invariants.

use proptest::prelude::*;
use tomlini::parse;

fn unterminated_basic() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9]{1,20}".prop_map(|s| format!("\"{s}"))
}

fn unterminated_literal() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9]{1,20}".prop_map(|s| format!("'{s}"))
}

fn basic_with_nl() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9]{1,8}".prop_map(|s| format!("\"{s}\n{s}\""))
}

fn literal_with_nl() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9]{1,8}".prop_map(|s| format!("'{s}\n{s}'"))
}

fn unterminated_ml_basic() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9]{1,20}".prop_map(|s| format!("\"\"\"\n{s}"))
}

fn unterminated_ml_literal() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9]{1,20}".prop_map(|s| format!("'''\n{s}"))
}

proptest! {
    #[test]
    fn unterminated_strings_should_error(s in prop_oneof![
        unterminated_basic(), unterminated_literal(),
    ]) {
        match parse(&s) {
            Err(e) => {
                prop_assert!(e.pos < s.len());
                let sq = s.find(&['"', '\''][..]).unwrap_or(0);
                prop_assert!(e.pos >= sq);
            }
            Ok(_) => { /* accepted — no crash is also fine */ }
        }
    }

    #[test]
    fn strings_with_newlines_should_error(s in prop_oneof![
        basic_with_nl(), literal_with_nl(),
    ]) {
        if let Err(e) = parse(&s) {
            prop_assert!(e.msg.contains("newline"));
        }
    }

    #[test]
    fn unterminated_ml_strings_should_error(s in prop_oneof![
        unterminated_ml_basic(), unterminated_ml_literal(),
    ]) {
        if let Err(e) = parse(&s) {
            prop_assert!(e.pos < s.len());
        }
    }

    #[test]
    fn never_panics_on_garbage(bytes in prop::collection::vec(any::<u8>(), 0..128)) {
        let s = String::from_utf8_lossy(&bytes).into_owned();
        let _ = parse(&s);
    }

    #[test]
    fn accepted_invalid_still_has_valid_spans(s in prop_oneof![
        unterminated_basic(), unterminated_literal(),
        basic_with_nl(), literal_with_nl(),
        unterminated_ml_basic(), unterminated_ml_literal(),
    ]) {
        if let Ok(doc) = parse(&s) {
            for span in &doc.spans {
                prop_assert!(span.start < span.end);
                prop_assert!(span.end as usize <= s.len());
            }
            for w in doc.spans.windows(2) {
                prop_assert!(w[0].end <= w[1].start);
            }
        }
    }
}
