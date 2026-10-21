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
//!   neither a digit nor a dot; everything from that point on is an ignored
//!   suffix (e.g. the `beta` in `1.2beta`).
//!
//! This implementation additionally follows the VERS test suite's "recommended"
//! semantics: a missing trailing segment is equivalent to a zero segment, so
//! `1.2` and `1.2.0` compare and canonicalize identically.
//!
//! Segments are compared by significant length and then lexicographically
//! rather than being parsed into a fixed-width integer type, so arbitrarily
//! large segments (far beyond `u64::MAX`) compare correctly without overflow.

use crate::VersError;
use crate::constraint::NativeVersionConverter;
use derive_more::Display;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::str::FromStr;

/// Scheme identifier string for `intdot` versions.
pub const INTDOT_SCHEME: &str = "intdot";

/// An `intdot` version: one or more non-negative integers separated by dots,
/// with an optional ignored suffix starting at the first non-digit,
/// non-dot character.
///
/// The value stored internally is always in canonical form: each segment has
/// its leading zeros stripped (an all-zero segment becomes a single `"0"`),
/// and any trailing all-zero segments are trimmed (keeping at least one
/// segment). This makes `Eq`, `Ord`, and `Hash` trivially consistent with
/// each other, since two versions are equal exactly when their canonical
/// forms are the same string.
///
/// # Examples
///
/// ```
/// use vers_rs::schemes::intdot::IntDotVersion;
/// use std::str::FromStr;
///
/// let a = IntDotVersion::from_str("01.002.0003").unwrap();
/// let b = IntDotVersion::from_str("1.2.3").unwrap();
/// assert_eq!(a, b);
/// assert_eq!(a.to_string(), "1.2.3");
///
/// // A missing trailing segment is equivalent to a zero segment.
/// assert_eq!(
///     IntDotVersion::from_str("1.2").unwrap(),
///     IntDotVersion::from_str("1.2.0").unwrap()
/// );
///
/// // Comparison is numeric per segment, not lexicographic.
/// assert!(IntDotVersion::from_str("1.2").unwrap() < IntDotVersion::from_str("1.10").unwrap());
///
/// // Everything after the first non-digit, non-dot character is ignored.
/// assert_eq!(
///     IntDotVersion::from_str("1.2beta").unwrap(),
///     IntDotVersion::from_str("1.2rc1").unwrap()
/// );
/// ```
#[derive(Display, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    // `intdot` has no scheme-specific native range syntax of its own, so the
    // default `NativeVersionConverter` behavior (standard vers operators,
    // `|` separator, `*` wildcard) is used as-is.
}

impl Default for IntDotVersion {
    fn default() -> Self {
        IntDotVersion("0".to_string())
    }
}

impl FromStr for IntDotVersion {
    type Err = VersError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        canonicalize(s).map(IntDotVersion).map_err(|reason| {
            VersError::InvalidVersionFormat(
                INTDOT_SCHEME.to_string(),
                s.to_string(),
                reason.to_string(),
            )
        })
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

impl PartialOrd for IntDotVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for IntDotVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        // Both sides are in canonical form: dot-separated runs of ASCII
        // digits with no leading zeros (an all-zero segment is "0"), and no
        // trailing all-zero segments. Segments are compared pairwise; when
        // one side runs out of segments first, the comparison is between it
        // and an implicit zero segment for every remaining position.
        let mut a = self.0.split('.');
        let mut b = other.0.split('.');
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
                // The common prefix is exhausted. A canonical string never
                // ends in a zero segment, so whichever side still has
                // segments left necessarily contains a non-zero value
                // somewhere in them, making it strictly greater.
                (None, Some(_)) => return Ordering::Less,
                (Some(_), None) => return Ordering::Greater,
            }
        }
    }
}

