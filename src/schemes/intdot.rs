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
//! `1.2` and `1.2.0` compare equal (even though both remain valid, distinct
//! ways to spell the same version).
//!
//! ## Standard vs. native parsing
//!
//! Standard `vers:intdot/...` strings are parsed via [`FromStr`], which only
//! accepts versions **already** in canonical form: one or more ASCII digit
//! segments separated by single dots, with no leading zeros (a zero segment
//! is exactly `"0"`) and no trailing suffix. Non-canonical spellings (leading
//! zeros, an ignored suffix) are rejected outright rather than silently
//! normalized, matching how other schemes in this crate (e.g. `SemVer`)
//! require their standard vers strings to already be canonical.
//!
//! Native-syntax parsing (via
//! [`NativeVersionConverter::from_native_constraint`], used by e.g.
//! [`DynamicVersionRange::parse_native`](crate::range::dynamic::DynamicVersionRange::parse_native))
//! is more permissive: it tolerates and normalizes leading zeros and ignored
//! suffixes, since real-world version strings commonly include this kind of
//! non-canonical variation. The same lenient parsing is used for the version
//! being tested in
//! [`DynamicVersionRange::contains`](crate::range::dynamic::DynamicVersionRange::contains):
//! only the vers string itself (the range) is required to be in canonical
//! form.
//!
//! Note that an explicit trailing zero segment (e.g. `2.0`, `1.2.0.0`) is
//! *not* considered non-canonical by either parsing mode: it has no leading
//! zeros, so it's accepted and stored exactly as written by both. Only
//! equality/ordering treat it as equivalent to the shorter spelling.
//!
//! Segments are compared by significant length and then lexicographically
//! rather than being parsed into a fixed-width integer type, so arbitrarily
//! large segments (far beyond `u64::MAX`) compare correctly without overflow.

use crate::Comparator;
use crate::VersError;
use crate::VersionConstraint;
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
/// The value stored internally is exactly the string it was parsed from (via
/// strict [`FromStr`]) or normalized to (via lenient native parsing); see the
/// [module documentation](self) for the distinction. `Eq`, `Ord`, and `Hash`
/// are implemented manually so that a missing trailing segment is treated as
/// equivalent to an explicit zero segment, without requiring that
/// equivalence to be reflected in storage.
///
/// # Examples
///
/// ```
/// use vers_rs::schemes::intdot::IntDotVersion;
/// use std::str::FromStr;
///
/// // Standard parsing requires the canonical form: ASCII digit segments
/// // separated by dots, with no leading zeros.
/// let a = IntDotVersion::from_str("1.2.3").unwrap();
/// assert!(IntDotVersion::from_str("01.2.3").is_err());
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

    /// `intdot` uses the standard vers operators, separator, and wildcard
    /// (the trait defaults), but overrides constraint parsing so that the
    /// version itself is parsed leniently (tolerating leading zeros and an
    /// ignored suffix) rather than through the strict [`FromStr`], which is
    /// reserved for standard `vers:` strings. See the [module
    /// documentation](self) for details.
    fn from_native_constraint(raw: &str) -> Result<VersionConstraint<Self>, VersError> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(VersError::InvalidConstraint("Empty constraint".to_string()));
        }

        if Self::native_wildcard() == Some(raw) {
            return Ok(VersionConstraint::new(Comparator::Any, Self::default()));
        }

        let operator_match = Self::native_operators()
            .iter()
            .find_map(|(token, comparator)| {
                raw.strip_prefix(token).map(|rest| (*comparator, rest))
            });

        if let Some((comparator, version_str)) = operator_match {
            let version_str = version_str.trim();
            if version_str.is_empty() {
                return Err(VersError::InvalidConstraint("Missing version".to_string()));
            }
            Ok(VersionConstraint::new(
                comparator,
                parse_lenient(version_str)?,
            ))
        } else {
            // No operator token matched: treat the whole (trimmed) string as
            // an implicit-equal bare version, matching standard vers syntax.
            Ok(VersionConstraint::new(
                Comparator::Equal,
                parse_lenient(raw)?,
            ))
        }
    }
}

impl Default for IntDotVersion {
    fn default() -> Self {
        IntDotVersion("0".to_string())
    }
}

impl FromStr for IntDotVersion {
    type Err = VersError;

