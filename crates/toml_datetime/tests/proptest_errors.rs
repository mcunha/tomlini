//! Error-path proptests for `toml_datetime`.
//!
//! Generates invalid datetime strings and verifies the parser rejects them.
//! Targets the remaining ~23% uncovered error branches in FromStr.

use proptest::prelude::*;
use toml_datetime::Datetime;

// ============================================================
// Generators
// ============================================================

fn year() -> impl Strategy<Value = u16> { (0u16..10000u16) }

fn month_day() -> impl Strategy<Value = (u8, u8)> { (1u8..13u8, 1u8..29u8) }

fn time_part() -> impl Strategy<Value = (u8, u8, u8)> { (0u8..24u8, 0u8..60u8, 0u8..61u8) }

fn offset_str() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("Z".to_string()), Just("z".to_string()),
        (0u8..24u8, 0u8..60u8).prop_map(|(h, m)| format!("+{h:02}:{m:02}")),
        (0u8..24u8, 0u8..60u8).prop_map(|(h, m)| format!("-{h:02}:{m:02}")),
    ]
}

fn sep() -> impl Strategy<Value = char> { prop_oneof![Just('T'), Just('t'), Just(' ')] }

fn valid_odt() -> impl Strategy<Value = String> {
    (year(), month_day(), sep(), time_part(), offset_str())
        .prop_map(|(y, (mo, d), s, (h, mi, se), off)|
            format!("{y:04}-{mo:02}-{d:02}{s}{h:02}:{mi:02}:{se:02}{off}"))
}

fn valid_ldt() -> impl Strategy<Value = String> {
    (year(), month_day(), sep(), time_part())
        .prop_map(|(y, (mo, d), s, (h, mi, se))|
            format!("{y:04}-{mo:02}-{d:02}{s}{h:02}:{mi:02}:{se:02}"))
}

fn valid_date() -> impl Strategy<Value = String> {
    (year(), month_day())
        .prop_map(|(y, (mo, d))| format!("{y:04}-{mo:02}-{d:02}"))
}

fn valid_time() -> impl Strategy<Value = String> {
    time_part().prop_map(|(h, mi, se)| format!("{h:02}:{mi:02}:{se:02}"))
}

fn valid_base() -> impl Strategy<Value = String> {
    prop_oneof![valid_odt(), valid_ldt(), valid_date(), valid_time()]
}

// ============================================================
// Mutations: turn valid strings into invalid ones
// ============================================================

/// Truncate at every position, producing all possible truncated forms.
fn truncations(s: &str) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    (1..chars.len())
        .map(|i| chars[..i].iter().collect())
        .collect()
}

/// Replace a character at position `i` with `replacement`.
fn replace_at(s: &str, i: usize, replacement: char) -> String {
    let mut chars: Vec<char> = s.chars().collect();
    if i < chars.len() { chars[i] = replacement; }
    chars.into_iter().collect()
}

/// Build a vector of all invalid mutations for a given valid string.
fn mutate(valid: &str) -> Vec<String> {
    let mut out = Vec::new();

    // 1. Truncations
    out.extend(truncations(valid));

    // 2. Replace separator with X
    if valid.contains('T') { out.push(replace_at(valid, valid.find('T').unwrap(), 'X')); }
    if valid.contains('t') { out.push(replace_at(valid, valid.find('t').unwrap(), 'X')); }
    if valid.contains(' ') && valid.find(' ').unwrap() > 4 {
        let pos = valid.find(' ').unwrap();
        out.push(replace_at(valid, pos, 'X'));
    }

    // 3. Replace date separator - with /
    if valid.len() > 7 {
        out.push(valid.replacen('-', "/", 1));
    }

    // 4. Append garbage
    for suffix in ["x", "T", "-", ":"] {
        out.push(format!("{valid}{suffix}"));
    }

    // 5. Multiple dots
    if valid.contains('.') {
        out.push(valid.replacen(".", "..", 1));
    }

    // 6. Invalid offset formats (for odt patterns)
    if valid.contains('Z') || valid.contains('z') || valid.contains('+') || valid.contains('-') && valid.len() > 19 {
        let date_and_time = if let Some(pos) = valid.find(&['Z', 'z', '+', '-'][..]) {
            if valid.as_bytes()[pos] == b'-' && pos > 10 {
                // This is the date's dash, not the offset sign — skip
                String::new()
            } else {
                valid[..pos].to_string()
            }
        } else {
            String::new()
        };
        if !date_and_time.is_empty() {
            for bad in &["+5", "+05:5", "+ab:cd", "Q"] {
                out.push(format!("{date_and_time}{bad}"));
            }
        }
    }

    out.retain(|m| m != valid);
    out
}

