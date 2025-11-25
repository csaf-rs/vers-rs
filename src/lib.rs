// Module declarations
pub mod comparator;
pub mod constraint;
pub mod error;
pub mod range;
pub mod schemes;

pub use comparator::Comparator;
pub use constraint::VersionConstraint;
pub use error::VersError;
pub use range::dynamic::DynamicVersionRange;
pub use range::generic::GenericVersionRange;
pub use range::VersionRange;

#[cfg(feature = "wasm")]
use serde_wasm_bindgen;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

/// Parse a version range specifier string into a `DynamicVersionRange`.
///
/// This function automatically detects the versioning scheme and constructs
/// the appropriate typed version range.
///
/// # Arguments
///
/// * `s` - The version range specifier string to parse
///
/// # Returns
///
/// A `Result` containing either the parsed `DynamicVersionRange` or an error
///
/// # Examples
///
/// ```
/// use vers_rs::parse;
/// use vers_rs::range::VersionRange;
///
/// let range = parse("vers:npm/>=1.0.0|<2.0.0").unwrap();
/// assert_eq!(range.versioning_scheme(), "npm");
/// assert_eq!(range.constraints().len(), 2);
/// ```
pub fn parse(s: &str) -> Result<DynamicVersionRange, VersError> {
    s.parse()
}

/// WebAssembly-friendly wrapper around `parse`.
///
/// Parse and return a structured JS object describing the parsed range.
/// On success returns an object { versioning_scheme, constraints: [{comparator, version}, ...] }
#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub fn parse_js(s: &str) -> Result<JsValue, JsValue> {
    match parse(s) {
        Ok(range) => {
            serde_wasm_bindgen::to_value(&range).map_err(|e| JsValue::from_str(&e.to_string()))
        }
        Err(e) => Err(JsValue::from(e)),
    }
}

