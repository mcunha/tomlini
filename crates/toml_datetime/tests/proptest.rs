//! Property-based tests for `toml_datetime`.
//!
//! Generates valid and invalid datetime strings and verifies invariants:
//! - Parse + Display roundtrip
//! - Component survival through parse
//! - Invalid strings are rejected

use proptest::prelude::*;
use toml_datetime::{Date, Datetime, Time};

// ============================================================
// Generators
// ============================================================

fn year() -> impl Strategy<Value = u16> {
    (0u16..10000u16)
}

fn month() -> impl Strategy<Value = u8> {
    (1u8..13u8)
}

fn day() -> impl Strategy<Value = u8> {
    (1u8..29u8) // safe range — no month-specific validation in generator
}

fn hour() -> impl Strategy<Value = u8> {
    (0u8..24u8)
}

fn minute() -> impl Strategy<Value = u8> {
    (0u8..60u8)
}

fn second() -> impl Strategy<Value = u8> {
    (0u8..61u8) // 0-60 (leap second allowed)
}

fn nanosecond() -> impl Strategy<Value = u32> {
    (0u32..1_000_000_000u32)
}

/// Generate a valid TOML offset: Z or ±HH:MM
fn offset_str() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("Z".to_string()),
        Just("z".to_string()),
        (0u8..24u8, 0u8..60u8).prop_map(|(h, m)| format!("+{h:02}:{m:02}")),
        (0u8..24u8, 0u8..60u8).prop_map(|(h, m)| format!("-{h:02}:{m:02}")),
    ]
}

/// Separator: T, t, or space
fn separator() -> impl Strategy<Value = char> {
    prop_oneof![Just('T'), Just('t'), Just(' ')]
}

// ============================================================
// Valid datetime generators
// ============================================================

/// Full offset date-time: YYYY-MM-DD (T|t| ) HH:MM:SS (Z|±HH:MM)
fn odt_str() -> impl Strategy<Value = String> {
    (year(), month(), day(), separator(), hour(), minute(), second(), nanosecond(), offset_str())
        .prop_map(|(y, mo, d, sep, h, mi, s, ns, off)| {
            if ns == 0 {
                format!("{y:04}-{mo:02}-{d:02}{sep}{h:02}:{mi:02}:{s:02}{off}")
            } else {
                format!("{y:04}-{mo:02}-{d:02}{sep}{h:02}:{mi:02}:{s:02}.{ns:09}{off}")
            }
        })
}

/// Local date-time: YYYY-MM-DD (T|t| ) HH:MM:SS (no offset)
fn ldt_str() -> impl Strategy<Value = String> {
    (year(), month(), day(), separator(), hour(), minute(), second(), nanosecond())
        .prop_map(|(y, mo, d, sep, h, mi, s, ns)| {
            if ns == 0 {
                format!("{y:04}-{mo:02}-{d:02}{sep}{h:02}:{mi:02}:{s:02}")
            } else {
                format!("{y:04}-{mo:02}-{d:02}{sep}{h:02}:{mi:02}:{s:02}.{ns:09}")
            }
        })
}

/// Local date: YYYY-MM-DD
fn date_str() -> impl Strategy<Value = String> {
    (year(), month(), day())
        .prop_map(|(y, mo, d)| format!("{y:04}-{mo:02}-{d:02}"))
}

/// Local time: HH:MM:SS
fn time_str() -> impl Strategy<Value = String> {
    (hour(), minute(), second(), nanosecond())
        .prop_map(|(h, mi, s, ns)| {
            if ns == 0 {
                format!("{h:02}:{mi:02}:{s:02}")
            } else {
                format!("{h:02}:{mi:02}:{s:02}.{ns:09}")
            }
        })
}

/// Any valid TOML datetime string
fn valid_datetime() -> impl Strategy<Value = String> {
    prop_oneof![
        odt_str(),
        ldt_str(),
        date_str(),
        time_str(),
    ]
}

// ============================================================
// Invalid datetime generators
// ============================================================

