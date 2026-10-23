//! Version type for the generic `intdot` VERS scheme.
//!
//! `intdot` is a generic, ecosystem-independent versioning scheme defined by the
//! [VERS specification](https://github.com/package-url/vers-spec). It covers version
//! strings that don't follow one of the well-known schemes (semver, deb, ...), but
//! are still "integers separated by dots", e.g. `10.234.5.12`.
//!
//! Per the specification:
//! - Versions consist of ASCII digits only, formatted as non-negative integers
//!   separated by dots (e.g. `10.234.5.12`).
//! - Leading zeros in each segment are ignored (`01.002` == `1.2`).
//! - Interpretation of the version stops at the first character that is
//!   neither a digit nor a dot; everything from that point on is a suffix
//!   that is ignored for comparison (e.g. the `beta` in `1.2beta`), but
//!   kept verbatim so that versions roundtrip through display and
//!   (de)serialization.
//! - Consecutive dots are rejected as an error only when immediately
//!   followed by a digit (e.g. `1..2`), since that would otherwise be
//!   interpreted as a segment separator with an empty segment in between.
//!   Consecutive dots not followed by a digit are simply part of the
//!   suffix, like any other non-digit content (e.g. `1..`, `1..beta`).
//!
//! This implementation additionally follows the VERS test suite's "recommended"
//! semantics: a missing trailing segment is equivalent to a zero segment, so
//! `1.2` and `1.2.0` compare equal (even though both remain valid, distinct
//! ways to spell the same version).
//!
//! ## There is no separate canonical form
//!
//! Unlike some other schemes in this crate (e.g. `SemVer`), the VERS
//! specification does not define a stricter "canonical form" for `intdot`
//! versions beyond the lenient rules above. Leading zeros are therefore
//! stripped unconditionally by [`FromStr`], while a suffix is kept verbatim
//! (it only affects comparison), both for standard `vers:intdot/...`
//! strings and for native-syntax parsing (via
//! [`NativeVersionConverter::from_native_constraint`], used by e.g.
//! [`DynamicVersionRange::parse_native`](crate::range::dynamic::DynamicVersionRange::parse_native)),
//! as well as for the version being tested in `DynamicVersionRange::contains`.
//!
//! Note that an explicit trailing zero segment (e.g. `2.0`, `1.2.0.0`) is
//! *not* considered non-canonical: it has no leading zeros, so it's accepted
//! and stored exactly as written. Only equality/ordering treat it as
//! equivalent to the shorter spelling.
//!
//! Segments are compared by significant length and then lexicographically
//! rather than being parsed into a fixed-width integer type, so arbitrarily
//! large segments (far beyond `u64::MAX`) compare correctly without overflow.

use crate::VersError;
use crate::constraint::NativeVersionConverter;
use derive_more::Display;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

/// Scheme identifier string for `intdot` versions.
pub const INTDOT_SCHEME: &str = "intdot";

/// An `intdot` version: one or more non-negative integers separated by dots.
///
/// The value stored internally is the normalized string produced by parsing
/// (see [`FromStr`]): leading zeros per segment are stripped, while any
/// trailing suffix is kept verbatim (it is ignored for comparison, but
/// retained so that parsing, display, and (de)serialization roundtrip).
/// `Eq`, `Ord`, and `Hash` are implemented manually so that a missing
/// trailing segment is treated as equivalent to an explicit zero segment,
/// without requiring that equivalence to be reflected in storage.
///
/// # Examples
///
/// ```
/// use vers_rs::schemes::intdot::IntDotVersion;
/// use std::str::FromStr;
///
/// // Parsing tolerates leading zeros and keeps any suffix verbatim (there
/// // is no separate, stricter canonical form for this scheme).
/// let a = IntDotVersion::from_str("1.2.3").unwrap();
/// assert_eq!(a, IntDotVersion::from_str("01.2.3").unwrap());
///
/// // The suffix is kept for display, but ignored for comparison.
/// assert_eq!(
///     IntDotVersion::from_str("1.2beta").unwrap().to_string(),
///     "1.2beta"
/// );
/// assert_eq!(
///     IntDotVersion::from_str("1.2beta").unwrap(),
///     IntDotVersion::from_str("1.2").unwrap()
/// );
///
/// // A missing trailing segment is equivalent to an explicit zero segment.
/// assert_eq!(a, IntDotVersion::from_str("1.2.3.0").unwrap());
///
/// // Comparison is numeric per segment, not lexicographic.
/// assert!(IntDotVersion::from_str("1.2").unwrap() < IntDotVersion::from_str("1.10").unwrap());
/// ```
#[derive(Display, Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
// `String` would normally generate a `string` TS type on its own, but since
// (de)serialization goes through the `try_from`/`into` attributes above
// rather than the derived (transparent) newtype behaviour, make the override
// explicit to match what actually crosses the wasm ABI (see `SemVer` for the
// same pattern).
#[cfg_attr(feature = "wasm", tsify(type = "string"))]
pub struct IntDotVersion(String);