/// Check if a version string is contained within a dynamic version range.
///
/// This function checks if a version string satisfies the constraints defined
/// in a dynamic version range, automatically handling version parsing.
///
/// # Arguments
///
/// * `range` - The dynamic version range to check against
/// * `version_str` - The version string to check
///
/// # Returns
///
/// A `Result` containing a boolean indicating whether the version is in the range
///
/// # Examples
///
/// ```
/// use vers_rs::{parse, contains};
///
/// let range = parse("vers:npm/>=1.0.0|<2.0.0").unwrap();
/// assert!(contains(&range, "1.5.0".to_string()).unwrap());
/// assert!(!contains(&range, "2.0.0".to_string()).unwrap());
/// ```
pub fn contains(range: &DynamicVersionRange, version_str: String) -> Result<bool, VersError> {
    range.contains(version_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schemes::semver::SemVer;
    use crate::VersError;

    #[test]
    fn test_parse_simple() {
        let range: DynamicVersionRange = "vers:npm/1.2.3".parse().unwrap();
        assert_eq!(range.versioning_scheme(), "npm");
        assert_eq!(range.constraints().len(), 1);
        assert_eq!(range.constraints()[0].comparator, Comparator::Equal);
        assert_eq!(range.constraints()[0].version.to_string(), "1.2.3");
    }

    #[test]
    fn test_parse_with_comparators() {
        let range: DynamicVersionRange = "vers:npm/>=1.0.0|<2.0.0".parse().unwrap();
        assert_eq!(range.versioning_scheme(), "npm");
        assert_eq!(range.constraints().len(), 2);
        assert_eq!(
            range.constraints()[0].comparator,
            Comparator::GreaterThanOrEqual
        );
        assert_eq!(range.constraints()[0].version.to_string(), "1.0.0");
        assert_eq!(range.constraints()[1].comparator, Comparator::LessThan);
        assert_eq!(range.constraints()[1].version.to_string(), "2.0.0");
    }

    #[test]
    fn test_parse_star() {
        let range: DynamicVersionRange = parse("vers:npm/*").unwrap();
        assert_eq!(range.versioning_scheme(), "npm");
        assert_eq!(range.constraints().len(), 1);
        assert_eq!(range.constraints()[0].comparator, Comparator::Any);
        assert_eq!(range.constraints()[0].version.to_string(), "0.0.0");
    }

    #[test]
    fn test_parse_with_spaces() {
        let range: DynamicVersionRange = parse("vers:npm/ >= 1.0.0 | < 2.0.0 ").unwrap();
        assert_eq!(range.versioning_scheme(), "npm");
        assert_eq!(range.constraints().len(), 2);
        assert_eq!(
            range.constraints()[0].comparator,
            Comparator::GreaterThanOrEqual
        );
        assert_eq!(range.constraints()[0].version.to_string(), "1.0.0");
        assert_eq!(range.constraints()[1].comparator, Comparator::LessThan);
        assert_eq!(range.constraints()[1].version.to_string(), "2.0.0");
    }

    #[test]
    fn test_parse_with_url_encoding() {
        // Test with a version that contains characters that need URL encoding
        let range: DynamicVersionRange = parse("vers:npm/1.0.0%2Bbuild.1").unwrap();
        assert_eq!(range.versioning_scheme(), "npm");
        assert_eq!(range.constraints().len(), 1);
        assert_eq!(range.constraints()[0].comparator, Comparator::Equal);
        assert_eq!(range.constraints()[0].version.to_string(), "1.0.0+build.1");
    }

    #[test]
    fn test_invalid_scheme() {
        let result: Result<GenericVersionRange<SemVer>, _> = "foo:npm/1.2.3".parse();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), VersError::InvalidScheme);
    }

    #[test]
    fn test_missing_scheme() {
        let result: Result<GenericVersionRange<SemVer>, _> = "vers:/1.2.3".parse();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), VersError::MissingVersioningScheme);
    }

    #[test]
    fn test_empty_constraints() {
        let result: Result<GenericVersionRange<SemVer>, _> = "vers:npm/".parse();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), VersError::EmptyConstraints);
    }

    #[test]
    fn test_duplicate_version() {
        let result: Result<GenericVersionRange<SemVer>, _> = "vers:npm/1.2.3|1.2.3".parse();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            VersError::DuplicateVersion(_)
        ));
    }

    #[test]
    fn test_invalid_constraint_simplification() {
        let result: DynamicVersionRange = parse("vers:npm/1.2.3|<2.0.0").unwrap();
        assert_eq!(result.to_string(), "vers:npm/<2.0.0");

        let result: DynamicVersionRange = parse("vers:npm/>1.0.0|>2.0.0").unwrap();
        assert_eq!(result.to_string(), "vers:npm/>1.0.0");

        let result: DynamicVersionRange = parse("vers:npm/<1.0.0|<2.0.0").unwrap();
        assert_eq!(result.to_string(), "vers:npm/<2.0.0");
    }

    #[test]
    fn test_normalize() {
        // Test normalization of redundant constraints
        let mut range = GenericVersionRange::<SemVer>::new(
            "npm".to_string(),
            vec![
                VersionConstraint::new(Comparator::GreaterThanOrEqual, "1.0.0".parse().unwrap()),
                VersionConstraint::new(Comparator::GreaterThan, "1.5.0".parse().unwrap()),
                VersionConstraint::new(Comparator::LessThan, "3.0.0".parse().unwrap()),
                VersionConstraint::new(Comparator::LessThanOrEqual, "2.0.0".parse().unwrap()),
            ],
        );

        // After normalization, validate should pass
        match range.normalize_and_validate() {
            Ok(_) => {}
            Err(e) => panic!("{}", e),
        }

        // Check that redundant constraints were removed
        assert_eq!(range.constraints().len(), 2);
        assert_eq!(
            range.constraints()[0].comparator,
            Comparator::GreaterThanOrEqual
        );
        assert_eq!(range.constraints()[0].version.to_string(), "1.0.0");
        assert_eq!(range.constraints()[1].comparator, Comparator::LessThan);
        assert_eq!(range.constraints()[1].version.to_string(), "3.0.0");
    }

    #[test]
    fn test_contains_simple() {
        let range: DynamicVersionRange = parse("vers:npm/1.2.3").unwrap();
        assert!(contains(&range, "1.2.3".to_string()).unwrap());
        assert!(!contains(&range, "1.2.4".to_string()).unwrap());
    }

    #[test]
    fn test_contains_range() {
        let range: DynamicVersionRange = parse("vers:npm/>=1.0.0|<2.0.0").unwrap();
        assert!(contains(&range, "1.0.0".to_string()).unwrap());
        assert!(contains(&range, "1.5.0".to_string()).unwrap());
        assert!(!contains(&range, "2.0.0".to_string()).unwrap());
        assert!(!contains(&range, "0.9.0".to_string()).unwrap());
    }

    #[test]
    fn test_contains_star() {
        let range: DynamicVersionRange = parse("vers:npm/*").unwrap();
        assert!(contains(&range, "1.0.0".to_string()).unwrap());
        assert!(contains(&range, "2.0.0".to_string()).unwrap());
        assert!(contains(&range, "0.0.1".to_string()).unwrap());
    }

    #[test]
    fn test_contains_not_equal() {
        let range: DynamicVersionRange = parse("vers:npm/!=1.2.3").unwrap();
        assert!(!contains(&range, "1.2.3".to_string()).unwrap());
        assert!(contains(&range, "1.2.4".to_string()).unwrap());
    }

    #[test]
    fn test_contains_complex() {
        // Test a complex range with multiple constraints
        let range: DynamicVersionRange = parse("vers:npm/>=1.0.0|<2.0.0|!=1.5.0").unwrap();
        assert!(contains(&range, "1.0.0".to_string()).unwrap());
        assert!(contains(&range, "1.7.0".to_string()).unwrap());
        assert!(!contains(&range, "1.5.0".to_string()).unwrap());
        assert!(!contains(&range, "2.0.0".to_string()).unwrap());
        assert!(!contains(&range, "0.9.0".to_string()).unwrap());
    }

    #[test]
    fn test_display() {
        // Test that the Display implementation produces the correct string
        let range: DynamicVersionRange = parse("vers:npm/>=1.0.0|<2.0.0").unwrap();
        assert_eq!(range.to_string(), "vers:npm/>=1.0.0|<2.0.0");

        let range: DynamicVersionRange = parse("vers:npm/*").unwrap();
        assert_eq!(range.to_string(), "vers:npm/*");

        let range: DynamicVersionRange = parse("vers:npm/1.2.3").unwrap();
        assert_eq!(range.to_string(), "vers:npm/1.2.3");
    }

    // Tests for DynamicVersionRange
    #[test]
    fn test_dynamic_parse_npm() {
        let range: DynamicVersionRange = "vers:npm/>=1.0.0|<2.0.0".parse().unwrap();
        assert_eq!(range.versioning_scheme(), "npm");
        assert_eq!(range.constraints().len(), 2);
    }

    #[test]
    fn test_dynamic_parse_semver() {
        let range: DynamicVersionRange = "vers:semver/>=1.0.0|<2.0.0".parse().unwrap();
        assert_eq!(range.versioning_scheme(), "semver");
        assert_eq!(range.constraints().len(), 2);
    }

    #[test]
    fn test_dynamic_parse_deb() {
        let range: DynamicVersionRange = "vers:deb/<<1.0".parse().unwrap();
        assert_eq!(range.versioning_scheme(), "deb");
        assert_eq!(range.constraints().len(), 1);
        assert_eq!(range.constraints()[0].comparator, Comparator::LessThan);
        assert_eq!(range.constraints()[0].version.to_string(), "1.0");
    }

    #[test]
    fn test_deb_version_ordering_basic() {
        let range: DynamicVersionRange = "vers:deb/<<1.0".parse().unwrap();
        assert!(contains(&range, "0.9".to_string()).unwrap());
        assert!(!contains(&range, "1.0".to_string()).unwrap());
    }

    #[test]
    fn test_deb_version_ordering_tilde_and_epoch() {
        // 1.0~beta < 1.0
        let range1: DynamicVersionRange = "vers:deb/<<1.0".parse().unwrap();
        assert!(contains(&range1, "1.0~beta".to_string()).unwrap());

        // 1:1.0 > 2.0 because epoch 1 > 0
        let range2: DynamicVersionRange = "vers:deb/>>2.0".parse().unwrap();
        assert!(contains(&range2, "1:1.0".to_string()).unwrap());
        assert!(!contains(&range2, "2.0".to_string()).unwrap());
    }

    #[test]
    fn test_dynamic_parse_unsupported() {
        let range: Result<DynamicVersionRange, VersError> = "vers:pypi/>=1.0.0|<2.0.0".parse();
        assert!(range.is_err());
        assert!(matches!(
            range.unwrap_err(),
            VersError::UnsupportedVersioningScheme(_)
        ));
    }

    #[test]
    fn test_dynamic_contains() {
        let range: DynamicVersionRange = "vers:npm/>=1.0.0|<2.0.0".parse().unwrap();
        assert!(range.contains("1.5.0".to_string()).unwrap());
        assert!(!range.contains("2.0.0".to_string()).unwrap());
        assert!(!range.contains("0.9.0".to_string()).unwrap());
    }

    #[test]
    fn test_dynamic_contains_invalid_version() {
        let range: DynamicVersionRange = "vers:npm/>=1.0.0|<2.0.0".parse().unwrap();
        let result = range.contains("invalid.version".to_string());
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            VersError::InvalidVersionFormat(..)
        ));
    }

    #[test]
    fn test_dynamic_display() {
        let range: DynamicVersionRange = "vers:npm/>=1.0.0|<2.0.0".parse().unwrap();
        assert_eq!(range.to_string(), "vers:npm/>=1.0.0|<2.0.0");
    }

    #[test]
    fn test_dynamic_equality() {
        let range1: DynamicVersionRange = "vers:npm/>=1.0.0|<2.0.0".parse().unwrap();
        let range2: DynamicVersionRange = "vers:npm/>=1.0.0|<2.0.0".parse().unwrap();
        let range3: DynamicVersionRange = "vers:semver/>=1.0.0|<2.0.0".parse().unwrap();

        assert_eq!(range1, range2);
        // Both should parse to the same SemVer range
        assert_eq!(range1.constraints(), range3.constraints());
    }

    #[test]
    fn test_parse_dynamic_function() {
        let range = parse("vers:npm/>=1.0.0|<2.0.0").unwrap();
        assert_eq!(range.versioning_scheme(), "npm");
        assert_eq!(range.constraints().len(), 2);
    }
}