/// Strings that look like datetimes but have invalid components
fn invalid_datetime() -> impl Strategy<Value = String> {
    prop_oneof![
        // Month out of range
        (year(), (13u8..100u8), day()).prop_map(|(y, mo, d)| format!("{y:04}-{mo:02}-{d:02}")),
        // Day too large for any month
        (year(), month(), (32u8..100u8)).prop_map(|(y, mo, d)| format!("{y:04}-{mo:02}-{d:02}")),
        // Hour 24 (not allowed)
        (24u8..100u8, minute(), second()).prop_map(|(h, mi, s)| format!("{h:02}:{mi:02}:{s:02}")),
        // Minute 60+ (not allowed)
        (hour(), (60u8..100u8), second()).prop_map(|(h, mi, s)| format!("{h:02}:{mi:02}:{s:02}")),
    ]
}

// ============================================================
// Invariants
// ============================================================

proptest! {
    /// 1. Valid datetimes parse and roundtrip through Display.
    #[test]
    fn valid_parse_and_roundtrip(s in valid_datetime()) {
        let dt: Datetime = s.parse().unwrap();
        let output = dt.to_string();

        // Parse the output again — must be stable
        let dt2: Datetime = output.parse().unwrap();
        prop_assert_eq!(dt, dt2);

        // Display must not be empty
        prop_assert!(!output.is_empty());
    }

    /// 2. Component values survive the parse.
    #[test]
    fn components_survive_parse(
        (y, mo, d, h, mi, s) in (0u16..10000u16, 1u8..13u8, 1u8..29u8, 0u8..24u8, 0u8..60u8, 0u8..61u8)
    ) {
        let input = format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z");
        let Ok(dt) = input.parse::<Datetime>() else { return Ok(()); };

        let date = dt.date.unwrap();
        prop_assert_eq!(date.year, y);
        prop_assert_eq!(date.month, mo);
        prop_assert_eq!(date.day, d);

        let time = dt.time.unwrap();
        prop_assert_eq!(time.hour, h);
        prop_assert_eq!(time.minute, mi);
        prop_assert_eq!(time.second, Some(s));
    }

    /// 3. Invalid datetimes are rejected.
    #[test]
    fn invalid_rejected(s in invalid_datetime()) {
        let result: Result<Datetime, _> = s.parse();
        prop_assert!(result.is_err(), "should reject {s:?} but got {result:?}");
    }

    /// 4. Arbitrary strings should not panic.
    #[test]
    fn never_panics(s in "\\PC{0,128}") {
        let _: Result<Datetime, _> = s.parse();
    }

    /// 5. Time components survive parse.
    #[test]
    fn time_components_survive(
        (h, mi, s) in (0u8..24u8, 0u8..60u8, 0u8..61u8)
    ) {
        let input = format!("{h:02}:{mi:02}:{s:02}");
        let Ok(dt) = input.parse::<Datetime>() else { return Ok(()); };

        let time = dt.time.unwrap();
        prop_assert_eq!(time.hour, h);
        prop_assert_eq!(time.minute, mi);
        prop_assert_eq!(time.second, Some(s));
    }

    /// 6. Date components survive parse.
    #[test]
    fn date_components_survive(
        (y, mo, d) in (0u16..10000u16, 1u8..13u8, 1u8..29u8)
    ) {
        let input = format!("{y:04}-{mo:02}-{d:02}");
        let Ok(dt) = input.parse::<Datetime>() else { return Ok(()); };

        let date = dt.date.unwrap();
        prop_assert_eq!(date.year, y);
        prop_assert_eq!(date.month, mo);
        prop_assert_eq!(date.day, d);
    }

    /// 7. Offset date-time Display preserves offset sign.
    #[test]
    fn offset_preserved(
        (y, mo, d, h, mi, s, off) in (0u16..10000u16, 1u8..13u8, 1u8..29u8, 0u8..24u8, 0u8..60u8, 0u8..61u8, offset_str())
    ) {
        let input = format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}{off}");
        let Ok(dt) = input.parse::<Datetime>() else { return Ok(()); };
        let output = dt.to_string();
        // Parse again
        let dt2: Datetime = output.parse().unwrap();
        prop_assert_eq!(dt, dt2);
    }

    /// 8. Nanosecond values survive parse and display correctly.
    #[test]
    fn nanosecond_survives(
        ns in (0u32..1_000_000_000u32)
    ) {
        let input = format!("1979-05-27T07:32:00.{ns:09}Z");
        let Ok(dt) = input.parse::<Datetime>() else { return Ok(()); };
        let time = dt.time.unwrap();
        prop_assert_eq!(time.nanosecond, Some(ns));
    }
}
