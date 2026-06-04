//! Property-based tests for `tomlini` parser and editor.

use proptest::prelude::*;
use tomlini::{SpanKind, parse, editor::Editor};
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

// ============================================================
// Editor fuzzer
// ============================================================

/// Random scalar value for edit ops.
fn edit_value() -> impl Strategy<Value = String> {
    prop_oneof![
        (any::<i64>()).prop_map(|i| i.to_string()),
        Just("true".to_string()),
        Just("false".to_string()),
        Just("\"hello\"".to_string()),
        Just("'literal'".to_string()),
        Just("3.14".to_string()),
    ]
}

fn edit_key() -> impl Strategy<Value = String> {
    "[a-zA-Z][a-zA-Z0-9_]{1,8}".prop_map(|s| s)
}


proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    /// The editor must never panic, regardless of operation sequence.
    /// Every successful commit must produce re-parseable output.
    #[test]
    fn editor_no_panic_and_roundtrip(
        doc_src in valid_toml(),
        ops in prop::collection::vec(
            (0usize..21, edit_key(), edit_key(), edit_value()),
            0..12,
        ),
    ) {
        let mut doc = match parse(&doc_src) {
            Ok(d) => d,
            Err(_) => return Ok(()),
        };

        for (op_idx, arg0, arg1, val) in ops {
            let mut e = Editor::new();
            match op_idx % 21 {
                0 => { e.set(&arg0, &val); }
                1 => { e.remove(&arg0); }
                2 => { e.insert(&arg0, &arg1, &val); }
                3 => { e.insert_section(&arg0); }
                4 => { e.rename_section(&arg0, &arg1); }
                5 => { e.clear_section(&arg0); }
                6 => { e.replace_section(&arg0, &[]); }
                7 => { e.rename_key(&arg0, &arg1); }
                8 => { e.move_key(&arg0, &arg1); }
                9 => { if arg0.contains('.') { e.promote_key(&arg0); } }
                10 => { e.move_key_create(&arg0, &arg1); }
                11 => { e.array_push(&arg0, &val); }
                12 => { e.array_set(&arg0, 0, &val); }
                13 => { e.array_insert(&arg0, 0, &val); }
                14 => { e.array_remove(&arg0, 0); }
                15 => { e.aot_push(&arg0, &[("x", "1")]); }
                16 => { e.aot_set(&arg0, 0, &arg1, &val); }
                17 => { e.inline_set(&arg0, &arg1, &val); }
                18 => { e.inline_insert(&arg0, &arg1, &val); }
                19 => { e.inline_remove(&arg0, &arg1); }
                20 => { e.reorder_root(&[]); }
                _ => {}
            }

            // Commit must not panic. Error is fine. Success → round-trip must parse.
            match e.commit(&mut doc) {
                Ok(()) => {
                    let output = doc.to_string();
                    let roundtrip = parse(&output);
                    prop_assert!(roundtrip.is_ok(),
                        "commit succeeded but output is unparseable.\n  source: {doc_src:?}\n  output: {output}");
                }
                Err(_) => { /* expected for random keys */ }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    /// Garbage values injected via set() must never cause a parse panic
    /// when the output is re-parsed.  The editor does not validate value
    /// syntax — broken TOML output is acceptable as long as the parser
    /// survives it.
    #[test]
    fn editor_survives_garbage_values(
        doc_src in valid_toml(),
        garbage in "\\PC{0,40}",
    ) {
        let mut doc = match parse(&doc_src) {
            Ok(d) => d,
            Err(_) => return Ok(()),
        };
        let keys = doc.keys();
        if keys.is_empty() { return Ok(()); }
        let target = &keys[0];

        let mut e = Editor::new();
        e.set(target, &garbage);

        match e.commit(&mut doc) {
            Ok(()) => {
                let output = doc.to_string();
                // Re-parsing must never panic, even if validation fails.
                let _ = parse(&output);
            }
            Err(_) => { /* fine */ }
        }
    }
}