//! Coverage-saturating tests for `tomlini`.

use tomlini::{SpanKind, parse};

fn assert_kinds(input: &str, expected: &[SpanKind]) {
    let doc = parse(input).unwrap();
    let kinds: Vec<SpanKind> = doc.spans.iter().map(|s| s.kind).collect();
    assert_eq!(kinds, expected, "kind mismatch for {input:?}\ngot: {kinds:?}");
}

#[test] fn ws_spaces()     { assert_kinds("   ", &[SpanKind::Whitespace]); }
#[test] fn ws_tabs()       { assert_kinds("\t\t", &[SpanKind::Whitespace]); }
#[test] fn ws_mixed()      { assert_kinds(" \t \t", &[SpanKind::Whitespace]); }
#[test] fn nl_lf()         { assert_kinds("\n", &[SpanKind::Newline]); }
#[test] fn nl_crlf()       { assert_kinds("\r\n", &[SpanKind::Newline]); }
#[test] fn nl_cr()         { assert_kinds("\r", &[SpanKind::Newline]); }
#[test] fn nl_multi()      { assert_kinds("\n\n\n", &[S(NEWLINE); 3]); }
#[test] fn comment_basic() { assert_kinds("# hi", &[SpanKind::Comment]); }
#[test] fn comment_empty() { assert_kinds("#", &[SpanKind::Comment]); }
#[test] fn comment_nl()    { assert_kinds("# hi\n", &[SpanKind::Comment, SpanKind::Newline]); }
#[test] fn comment_at_eof() { assert!(parse("#").unwrap().spans[0].kind == SpanKind::Comment); }
#[test] fn bare_simple()   { assert_kinds("hi", &[SpanKind::BareKey]); }
#[test] fn bare_dash()     { assert_kinds("my-key", &[SpanKind::BareKey]); }
#[test] fn bare_underscore(){ assert_kinds("my_key", &[SpanKind::BareKey]); }
#[test] fn bare_digit()    { assert_kinds("key123", &[SpanKind::BareKey]); }
#[test] fn bare_lead_us()  { assert_kinds("_key", &[SpanKind::BareKey]); }
#[test] fn bare_not_digit() { assert_kinds("123", &[SpanKind::Integer]); }
#[test] fn basic_empty()   { assert_kinds("\"\"", &[SpanKind::BasicString]); }
#[test] fn basic_simple()  { assert_kinds("\"hi\"", &[SpanKind::BasicString]); }
#[test] fn basic_escapes() { assert_kinds(r#""\n\t""#, &[SpanKind::BasicString]); }
#[test] fn basic_unicode() { assert_kinds(r#""\u0041""#, &[SpanKind::BasicString]); }
#[test] fn basic_unterr()  { assert!(parse("\"unterminated").is_err()); }
#[test] fn lit_empty()     { assert_kinds("''", &[SpanKind::LiteralString]); }
#[test] fn lit_simple()    { assert_kinds("'hi'", &[SpanKind::LiteralString]); }
#[test] fn lit_bslash()    { assert_kinds(r"'c:\path'", &[SpanKind::LiteralString]); }
#[test] fn lit_unterr()    { assert!(parse("'unterminated").is_err()); }
#[test] fn mlb_empty()     { assert_kinds("\"\"\"\"\"\"", &[SpanKind::MlBasicString]); }
#[test] fn mlb_simple()    { assert_kinds("\"\"\"\nhi\n\"\"\"", &[SpanKind::MlBasicString]); }
#[test] fn mlb_escapes()   { assert_kinds("\"\"\"\n\\n\n\"\"\"", &[SpanKind::MlBasicString]); }
#[test] fn mlb_unterr()    { assert!(parse("\"\"\"\nunterminated").is_err()); }
#[test] fn mll_empty()     { assert_kinds("''''''", &[SpanKind::MlLiteralString]); }
#[test] fn mll_simple()    { assert_kinds("'''\nhi\n'''", &[SpanKind::MlLiteralString]); }
#[test] fn mll_unterr()    { assert!(parse("'''\nunterminated").is_err()); }
#[test] fn int_dec()       { assert_kinds("42", &[SpanKind::Integer]); }
#[test] fn int_neg()       { assert_kinds("-42", &[SpanKind::Integer]); }
#[test] fn int_pos()       { assert_kinds("+42", &[SpanKind::Integer]); }
#[test] fn int_hex()       { assert_kinds("0x2A", &[SpanKind::Integer]); }
#[test] fn int_hex_upper() { assert_kinds("0xDEAD", &[SpanKind::Integer]); }
#[test] fn int_oct()       { assert_kinds("0o755", &[SpanKind::Integer]); }
#[test] fn int_bin()       { assert_kinds("0b1010", &[SpanKind::Integer]); }
#[test] fn int_und()       { assert_kinds("1_000", &[SpanKind::Integer]); }
#[test] fn float_inf()      { assert_kinds("inf", &[SpanKind::Float]); }
#[test] fn float_nan()      { assert_kinds("nan", &[SpanKind::Float]); }
#[test] fn float_neg_inf()  { assert_kinds("-inf", &[SpanKind::Float]); }
#[test] fn float_neg_nan()  { assert_kinds("-nan", &[SpanKind::Float]); }
#[test] fn float_dec()     { assert_kinds("3.14", &[SpanKind::Float]); }
#[test] fn float_exp()     { assert_kinds("1.5e10", &[SpanKind::Float]); }
#[test] fn float_neg_exp() { assert_kinds("1.5e-5", &[SpanKind::Float]); }
#[test] fn float_e()       { assert_kinds("1E6", &[SpanKind::Float]); }
#[test] fn float_plus_inf(){ assert_kinds("+inf", &[SpanKind::Float]); }
#[test] fn float_minus_inf(){ assert_kinds("-inf", &[SpanKind::Float]); }
#[test] fn bool_true()     { assert_kinds("true", &[SpanKind::Boolean]); }
#[test] fn bool_false()    { assert_kinds("false", &[SpanKind::Boolean]); }
#[test] fn dt_odt()        { assert_kinds("1979-05-27T07:32:00Z", &[SpanKind::Datetime]); }
#[test] fn dt_offset()     { assert_kinds("1979-05-27T00:32:00-07:00", &[SpanKind::Datetime]); }
#[test] fn dt_ldt()        { assert_kinds("1979-05-27T07:32:00", &[SpanKind::Datetime]); }
#[test] fn dt_date()       { assert_kinds("1979-05-27", &[SpanKind::Datetime]); }
#[test] fn dt_time()       { assert_kinds("07:32:00", &[SpanKind::Datetime]); }
#[test] fn dt_space()      { assert_kinds("1979-05-27 07:32:00Z", &[SpanKind::Datetime]); }
#[test] fn arr_empty()     { assert_kinds("[]", &[SpanKind::ArrayOpen, SpanKind::ArrayClose]); }
#[test] fn arr_single()    { assert_kinds("[1]", &[S(ARRAY_OPEN), S(INTEGER), S(ARRAY_CLOSE)]); }
#[test] fn arr_multi()     { assert_kinds("[1,2]", &[S(ARRAY_OPEN),S(INTEGER),S(COMMA),S(INTEGER),S(ARRAY_CLOSE)]); }
#[test] fn arr_nested()    {
    // Known limitation: [[1]] parses as ArrayTableOpen + 1 + ArrayTableClose
    assert_kinds("[[1]]", &[SpanKind::ArrayTableOpen, SpanKind::Integer, SpanKind::ArrayTableClose]);
}
#[test] fn inline_empty()  { assert_kinds("{}", &[SpanKind::InlineTableOpen, SpanKind::InlineTableClose]); }
#[test] fn inline_single() { assert_kinds("{a=1}", &[S(INLINE_TABLE_OPEN),S(BARE_KEY),S(EQUALS),S(INTEGER),S(INLINE_TABLE_CLOSE)]); }
#[test] fn table_std()     { assert_kinds("[t]", &[S(ARRAY_OPEN),S(BARE_KEY),S(ARRAY_CLOSE)]); }
#[test] fn table_aot()     { assert_kinds("[[e]]", &[SpanKind::ArrayTableOpen, SpanKind::BareKey, SpanKind::ArrayTableClose]); }
#[test] fn kv()            { assert_kinds("k=1", &[S(BARE_KEY),S(EQUALS),S(INTEGER)]); }
#[test] fn dotted()        { assert_kinds("a.b=1", &[S(BARE_KEY),S(DOT),S(BARE_KEY),S(EQUALS),S(INTEGER)]); }
#[test] fn dot_alone()     { assert_kinds(".", &[SpanKind::Dot]); }
#[test] fn eq_alone()      { assert_kinds("=", &[SpanKind::Equals]); }
#[test] fn comma_alone()   { assert_kinds(",", &[SpanKind::Comma]); }
#[test] fn empty()         { assert!(parse("").unwrap().spans.is_empty()); }
#[test] fn roundtrip()     { let s="# cmt\nk=1\n"; assert_eq!(parse(s).unwrap().source, s); }
#[test] fn span_bounds_ok(){ let d=parse("a=1\n").unwrap(); let mn=d.spans.iter().map(|s|s.start).min().unwrap(); let mx=d.spans.iter().map(|s|s.end).max().unwrap(); assert_eq!(mn,0); assert_eq!(mx as usize,d.source.len()); for s in &d.spans{ assert!(s.start<s.end); assert!(s.end as usize<=d.source.len()); } }

// helpers
const S: fn(SpanKind) -> SpanKind = |k| k;
const NEWLINE: SpanKind = SpanKind::Newline;
const ARRAY_OPEN: SpanKind = SpanKind::ArrayOpen;
const ARRAY_CLOSE: SpanKind = SpanKind::ArrayClose;
const INTEGER: SpanKind = SpanKind::Integer;
const COMMA: SpanKind = SpanKind::Comma;
const INLINE_TABLE_OPEN: SpanKind = SpanKind::InlineTableOpen;
const INLINE_TABLE_CLOSE: SpanKind = SpanKind::InlineTableClose;
const BARE_KEY: SpanKind = SpanKind::BareKey;
const EQUALS: SpanKind = SpanKind::Equals;
const DOT: SpanKind = SpanKind::Dot;