    /// Strictly parses `s` as a canonical `intdot` version: one or more
    /// ASCII digit segments separated by single dots, with no leading zeros
    /// (a zero segment must be exactly `"0"`) and nothing else. Unlike
    /// native parsing, this does **not** tolerate or normalize leading
    /// zeros or a trailing suffix; such input is rejected.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if is_canonical(s) {
            Ok(IntDotVersion(s.to_string()))
        } else {
            Err(VersError::InvalidVersionFormat(
                INTDOT_SCHEME.to_string(),
                s.to_string(),
                "version must be in canonical intdot form: one or more ASCII digit segments \
                 separated by single dots, with no leading zeros and no trailing characters \
                 (e.g. \"1.2.3\", not \"01.2.3\" or \"1.2.3beta\")"
                    .to_string(),
            ))
        }
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

/// Returns `true` if `s` is already in canonical `intdot` form: one or more
/// ASCII-digit segments separated by single dots, with no leading zeros (a
/// zero segment is exactly `"0"`), and nothing else (no suffix, no empty
/// segments, no leading/trailing dot).
fn is_canonical(s: &str) -> bool {
    !s.is_empty()
        && s.split('.').all(|segment| {
            !segment.is_empty()
                && segment.bytes().all(|b| b.is_ascii_digit())
                && (segment.len() == 1 || segment.as_bytes()[0] != b'0')
        })
}

/// Parse `s` leniently into an `intdot` version, tolerating non-canonical
/// input: leading zeros per segment (stripped), and any suffix starting at
/// the first character that is neither an ASCII digit nor a dot immediately
/// followed by another digit (dropped, along with any dot that precedes it).
///
/// Used for native-syntax parsing (see
/// [`NativeVersionConverter::from_native_constraint`] above) as well as for
/// parsing the version being tested in
/// [`VersionRange::contains`](crate::range::VersionRange::contains) (see
/// [`DynamicVersionRange::contains`](crate::range::dynamic::DynamicVersionRange::contains)),
/// since real-world version strings commonly include this kind of
/// non-canonical variation. Standard `vers:` strings themselves must still
/// already be in canonical form (see [`FromStr`]).
pub(crate) fn parse_lenient(s: &str) -> Result<IntDotVersion, VersError> {
    let bytes = s.as_bytes();

    if bytes.is_empty() || !bytes[0].is_ascii_digit() {
        return Err(VersError::InvalidVersionFormat(
            INTDOT_SCHEME.to_string(),
            s.to_string(),
            "version must start with an ASCII digit".to_string(),
        ));
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
        let significant_digits = match segment.bytes().position(|b| b != b'0') {
            Some(p) => &segment[p..],
            None => "0",
        };

        if !canonical.is_empty() {
            canonical.push('.');
        }
        canonical.push_str(significant_digits);

        // Only continue past a dot when it's immediately followed by
        // another digit; otherwise everything from here on (a trailing
        // dot, a run of dots, or any other character) is an ignored suffix.
        if i + 1 < bytes.len() && bytes[i] == b'.' && bytes[i + 1].is_ascii_digit() {
            i += 1;
        } else {
            break;
        }
    }

    Ok(IntDotVersion(canonical))
}

