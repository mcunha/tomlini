//! Property-based tests for `toml_fast`.

use proptest::prelude::*;
use toml_fast::{SpanKind, parse};

// ============================================================
// TOML document generator — rejection-free
// ============================================================

fn bare_key() -> impl Strategy<Value = String> {
    "[a-zA-Z][a-zA-Z0-9_]{0,15}".prop_map(|s| s)
}

fn dotted_key() -> impl Strategy<Value = String> {
    prop::collection::vec(bare_key(), 1..4).prop_map(|ks| ks.join("."))
}

fn key() -> impl Strategy<Value = String> {
    prop_oneof![
        bare_key(),
        dotted_key(),
        Just("\"quoted key\"".to_string()),
        Just("'literal key'".to_string()),
    ]
}

fn float() -> impl Strategy<Value = String> {
    prop_oneof![
        (any::<f64>()).prop_map(|f| {
            if f.is_nan() { "nan".into() }
            else if f.is_infinite() && f > 0.0 { "+inf".into() }
            else if f.is_infinite() { "-inf".into() }
            else { format!("{f}") }
        }),
        Just("3.14".to_string()),
        Just("1.5e10".to_string()),
        Just("-0.0".to_string()),
    ]
}

fn datetime() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("1979-05-27T07:32:00Z".to_string()),
        Just("1979-05-27T00:32:00-07:00".to_string()),
        Just("1979-05-27T07:32:00".to_string()),
        Just("1979-05-27".to_string()),
        Just("07:32:00".to_string()),
    ]
}

fn value() -> impl Strategy<Value = String> {
    prop_oneof![
        (any::<i64>()).prop_map(|i| i.to_string()),
        Just("true".to_string()),
        Just("false".to_string()),
        float(),
        datetime(),
        Just("\"hello\"".to_string()),
        Just("\"world\"".to_string()),
        Just("'literal'".to_string()),
        array(),
        inline_table(),
    ]
}

fn array() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            (any::<i64>()).prop_map(|i| i.to_string()),
            Just("true".to_string()),
            Just("false".to_string()),
            Just("\"a\"".to_string()),
        ],
        0..6,
    ).prop_map(|vals| format!("[{}]", vals.join(", ")))
}

fn inline_table() -> impl Strategy<Value = String> {
    prop::collection::vec(
        (bare_key(), prop_oneof![
            (any::<i64>()).prop_map(|i| i.to_string()),
            Just("true".to_string()),
            Just("false".to_string()),
            Just("\"v\"".to_string()),
        ]),
        0..4,
    ).prop_map(|pairs| {
        let inner: Vec<_> = pairs.into_iter().map(|(k, v)| format!("{k} = {v}")).collect();
        format!("{{{}}}", inner.join(", "))
    })
}

fn kv_pair() -> impl Strategy<Value = String> {
    (key(), value()).prop_map(|(k, v)| format!("{k} = {v}\n"))
}

fn table_section() -> impl Strategy<Value = String> {
    let header = key().prop_map(|k| format!("[{k}]\n"));
    let body = prop::collection::vec(kv_pair(), 0..5);
    (header, body).prop_map(|(h, pairs)| {
        let mut s = h;
        for p in &pairs { s.push_str(p); }
        s.push('\n');
        s
    })
}

fn aot_section() -> impl Strategy<Value = String> {
    let header = key().prop_map(|k| format!("[[{k}]]\n"));
    let body = prop::collection::vec(kv_pair(), 1..3);
    (header, body).prop_map(|(h, pairs)| {
        let mut s = h;
        for p in &pairs { s.push_str(p); }
        s.push('\n');
        s
    })
}

fn comment_line() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("# a comment\n".to_string()),
        Just("#\n".to_string()),
    ]
}

fn blank_line() -> impl Strategy<Value = String> {
    Just("\n".to_string())
}

fn toml_document() -> impl Strategy<Value = String> {
    let element = prop_oneof![
        kv_pair(),
        table_section(),
        aot_section(),
        comment_line(),
        blank_line(),
    ];
    prop::collection::vec(element, 0..30).prop_map(|parts| parts.concat())
}

fn valid_toml() -> impl Strategy<Value = String> {
    toml_document()
}

// ============================================================
// Invariants — skip inputs the parser rejects
// ============================================================

proptest! {
    #[test]
    fn span_integrity(s in valid_toml()) {
        let Ok(doc) = parse(&s) else { return Ok(()); };
        for span in &doc.spans {
            prop_assert!(span.start < span.end);
            prop_assert!(span.end as usize <= s.len());
        }
    }

    #[test]
    fn span_coverage(s in valid_toml()) {
        let Ok(doc) = parse(&s) else { return Ok(()); };
        let mut covered = vec![false; s.len()];
        for span in &doc.spans {
            for i in span.start as usize..span.end as usize { covered[i] = true; }
        }
        for i in 0..s.len() {
            if !covered[i] {
                let lo = i.saturating_sub(10);
                let hi = (i + 10).min(s.len());
                prop_assert!(covered[i], "byte {i} uncovered near {:?}", &s[lo..hi]);
            }
        }
    }

    #[test]
    fn spans_non_overlapping(s in valid_toml()) {
        let Ok(doc) = parse(&s) else { return Ok(()); };
        for w in doc.spans.windows(2) {
            prop_assert!(w[0].end <= w[1].start);
        }
    }

    #[test]
    fn source_preserved(s in valid_toml()) {
        let Ok(doc) = parse(&s) else { return Ok(()); };
        prop_assert_eq!(&doc.source, &s);
    }

    #[test]
    fn comments_no_newlines(s in valid_toml()) {
        let Ok(doc) = parse(&s) else { return Ok(()); };
        for span in &doc.spans {
            if span.kind == SpanKind::Comment {
                let text = &doc.source[span.start as usize..span.end as usize];
                prop_assert!(!text.contains('\n'));
                prop_assert!(!text.contains('\r'));
            }
        }
    }

    #[test]
    fn idempotent(s in valid_toml()) {
        let Ok(doc1) = parse(&s) else { return Ok(()); };
        let Ok(doc2) = parse(&doc1.source) else { return Ok(()); };
        prop_assert_eq!(doc1.spans.len(), doc2.spans.len());
        for (a, b) in doc1.spans.iter().zip(doc2.spans.iter()) {
            prop_assert_eq!(a.kind, b.kind);
            prop_assert_eq!(a.start, b.start);
            prop_assert_eq!(a.end, b.end);
        }
    }
}