/// Parse the longest `digits ('.' digits)*` prefix of `s` into its canonical
/// `intdot` form, ignoring everything from the first character that is
/// neither an ASCII digit nor a dot immediately followed by an ASCII digit.
fn canonicalize(s: &str) -> Result<String, &'static str> {
    let bytes = s.as_bytes();

    if bytes.is_empty() {
        return Err("version must not be empty");
    }
    if !bytes[0].is_ascii_digit() {
        return Err("version must start with an ASCII digit");
    }

    let mut canonical = String::with_capacity(s.len());
    let mut i = 0;
    loop {
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        // Safe: `start..i` spans only ASCII digit bytes, so it's always a
        // valid (and non-empty, on first entry guaranteed by the check
        // above, and on later entries by the lookahead below) UTF-8 slice.
        let segment = &s[start..i];
        let significant = match segment.bytes().position(|b| b != b'0') {
            Some(p) => &segment[p..],
            None => "0",
        };

        if !canonical.is_empty() {
            canonical.push('.');
        }
        canonical.push_str(significant);

        // Only continue past a dot when it's immediately followed by
        // another digit; otherwise everything from here on (a trailing
        // dot, a run of dots, or any other character) is an ignored suffix.
        if i + 1 < bytes.len() && bytes[i] == b'.' && bytes[i + 1].is_ascii_digit() {
            i += 1;
        } else {
            break;
        }
    }

    // Trim trailing all-zero segments (but always keep at least one),
    // so that e.g. "1.2.0.0" canonicalizes identically to "1.2".
    while let Some(dot) = canonical.rfind('.') {
        if &canonical[dot + 1..] == "0" {
            canonical.truncate(dot);
        } else {
            break;
        }
    }

    Ok(canonical)
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
    use super::*;
    use crate::Comparator;
    use crate::range::VersionRange;
    use crate::range::dynamic::DynamicVersionRange;
    use crate::range::vers::VersVersionRange;

    // --- Parsing / canonicalization ---------------------------------------

    #[test]
    fn leading_zeros_are_stripped_per_segment() {
        assert_eq!(
            IntDotVersion::from_str("01.002.0003").unwrap().to_string(),
            "1.2.3"
        );
    }

    #[test]
    fn trailing_zero_segments_are_trimmed() {
        assert_eq!(IntDotVersion::from_str("1.2.0").unwrap().to_string(), "1.2");
        assert_eq!(
            IntDotVersion::from_str("1.2.0.0").unwrap().to_string(),
            "1.2"
        );
        assert_eq!(IntDotVersion::from_str("0.0").unwrap().to_string(), "0");
        assert_eq!(IntDotVersion::from_str("0").unwrap().to_string(), "0");
    }

    #[test]
    fn suffix_after_first_non_digit_non_dot_char_is_ignored() {
        assert_eq!(
            IntDotVersion::from_str("1.2beta").unwrap().to_string(),
            "1.2"
        );
        assert_eq!(
            IntDotVersion::from_str("1.10alpha").unwrap().to_string(),
            "1.10"
        );
        assert_eq!(
            IntDotVersion::from_str("1.3rc1").unwrap().to_string(),
            "1.3"
        );
        assert_eq!(IntDotVersion::from_str("10e2").unwrap().to_string(), "10");
    }

    #[test]
    fn trailing_dot_and_double_dot_stop_interpretation() {
        // A trailing dot with nothing after it: interpretation stops before it.
        assert_eq!(IntDotVersion::from_str("1.2.").unwrap().to_string(), "1.2");
        // A dot not followed by a digit stops interpretation, even mid-string.
        assert_eq!(IntDotVersion::from_str("1..2").unwrap().to_string(), "1");
        assert_eq!(
            IntDotVersion::from_str("1.2.beta").unwrap().to_string(),
            "1.2"
        );
    }

    #[test]
    fn invalid_versions_are_rejected() {
        for invalid in ["", "beta", ".1", "v1.2", "-1.2"] {
            assert!(
                IntDotVersion::from_str(invalid).is_err(),
                "expected {invalid:?} to be rejected"
            );
        }
    }

    #[test]
    fn non_ascii_digits_are_not_accepted_as_digits() {
        // "١٠" is Arabic-Indic for "10"; it must not be treated as a digit.
        assert!(IntDotVersion::from_str("١٠").is_err());
        // The numeric prefix stops before non-ASCII characters, same as any
        // other non-digit, non-dot byte.
        assert_eq!(IntDotVersion::from_str("1.٢").unwrap().to_string(), "1");
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
        let mut versions: Vec<IntDotVersion> = ["1.0002.10", "1.2.2", "1.02.0011"]
            .into_iter()
            .map(|s| IntDotVersion::from_str(s).unwrap())
            .collect();
        versions.sort();
        let sorted: Vec<String> = versions.iter().map(ToString::to_string).collect();
        assert_eq!(sorted, vec!["1.2.2", "1.2.10", "1.2.11"]);
    }

    #[test]
    fn suffixes_are_ignored_for_comparison() {
        let mut versions: Vec<IntDotVersion> = ["1.2beta", "1.10alpha", "1.3rc1"]
            .into_iter()
            .map(|s| IntDotVersion::from_str(s).unwrap())
            .collect();
        versions.sort();
        let sorted: Vec<String> = versions.iter().map(ToString::to_string).collect();
        assert_eq!(sorted, vec!["1.2", "1.3", "1.10"]);
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
        // Suffixes are ignored, same as the typed API.
        assert!(range.contains("1.2beta".to_string()).unwrap());
    }

    #[test]
    fn dynamic_parse_native_intdot() {
        let range = DynamicVersionRange::parse_native("intdot", ">=01.2|<2.0.0").unwrap();
        assert_eq!(range.versioning_scheme(), "intdot");
        // Canonicalized on parse.
        assert_eq!(range.to_string(), "vers:intdot/>=1.2|<2");
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
        let v = IntDotVersion::from_str("01.2.0").unwrap();
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, "\"1.2\"");
        let roundtripped: IntDotVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(v, roundtripped);
    }

    #[test]
    fn typed_serde_rejects_invalid_version_strings() {
        let result: Result<IntDotVersion, _> = serde_json::from_str("\"beta\"");
        assert!(result.is_err());
    }
}
