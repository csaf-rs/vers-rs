//! Helpers that run the generated VERS test-suite cases against vers-rs.
//!
//! The generated cases call these helpers with the data from the `vers-spec`
//! test files. Every helper panics on a mismatch.

use std::cmp::Ordering;
use std::fmt::Debug;
use std::str::FromStr;

use vers_rs::range::VersionRange;
use vers_rs::schemes::cargo::CargoVersion;
use vers_rs::schemes::deb::DebVersion;
use vers_rs::schemes::intdot::IntDotVersion;
use vers_rs::schemes::semver::SemVer;
use vers_rs::{Comparator, VersionConstraint, parse, parse_native};

fn comparator(operator: &str) -> Comparator {
    match operator {
        "" | "=" => Comparator::Equal,
        "!=" => Comparator::NotEqual,
        "<" => Comparator::LessThan,
        "<=" => Comparator::LessThanOrEqual,
        ">" => Comparator::GreaterThan,
        ">=" => Comparator::GreaterThanOrEqual,
        "*" => Comparator::Any,
        _ => panic!("unknown comparator `{operator}`"),
    }
}

fn unsupported(scheme: &str) -> ! {
    panic!("vers-rs does not implement the `{scheme}` versioning scheme")
}

pub fn parse_ok(input: &str, scheme: &str, constraints: &[(&str, &str)]) {
    let range = parse(input).unwrap_or_else(|e| panic!("`{input}` should parse: {e}"));
    assert_eq!(range.versioning_scheme(), scheme);
    let expected: Vec<VersionConstraint<String>> = constraints
        .iter()
        .map(|(operator, version)| {
            VersionConstraint::new(comparator(operator), version.to_string())
        })
        .collect();
    assert_eq!(range.constraints(), &expected);
}

pub fn parse_err(input: &str) {
    assert!(parse(input).is_err(), "`{input}` should fail to parse");
}

/// `FromStr` never normalizes, so a canonical input must round-trip unchanged
/// and a non-canonical input must be rejected instead of being rewritten.
pub fn validate(input: &str, expected: &str) {
    if input == expected {
        let range = parse(input).unwrap_or_else(|e| panic!("`{input}` should parse: {e}"));
        assert_eq!(range.to_string(), expected);
    } else {
        assert!(
            parse(input).is_err(),
            "non-canonical `{input}` (canonical: `{expected}`) should be rejected, not normalized"
        );
    }
}

pub fn from_native_ok(scheme: &str, native: &str, expected: &str) {
    let range =
        parse_native(scheme, native).unwrap_or_else(|e| panic!("`{native}` should convert: {e}"));
    assert_eq!(range.to_string(), expected);
}

pub fn from_native_err(scheme: &str, native: &str) {
    assert!(
        parse_native(scheme, native).is_err(),
        "`{native}` should fail to convert"
    );
}

pub fn containment(vers: &str, version: &str, expected: bool) {
    let range = parse(vers).unwrap_or_else(|e| panic!("`{vers}` should parse: {e}"));
    let actual = range
        .contains(version.to_string())
        .unwrap_or_else(|e| panic!("`{version}` should be checkable: {e}"));
    assert_eq!(actual, expected);
}

pub fn sorted(scheme: &str, versions: &[&str], expected: &[&str]) {
    let actual = match scheme {
        "semver" | "npm" => sort::<SemVer>(versions),
        "deb" => sort::<DebVersion>(versions),
        "cargo" => sort::<CargoVersion>(versions),
        "intdot" => sort::<IntDotVersion>(versions),
        _ => unsupported(scheme),
    };
    assert_eq!(actual, expected);
}

pub fn equal(scheme: &str, a: &str, b: &str, expected: bool) {
    let actual = match scheme {
        "semver" | "npm" => same::<SemVer>(a, b),
        "deb" => same::<DebVersion>(a, b),
        "cargo" => same::<CargoVersion>(a, b),
        "intdot" => same::<IntDotVersion>(a, b),
        _ => unsupported(scheme),
    };
    assert_eq!(actual, expected);
}

fn parse_version<V>(version: &str) -> V
where
    V: FromStr,
    V::Err: Debug,
{
    version
        .parse()
        .unwrap_or_else(|e| panic!("`{version}` is not a valid version: {e:?}"))
}

/// Sorts the original version strings by their parsed value.
///
/// The strings are returned exactly as given: `FromStr` is only used to get a
/// sort key and its output is never printed back, so a normalizing parser
/// cannot make a test pass by rewriting the expected text.
fn sort<V>(versions: &[&str]) -> Vec<String>
where
    V: FromStr + Ord,
    V::Err: Debug,
{
    let mut keyed: Vec<(V, &str)> = versions.iter().map(|v| (parse_version(v), *v)).collect();
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    keyed.into_iter().map(|(_, v)| v.to_string()).collect()
}

fn same<V>(a: &str, b: &str) -> bool
where
    V: FromStr + Ord,
    V::Err: Debug,
{
    parse_version::<V>(a).cmp(&parse_version::<V>(b)) == Ordering::Equal
}