/// Returns the comparison/hash form of `s`: `s` with any trailing all-zero
/// segments trimmed (keeping at least one segment), so that e.g. `"1.2.0"`
/// and `"1.2.0.0"` both reduce to `"1.2"`. `s` is assumed to already have no
/// leading zeros per segment (guaranteed by both `FromStr` and
/// [`parse_lenient`]).
///
/// This is deliberately *not* used for `Display`/storage: an explicit
/// trailing zero segment is a valid, distinct spelling of a version (see the
/// [module documentation](self)), and is preserved verbatim there. It's only
/// `Eq`, `Ord`, and `Hash` that treat it as equivalent to the shorter form.
fn significant(s: &str) -> &str {
    let mut result = s;
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
    use super::*;
    use crate::range::VersionRange;
    use crate::range::dynamic::DynamicVersionRange;
    use crate::range::vers::VersVersionRange;

    // --- Strict (standard vers) parsing -------------------------------------

    #[test]
    fn strict_parsing_accepts_canonical_forms() {
        for canonical in ["0", "1", "1.2.3", "2.0", "1.2.0.0", "18446744073709551616"] {
            let v = IntDotVersion::from_str(canonical)
                .unwrap_or_else(|e| panic!("expected {canonical:?} to be accepted, got {e}"));
            // Strict parsing never rewrites its input: it's accepted verbatim.
            assert_eq!(v.to_string(), canonical);
        }
    }

    #[test]
    fn strict_parsing_rejects_non_canonical_or_invalid_forms() {
        for invalid in [
            "", "beta", ".1", "1.", "1..2", "v1.2", "-1.2", "00", "01.2.3", "1.2beta", "1.2.beta",
            "١٠",
        ] {
            assert!(
                IntDotVersion::from_str(invalid).is_err(),
                "expected {invalid:?} to be rejected by strict parsing"
            );
        }
    }

    #[test]
    fn standard_vers_string_rejects_non_canonical_version() {
        let result: Result<DynamicVersionRange, VersError> = "vers:intdot/01.2".parse();
        assert!(matches!(
            result,
            Err(VersError::InvalidVersionFormat(_, _, _))
        ));
    }

    // --- Lenient (native) parsing --------------------------------------------

    #[test]
    fn native_parsing_strips_leading_zeros_per_segment() {
        assert_eq!(parse_lenient("01.002.0003").unwrap().to_string(), "1.2.3");
        assert_eq!(parse_lenient("01.000.0003").unwrap().to_string(), "1.0.3");
        assert_eq!(parse_lenient("00.000.0003").unwrap().to_string(), "0.0.3");
    }

    #[test]
    fn native_parsing_preserves_trailing_zero_segments() {
        // Unlike leading zeros, an explicit trailing zero segment is not
        // stripped by canonicalization; it's a valid spelling on its own.
        assert_eq!(parse_lenient("1.2.0").unwrap().to_string(), "1.2.0");
        assert_eq!(parse_lenient("01.2.0").unwrap().to_string(), "1.2.0");
    }

    #[test]
    fn native_parsing_ignores_suffix_after_first_non_digit_non_dot_char() {
        assert_eq!(parse_lenient("1.2beta").unwrap().to_string(), "1.2");
        assert_eq!(parse_lenient("1.10alpha").unwrap().to_string(), "1.10");
        assert_eq!(parse_lenient("1.3rc1").unwrap().to_string(), "1.3");
        assert_eq!(parse_lenient("10e2").unwrap().to_string(), "10");
    }

    #[test]
    fn native_parsing_trailing_dot_and_double_dot_stop_interpretation() {
        assert_eq!(parse_lenient("1.2.").unwrap().to_string(), "1.2");
        assert_eq!(parse_lenient("1..2").unwrap().to_string(), "1");
        assert_eq!(parse_lenient("1.2.beta").unwrap().to_string(), "1.2");
    }

    #[test]
    fn native_parsing_rejects_structurally_invalid_versions() {
        for invalid in ["", "beta", ".1", "v1.2", "-1.2"] {
            assert!(
                parse_lenient(invalid).is_err(),
                "expected {invalid:?} to be rejected"
            );
        }
    }

    #[test]
    fn native_parsing_non_ascii_digits_are_not_accepted_as_digits() {
        // "١٠" is Arabic-Indic for "10"; it must not be treated as a digit.
        assert!(parse_lenient("١٠").is_err());
        // The numeric prefix stops before non-ASCII characters, same as any
        // other non-digit, non-dot byte.
        assert_eq!(parse_lenient("1.٢").unwrap().to_string(), "1");
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
            parse_lenient("01.002.0003").unwrap(),
            IntDotVersion::from_str("1.2.3").unwrap()
        );
    }

    #[test]
    fn missing_trailing_segment_is_equivalent_to_zero_segment() {
        // Both spellings are individually valid canonical forms, so this
        // works through strict parsing alone.
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
            parse_lenient("1.2beta").unwrap(),
            parse_lenient("1.2rc1").unwrap()
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
            .map(|s| parse_lenient(s).unwrap())
            .collect();
        versions.sort();
        let sorted: Vec<String> = versions.iter().map(ToString::to_string).collect();
        assert_eq!(sorted, vec!["1.0.6", "1.2.2", "1.2.10", "1.2.11"]);
    }

    #[test]
    fn suffixes_are_ignored_for_comparison() {
        let mut versions: Vec<IntDotVersion> = ["1.2beta", "1.10alpha", "1.3rc1"]
            .into_iter()
            .map(|s| parse_lenient(s).unwrap())
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
        let parsed: Vec<IntDotVersion> =
            versions.iter().map(|s| parse_lenient(s).unwrap()).collect();
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
        // Unlike the range itself (which must be in canonical `vers:` form),
        // the tested version passed to `contains` is parsed leniently:
        // leading zeros and an ignored suffix are tolerated and normalized,
        // not rejected.
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
    fn typed_serde_rejects_non_canonical_leading_zero() {
        let result: Result<IntDotVersion, _> = serde_json::from_str("\"01.2\"");
        assert!(result.is_err());
    }

    #[test]
    fn typed_serde_rejects_suffixed_version() {
        let result: Result<IntDotVersion, _> = serde_json::from_str("\"1.2beta\"");
        assert!(result.is_err());
    }
}