impl NativeVersionConverter for IntDotVersion {
    const SCHEME_NAME: &'static str = INTDOT_SCHEME;

    // `intdot` uses the standard vers operators, separator, wildcard, and
    // constraint-parsing logic (the trait defaults): since `FromStr` for
    // `IntDotVersion` already parses leniently (there's no separate
    // canonical form for this scheme; see the [module documentation](self)),
    // no override is needed here.
}

impl Default for IntDotVersion {
    fn default() -> Self {
        IntDotVersion("0".to_string())
    }
}

impl FromStr for IntDotVersion {
    type Err = VersError;

    /// Parses `s` into an `intdot` version, tolerating non-canonical input:
    /// leading zeros per segment (stripped), and any suffix starting at the
    /// first character that is neither an ASCII digit nor a dot immediately
    /// followed by another digit (kept verbatim, but ignored for comparison;
    /// see `Eq`/`Ord`/`Hash`). There is no separate, stricter canonical form
    /// for this scheme (see the [module documentation](self)), so this
    /// leniency applies equally to standard `vers:intdot/...` strings,
    /// native-syntax parsing, and the version being tested in
    /// [`VersionRange::contains`](crate::range::VersionRange::contains).
    ///
    /// Consecutive dots are rejected as an error only when they are
    /// immediately followed by another digit, i.e. when they would
    /// otherwise be interpreted as a segment separator with an empty
    /// segment in between (e.g. `1..2`). Consecutive dots that are *not*
    /// followed by a digit are simply part of the suffix, same as any
    /// other non-digit content (e.g. `1..beta`, or a bare trailing `1..`).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() || !s.as_bytes()[0].is_ascii_digit() {
            return Err(VersError::InvalidVersionFormat(
                INTDOT_SCHEME.to_string(),
                s.to_string(),
                "version must start with an ASCII digit".to_string(),
            ));
        }

        let (numeric, suffix) = split_numeric_and_suffix(s);

        // A run of consecutive dots at the numeric/suffix boundary that is
        // immediately followed by a digit (e.g. the `..2` in `1..2`) would
        // read as a segment separator with an empty segment in between, so
        // it is rejected. Consecutive dots *not* followed by a digit are
        // simply part of the suffix (e.g. `1..beta`, or a bare trailing
        // `1..`).
        if suffix.starts_with('.') {
            let after_dots = suffix.trim_start_matches('.');
            if after_dots.starts_with(|c: char| c.is_ascii_digit()) {
                return Err(VersError::InvalidVersionFormat(
                    INTDOT_SCHEME.to_string(),
                    s.to_string(),
                    "version must not contain consecutive dots".to_string(),
                ));
            }
        }

        // Strip leading zeros within each numeric segment.
        let mut normalized = String::with_capacity(s.len());
        for segment in numeric.split('.') {
            let significant_digits = match segment.bytes().position(|b| b != b'0') {
                Some(p) => &segment[p..],
                None => "0",
            };
            if !normalized.is_empty() {
                normalized.push('.');
            }
            normalized.push_str(significant_digits);
        }

        // Keep the suffix verbatim: `Eq`/`Ord`/`Hash` ignore it (see
        // `significant`), but preserving it ensures that parsing, `Display`,
        // and (de)serialization roundtrip.
        normalized.push_str(suffix);

        Ok(IntDotVersion(normalized))
    }
}