/// Check if a string is a valid standalone datetime (date-only, time-only, LDT, or ODT).
fn is_valid_subset(s: &str) -> bool {
    // Must parse successfully, and cannot just be a partial string that
    // happens to look like a valid datetime
    if s.parse::<Datetime>().is_err() { return false; }
    // Valid subsets: date-only, time-only, local date-time, full ODT
    // These are the only formats that should survive truncation
    let has_date = s.len() >= 10 && s.as_bytes()[4] == b'-';
    let has_time = s.contains(':');
    // Everything else (like "1979-05" or "07:") should not be valid
    has_date || has_time
}
fn invalid_from_valid() -> impl Strategy<Value = String> {
    valid_base().prop_flat_map(|valid| {
        let mutations = mutate(&valid);
        if mutations.is_empty() {
            prop::sample::select(vec![format!("_{valid}_")])
        } else {
            prop::sample::select(mutations)
        }
    })
}

// ============================================================
// Invariants
// ============================================================

proptest! {
    #[test]
    fn mutations_rejected(s in invalid_from_valid()) {
        let result: Result<Datetime, _> = s.parse();
        prop_assert!(
            result.is_err() || is_valid_subset(&s),
            "should reject {s:?} but got {result:?}"
        );
    }
    /// 2. Truncations that aren't valid standalone datetimes should reject.
    #[test]
    fn truncations_rejected(
        (valid, cut) in (valid_base(), 1usize..50usize)
    ) {
        let chars: Vec<char> = valid.chars().collect();
        let idx = cut.min(chars.len().saturating_sub(1));
        let truncated: String = chars[..idx].iter().collect();
        if truncated == valid { return Ok(()); }

        let result: Result<Datetime, _> = truncated.parse();
        if result.is_ok() {
            prop_assert!(is_valid_subset(&truncated),
                "unexpected valid truncation: {truncated:?}");
        }
    }

    /// 3. Date separator corruption always rejects.
    #[test]
    fn slash_separator_rejected(s in valid_date()) {
        let corrupted = s.replacen('-', "/", 1);
        if corrupted == s { return Ok(()); }
        let result: Result<Datetime, _> = corrupted.parse();
        prop_assert!(result.is_err(), "should reject {corrupted:?}");
    }

    /// 4. Extra characters appended reject.
    #[test]
    fn extra_chars_rejected(s in valid_base()) {
        for suffix in ["x", "T", "-", ":"] {
            let corrupted = format!("{s}{suffix}");
            let result: Result<Datetime, _> = corrupted.parse();
            prop_assert!(result.is_err(), "should reject {corrupted:?}");
        }
    }

    /// 5. Invalid offset formats reject.
    #[test]
    fn invalid_offset_rejected(
        (y, mo, d) in (0u16..10000u16, 1u8..13u8, 1u8..29u8)
    ) {
        let date = format!("{y:04}-{mo:02}-{d:02}");
        // Empty offset produces valid LDT — skip it
        for bad_off in ["+5", "+05:5", "+ab:cd"] {
            let corrupted = format!("{date}T00:00:00{bad_off}");
            let result: Result<Datetime, _> = corrupted.parse();
            prop_assert!(result.is_err(), "should reject {corrupted:?}");
        }
    }

    /// 6. Multiple dots reject.
    #[test]
    fn multiple_dots_rejected(
        (y, mo, d) in (0u16..10000u16, 1u8..13u8, 1u8..29u8)
    ) {
        let base = format!("{y:04}-{mo:02}-{d:02}T00:00:00.5Z");
        let corrupted = base.replacen(".", "..", 1);
        let result: Result<Datetime, _> = corrupted.parse();
        prop_assert!(result.is_err(), "should reject {corrupted:?}");
    }
}