impl TryFrom<String> for IntDotVersion {
    type Error = VersError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<IntDotVersion> for String {
    fn from(v: IntDotVersion) -> String {
        v.0
    }
}

impl PartialEq for IntDotVersion {
    fn eq(&self, other: &Self) -> bool {
        significant(&self.0) == significant(&other.0)
    }
}

impl Eq for IntDotVersion {}

impl Hash for IntDotVersion {
    fn hash<H: Hasher>(&self, state: &mut H) {
        significant(&self.0).hash(state);
    }
}

impl PartialOrd for IntDotVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for IntDotVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        // Compare the "significant" form of each side (trailing all-zero
        // segments trimmed) segment-by-segment. Since neither significant
        // form has leading zeros or a trailing zero segment, the side that
        // runs out of segments first is always the smaller one: its
        // counterpart's remaining segments necessarily end in a non-zero
        // value (that's exactly what "significant" guarantees).
        let mut a = significant(&self.0).split('.');
        let mut b = significant(&other.0).split('.');
        loop {
            match (a.next(), b.next()) {
                (Some(x), Some(y)) => {
                    // Neither segment has leading zeros, so the longer
                    // digit run is always the larger number, and equal-length
                    // runs order the same lexicographically as numerically.
                    let ord = x.len().cmp(&y.len()).then_with(|| x.cmp(y));
                    if ord != Ordering::Equal {
                        return ord;
                    }
                }
                (None, None) => return Ordering::Equal,
                (None, Some(_)) => return Ordering::Less,
                (Some(_), None) => return Ordering::Greater,
            }
        }
    }
}

/// Splits `s` into its leading numeric part and the suffix that follows it.
///
/// The numeric part spans ASCII digit segments separated by single dots,
/// where a dot counts as a separator only when it is immediately followed
/// by another digit; it ends at the first character that is neither a digit
/// nor such a separator dot. Everything from that point on is the suffix.
///
/// This shared scan is the single source of truth for where the numeric
/// part ends: [`FromStr`] keeps the suffix verbatim in the stored value,
/// while [`significant`] strips it off for comparison, so the two can
/// never drift apart. Note that this function does not validate the
/// boundary: a run of consecutive dots at the split point that is
/// immediately followed by a digit (e.g. the `..2` in `1..2`) lands in the
/// suffix, and is rejected separately by `FromStr`.
fn split_numeric_and_suffix(s: &str) -> (&str, &str) {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            i += 1;
        } else if bytes[i] == b'.' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
            i += 2;
        } else {
            break;
        }
    }
    s.split_at(i)
}

/// Returns the comparison/hash form of `s`: `s` with any ignored suffix
/// removed and any trailing all-zero segments trimmed (keeping at least one
/// segment), so that e.g. `"1.2.0"`, `"1.2.0.0"`, and `"1.2.0beta"` all
/// reduce to `"1.2"`. `s` is assumed to already have no leading zeros per
/// segment (guaranteed by [`FromStr`]).
///
/// This is deliberately *not* used for `Display`/storage: an explicit
/// trailing zero segment or a suffix is a valid spelling of a version (see
/// the [module documentation](self)), and is preserved verbatim there. It's
/// only `Eq`, `Ord`, and `Hash` that treat it as equivalent to the plain
/// numeric form.
fn significant(s: &str) -> &str {
    let (numeric, _) = split_numeric_and_suffix(s);
    let mut result = numeric;

    while let Some(dot) = result.rfind('.') {
        if &result[dot + 1..] == "0" {
            result = &result[..dot];
        } else {
            break;
        }
    }
    result
}

#[cfg(all(test, feature = "wasm"))]
mod tsify_decl_check {
    use super::IntDotVersion;
    use tsify::Tsify;

    #[test]
    fn intdot_version_ts_decl_is_string() {
        // `DECL` also includes a JSDoc block generated from this type's doc
        // comment, so check the generated type alias itself rather than an
        // exact match of the whole declaration.
        assert!(IntDotVersion::DECL.ends_with("export type IntDotVersion = string;"));
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::Comparator;
    use crate::range::VersionRange;
    use crate::range::dynamic::DynamicVersionRange;
    use crate::range::vers::VersVersionRange;

    // --- Parsing (there is no separate canonical form) ----------------------

    #[test]
    fn parsing_accepts_already_normalized_forms_verbatim() {
        for normalized in ["0", "1", "1.2.3", "2.0", "1.2.0.0", "18446744073709551616"] {
            let v = IntDotVersion::from_str(normalized)
                .unwrap_or_else(|e| panic!("expected {normalized:?} to be accepted, got {e}"));
            assert_eq!(v.to_string(), normalized);
        }
    }

    #[test]
    fn parsing_rejects_structurally_invalid_versions() {
        for invalid in ["", "beta", ".1", "v1.2", "-1.2"] {
            assert!(
                IntDotVersion::from_str(invalid).is_err(),
                "expected {invalid:?} to be rejected"
            );
        }
    }

    #[test]
    fn parsing_rejects_consecutive_dots_followed_by_a_digit() {
        // Consecutive dots immediately followed by a digit would otherwise
        // be interpreted as a segment separator with an empty segment in
        // between, so they're rejected.
        for invalid in ["1..2", "1...2", "1.2..3"] {
            assert!(
                IntDotVersion::from_str(invalid).is_err(),
                "expected {invalid:?} to be rejected"
            );
        }
    }

    #[test]
    fn parsing_allows_consecutive_dots_in_an_ignored_suffix() {
        // Consecutive dots *not* followed by a digit are just part of the
        // suffix, same as any other non-digit content: kept verbatim, but
        // ignored for comparison.
        assert_eq!(IntDotVersion::from_str("1..").unwrap().to_string(), "1..");
        assert_eq!(
            IntDotVersion::from_str("1.2.3..").unwrap().to_string(),
            "1.2.3.."
        );
        assert_eq!(
            IntDotVersion::from_str("1..beta").unwrap().to_string(),
            "1..beta"
        );
    }

    #[test]
    fn standard_vers_string_accepts_non_normalized_version() {
        // There is no separate canonical form for `intdot`, so standard
        // `vers:` strings are parsed just as leniently as native syntax.
        let result: Result<DynamicVersionRange, VersError> = "vers:intdot/01.2".parse();
        assert!(result.is_ok());
    }

    #[rstest]
    #[case("1.2.a", "1.2.a")]
    #[case("1.2.3a", "1.2.3a")]
    #[case("1.a.3", "1.a.3")]
    #[case("1.2a.3", "1.2a.3")]
    fn non_digit_stops_segment_parsing_keeps_suffix(#[case] input: &str, #[case] expected: &str) {
        // A non-digit stops segment parsing, but the remaining suffix is
        // kept verbatim (and ignored for comparison).
        let result = IntDotVersion::from_str(input).unwrap();
        assert_eq!(result.0, expected);
    }

    #[test]
    fn parsing_strips_leading_zeros_per_segment() {
        assert_eq!(
            IntDotVersion::from_str("01.002.0003").unwrap().to_string(),
            "1.2.3"
        );
        assert_eq!(
            IntDotVersion::from_str("01.000.0003").unwrap().to_string(),
            "1.0.3"
        );
        assert_eq!(
            IntDotVersion::from_str("00.000.0003").unwrap().to_string(),
            "0.0.3"
        );
    }

    #[test]
    fn parsing_preserves_trailing_zero_segments() {
        // Unlike leading zeros, an explicit trailing zero segment is not
        // stripped by normalization; it's a valid spelling on its own.
        assert_eq!(
            IntDotVersion::from_str("1.2.0").unwrap().to_string(),
            "1.2.0"
        );
        assert_eq!(
            IntDotVersion::from_str("01.2.0").unwrap().to_string(),
            "1.2.0"
        );
    }

    #[test]
    fn suffix_after_first_non_digit_non_dot_char_is_kept_for_display() {
        // The suffix does not contribute segments, but it is preserved
        // verbatim so that display and (de)serialization roundtrip; only
        // comparison ignores it.
        assert_eq!(
            IntDotVersion::from_str("1.2beta").unwrap().to_string(),
            "1.2beta"
        );
        assert_eq!(
            IntDotVersion::from_str("1.10alpha").unwrap().to_string(),
            "1.10alpha"
        );
        assert_eq!(
            IntDotVersion::from_str("1.3rc1").unwrap().to_string(),
            "1.3rc1"
        );
        assert_eq!(IntDotVersion::from_str("10e2").unwrap().to_string(), "10e2");
    }

    #[test]
    fn parsing_trailing_dot_stops_interpretation() {
        // A trailing dot (or a dot followed by a non-digit) ends the numeric
        // part; the remainder is kept verbatim as a suffix.
        assert_eq!(IntDotVersion::from_str("1.2.").unwrap().to_string(), "1.2.");
        assert_eq!(
            IntDotVersion::from_str("1.2.beta").unwrap().to_string(),
            "1.2.beta"
        );
    }

    #[test]
    fn parsing_non_ascii_digits_are_not_accepted_as_digits() {
        // "١٠" is Arabic-Indic for "10"; it must not be treated as a digit.
        assert!(IntDotVersion::from_str("١٠").is_err());
        // The numeric prefix stops before non-ASCII characters, same as any
        // other non-digit, non-dot byte; the rest is kept as a suffix.
        assert_eq!(IntDotVersion::from_str("1.٢").unwrap().to_string(), "1.٢");
    }

    // --- Equality (ported from the official VERS intdot test suite) -------

    #[test]
    fn identical_versions_are_equal() {
        assert_eq!(
            IntDotVersion::from_str("1.2.3").unwrap(),
            IntDotVersion::from_str("1.2.3").unwrap()
        );
    }

    #[test]
    fn leading_zeros_are_ignored_for_equality() {
        assert_eq!(
            IntDotVersion::from_str("01.002.0003").unwrap(),
            IntDotVersion::from_str("1.2.3").unwrap()
        );
    }

    #[test]
    fn missing_trailing_segment_is_equivalent_to_zero_segment() {
        assert_eq!(
            IntDotVersion::from_str("1.2").unwrap(),
            IntDotVersion::from_str("1.2.0").unwrap()
        );
    }

    #[test]
    fn distinct_numeric_versions_are_not_equal() {
        assert_ne!(
            IntDotVersion::from_str("1.2").unwrap(),
            IntDotVersion::from_str("1.3").unwrap()
        );
    }

    #[test]
    fn suffixes_are_ignored_for_equality() {
        assert_eq!(
            IntDotVersion::from_str("1.2beta").unwrap(),
            IntDotVersion::from_str("1.2rc1").unwrap()
        );
    }

    // --- Comparison (ported from the official VERS intdot test suite) -----

    #[test]
    fn comparison_is_numeric_per_segment_not_lexicographic() {
        let a = IntDotVersion::from_str("1.10").unwrap();
        let b = IntDotVersion::from_str("1.2").unwrap();
        assert!(b < a);
        assert!(a > b);
    }

    #[test]
    fn additional_non_zero_segment_sorts_after_its_prefix() {
        let a = IntDotVersion::from_str("1.2").unwrap();
        let b = IntDotVersion::from_str("1.2.1").unwrap();
        assert!(a < b);
    }

    #[test]
    fn mixed_leading_zero_forms_sort_by_normalized_numeric_segments() {
        let mut versions: Vec<IntDotVersion> = ["1.0002.10", "1.00.6", "1.2.2", "1.02.0011"]
            .into_iter()
            .map(|s| IntDotVersion::from_str(s).unwrap())
            .collect();
        versions.sort();
        let sorted: Vec<String> = versions.iter().map(ToString::to_string).collect();
        assert_eq!(sorted, vec!["1.0.6", "1.2.2", "1.2.10", "1.2.11"]);
    }

    #[test]
    fn suffixes_are_ignored_for_comparison() {
        let mut versions: Vec<IntDotVersion> = ["1.2beta", "1.10alpha", "1.3rc1"]
            .into_iter()
            .map(|s| IntDotVersion::from_str(s).unwrap())
            .collect();
        versions.sort();
        // Sorting is by the numeric segments only; the kept suffixes show up
        // verbatim in the display form.
        let sorted: Vec<String> = versions.iter().map(ToString::to_string).collect();
        assert_eq!(sorted, vec!["1.2beta", "1.3rc1", "1.10alpha"]);
    }

    #[test]
    fn arbitrarily_large_segments_compare_correctly() {
        // These exceed u64::MAX (2^64 - 1 = 18446744073709551615), which
        // would overflow a naive integer-parsing comparison.
        let a = IntDotVersion::from_str("18446744073709551615").unwrap();
        let b = IntDotVersion::from_str("18446744073709551616").unwrap();
        assert!(a < b);

        // 2^53 and 2^53 + 1 are not distinguishable as IEEE-754 doubles,
        // which has tripped up other implementations.
        let c = IntDotVersion::from_str("9007199254740992").unwrap();
        let d = IntDotVersion::from_str("9007199254740993").unwrap();
        assert!(c < d);
    }

    #[test]
    fn eq_is_consistent_with_ord() {
        let versions = [
            "1.2", "1.2.0", "1.2.0.0", "1.20", "01.2", "1.2beta", "1.2.1", "2", "0", "0.0",
        ];
        let parsed: Vec<IntDotVersion> = versions
            .iter()
            .map(|s| IntDotVersion::from_str(s).unwrap())
            .collect();
        for a in &parsed {
            for b in &parsed {
                assert_eq!(a == b, a.cmp(b) == Ordering::Equal, "{a} vs {b}");
            }
        }
    }

    // --- Containment (ported from the official VERS intdot test suite) ----

    #[test]
    fn containment_range_includes_values_inside_its_bounds() {
        let range: VersVersionRange<IntDotVersion> = "vers:intdot/>=1.2|<2.0".parse().unwrap();
        assert!(
            range
                .contains(IntDotVersion::from_str("1.5.0").unwrap())
                .unwrap()
        );
    }

    #[test]
    fn containment_range_includes_inclusive_lower_bound() {
        let range: VersVersionRange<IntDotVersion> = "vers:intdot/>=1.2|<2.0".parse().unwrap();
        assert!(
            range
                .contains(IntDotVersion::from_str("1.2").unwrap())
                .unwrap()
        );
    }

    #[test]
    fn containment_range_excludes_exclusive_upper_bound() {
        let range: VersVersionRange<IntDotVersion> = "vers:intdot/>=1.2|<2.0".parse().unwrap();
        assert!(
            !range
                .contains(IntDotVersion::from_str("2.0").unwrap())
                .unwrap()
        );
    }

    // --- Dynamic dispatch integration ---------------------------------------

    #[test]
    fn dynamic_parse_detects_intdot_scheme() {
        let range: DynamicVersionRange = "vers:intdot/>=1.2|<2.0".parse().unwrap();
        assert_eq!(range.versioning_scheme(), "intdot");
        assert_eq!(range.constraints().len(), 2);
    }

    #[test]
    fn dynamic_contains_uses_intdot_semantics() {
        let range: DynamicVersionRange = "vers:intdot/>=1.2|<2.0".parse().unwrap();
        assert!(range.contains("1.5.0".to_string()).unwrap());
        assert!(range.contains("1.2".to_string()).unwrap());
        assert!(!range.contains("2.0".to_string()).unwrap());
    }

    #[test]
    fn dynamic_contains_parses_tested_version_leniently() {
        // Both the range itself and the tested version passed to `contains`
        // are parsed leniently: leading zeros and an ignored suffix are
        // tolerated and normalized, not rejected.
        let range: DynamicVersionRange = "vers:intdot/>=1.2|<2.0".parse().unwrap();
        assert!(range.contains("1.2beta".to_string()).unwrap());
        assert!(range.contains("01.5.0".to_string()).unwrap());
        assert!(!range.contains("2.0beta".to_string()).unwrap());
    }

    #[test]
    fn dynamic_parse_native_intdot() {
        let range = DynamicVersionRange::parse_native("intdot", ">=01.2|<2.0.0").unwrap();
        assert_eq!(range.versioning_scheme(), "intdot");
        // Leading zeros are stripped, but the explicit trailing zero
        // segments in "2.0.0" are preserved, not trimmed.
        assert_eq!(range.to_string(), "vers:intdot/>=1.2|<2.0.0");
    }

    #[test]
    fn dynamic_parse_native_bare_version_is_equal_comparator() {
        let range = DynamicVersionRange::parse_native("intdot", "1.2.3").unwrap();
        assert_eq!(range.constraints().len(), 1);
        assert_eq!(range.constraints()[0].comparator, Comparator::Equal);
        assert_eq!(range.constraints()[0].version.to_string(), "1.2.3");
    }

    #[test]
    fn typed_serde_json_version_is_plain_string() {
        let v = IntDotVersion::from_str("1.2.0").unwrap();
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, "\"1.2.0\"");
        let roundtripped: IntDotVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(v, roundtripped);
    }

    #[test]
    fn typed_serde_rejects_invalid_version_strings() {
        let result: Result<IntDotVersion, _> = serde_json::from_str("\"beta\"");
        assert!(result.is_err());
    }

    #[test]
    fn typed_serde_normalizes_leading_zero() {
        let v: IntDotVersion = serde_json::from_str("\"01.2\"").unwrap();
        assert_eq!(v.to_string(), "1.2");
    }

    #[test]
    fn typed_serde_keeps_suffixed_version() {
        // The suffix is kept verbatim across (de)serialization so that
        // versions roundtrip; only comparison ignores it.
        let v: IntDotVersion = serde_json::from_str("\"1.2beta\"").unwrap();
        assert_eq!(v.to_string(), "1.2beta");
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, "\"1.2beta\"");
    }
}
