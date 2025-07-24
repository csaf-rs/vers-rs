//! # vers-rs
//!
//! A Rust library for parsing, validating, and checking version range specifiers.
//!
//! This library implements the version range specifier (vers) format as described in
//! the [VERSION-RANGE-SPEC](https://github.com/package-url/purl-spec/blob/main/VERSION-RANGE-SPEC.rst).
//!
//! ## Usage
//!
//! ```rust
//! use vers_rs::schemes::semver::SemVer;
//! use vers_rs::{parse, contains, VersionRange};
//!
//! // Parse a version range specifier
//! let range: VersionRange<SemVer> = parse("vers:npm/>=1.0.0|<2.0.0").unwrap();
//!
//! // Check if a version is within the range
//! assert!(contains(&range, &"1.5.0".parse().unwrap()).unwrap());
//! assert!(!contains(&range, &"2.0.0".parse().unwrap()).unwrap());
//! ```
//!
//! ## Features
//!
//! - Parse version range specifiers in the format `vers:<versioning-scheme>/<version-constraint>|<version-constraint>|...`
//! - Validate version range specifiers according to the rules in the specification
//! - Normalize and simplify version range specifiers
//! - Check if a version is within a specified range
//! - Support for different versioning schemes (npm/semver, pypi, maven, deb, etc.)
//!
//! ## TODO: Future Improvements
//!
//! - **Version Comparison**: Implement proper version comparison for different versioning schemes:
//!   - PEP440 for Python/PyPI
//!   - Maven versioning rules
//!   - Debian versioning rules
//!   - RubyGems versioning rules
//!
//! - **Normalization**: Improve the normalization algorithm:
//!   - Use proper version comparison for sorting
//!   - Handle more edge cases
//!   - Optimize for better performance
//!
//! - **Validation**: Enhance validation:
//!   - Validate version formats for different versioning schemes
//!   - Add more detailed error messages
//!   - Make sort order validation a hard requirement
//!
//! - **Error Handling**: Improve error handling:
//!   - Add more specific error types
//!   - Provide more context in error messages
//!   - Consider returning errors for unknown versioning schemes
//!

// Module declarations
mod error;
mod comparator;
mod range;
mod constraint;
pub mod schemes;

pub use comparator::Comparator;
pub use constraint::VersionConstraint;
// Re-exports
pub use error::VersError;
pub use range::VersionRange;
use crate::constraint::VT;

/// Parse a version range specifier string into a `VersionRange`.
///
/// This function parses a string like "vers:npm/>=1.0.0|<2.0.0" into a `VersionRange`
/// with the appropriate versioning scheme and constraints.
///
/// # Arguments
///
/// * `s` - The version range specifier string to parse
///
/// # Returns
///
/// A `Result` containing either the parsed `VersionRange` or an error
///
/// # Examples
///
/// ```
/// use vers_rs::{parse, VersionRange};
/// use vers_rs::schemes::semver::SemVer;
///
/// let range: VersionRange<SemVer> = parse("vers:npm/>=1.0.0|<2.0.0").unwrap();
/// assert_eq!(range.versioning_scheme, "npm");
/// assert_eq!(range.constraints.len(), 2);
/// ```
pub fn parse<V : VT>(s: &str) -> Result<VersionRange<V>, VersError> {
    s.parse()
}

/// Check if a version is contained within a version range.
///
/// This function checks if a version string satisfies the constraints defined
/// in a version range.
///
/// # Arguments
///
/// * `range` - The version range to check against
/// * `version` - The version string to check
///
/// # Returns
///
/// A `Result` containing a boolean indicating whether the version is in the range
///
/// # Examples
///
/// ```
/// use semver::Version;
/// use vers_rs::{parse, contains, VersionRange};
/// use vers_rs::schemes::semver::SemVer;
///
/// let range: VersionRange<SemVer> = "vers:npm/>=1.0.0|<2.0.0".parse().unwrap();
/// assert!(contains(&range, &"1.5.0".parse().unwrap()).unwrap());
/// assert!(!contains(&range, &"2.0.0".parse().unwrap()).unwrap());
/// ```
pub fn contains<V : VT>(range: &VersionRange<V>, version: &V) -> Result<bool, VersError> {
    range.contains(version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schemes::semver::SemVer;

    #[test]
    fn test_parse_simple() {
        let range: VersionRange<SemVer> = "vers:npm/1.2.3".parse().unwrap();
        assert_eq!(range.versioning_scheme, "npm");
        assert_eq!(range.constraints.len(), 1);
        assert_eq!(range.constraints[0].comparator, Comparator::Equal);
        assert_eq!(range.constraints[0].version, "1.2.3".parse().unwrap());
    }
    
    #[test]
    fn test_parse_with_comparators() {
        let range: VersionRange<SemVer> = "vers:npm/>=1.0.0|<2.0.0".parse().unwrap();
        assert_eq!(range.versioning_scheme, "npm");
        assert_eq!(range.constraints.len(), 2);
        assert_eq!(range.constraints[0].comparator, Comparator::GreaterThanOrEqual);
        assert_eq!(range.constraints[0].version, "1.0.0".parse().unwrap());
        assert_eq!(range.constraints[1].comparator, Comparator::LessThan);
        assert_eq!(range.constraints[1].version, "2.0.0".parse().unwrap());
    }
    
    #[test]
    fn test_parse_star() {
        let range: VersionRange<SemVer> = parse("vers:npm/*").unwrap();
        assert_eq!(range.versioning_scheme, "npm");
        assert_eq!(range.constraints.len(), 1);
        assert_eq!(range.constraints[0].comparator, Comparator::Any);
        assert_eq!(range.constraints[0].version, "0.0.0".parse().unwrap());
    }
    
    #[test]
    fn test_parse_with_spaces() {
        let range: VersionRange<SemVer> = parse("vers:npm/ >= 1.0.0 | < 2.0.0 ").unwrap();
        assert_eq!(range.versioning_scheme, "npm");
        assert_eq!(range.constraints.len(), 2);
        assert_eq!(range.constraints[0].comparator, Comparator::GreaterThanOrEqual);
        assert_eq!(range.constraints[0].version, "1.0.0".parse().unwrap());
        assert_eq!(range.constraints[1].comparator, Comparator::LessThan);
        assert_eq!(range.constraints[1].version, "2.0.0".parse().unwrap());
    }
    
    #[test]
    fn test_parse_with_url_encoding() {
        // Test with a version that contains characters that need URL encoding
        let range: VersionRange<SemVer> = parse("vers:npm/1.0.0%2Bbuild.1").unwrap();
        assert_eq!(range.versioning_scheme, "npm");
        assert_eq!(range.constraints.len(), 1);
        assert_eq!(range.constraints[0].comparator, Comparator::Equal);
        assert_eq!(range.constraints[0].version, "1.0.0+build.1".parse().unwrap());
    }
    
    #[test]
    fn test_invalid_scheme() {
        let result: Result<VersionRange<SemVer>, _> = parse("foo:npm/1.2.3");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), VersError::InvalidScheme);
    }
    
    #[test]
    fn test_missing_scheme() {
        let result: Result<VersionRange<SemVer>, _> = parse("vers:/1.2.3");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), VersError::MissingVersioningScheme);
    }
    
    #[test]
    fn test_empty_constraints() {
        let result: Result<VersionRange<SemVer>, _> = parse("vers:npm/");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), VersError::EmptyConstraints);
    }
    
    #[test]
    fn test_duplicate_version() {
        let result: Result<VersionRange<SemVer>, _> = parse("vers:npm/1.2.3|1.2.3");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), VersError::DuplicateVersion(_)));
    }
    
    #[test]
    fn test_invalid_constraint_simplification() {
        let result: VersionRange<SemVer> = parse("vers:npm/1.2.3|<2.0.0").unwrap();
        assert_eq!(result.to_string(), "vers:npm/<2.0.0");

        let result: VersionRange<SemVer> = parse("vers:npm/>1.0.0|>2.0.0").unwrap();
        assert_eq!(result.to_string(), "vers:npm/>1.0.0");
        
        let result: VersionRange<SemVer> = parse("vers:npm/<1.0.0|<2.0.0").unwrap();
        assert_eq!(result.to_string(), "vers:npm/<2.0.0");
    }
    
    #[test]
    fn test_normalize() {
        // Test normalization of redundant constraints
        let mut range = VersionRange::<SemVer>::new(
            "npm".to_string(),
            vec![
                VersionConstraint::new(Comparator::GreaterThanOrEqual, "1.0.0".parse().unwrap()),
                VersionConstraint::new(Comparator::GreaterThan, "1.5.0".parse().unwrap()),
                VersionConstraint::new(Comparator::LessThan, "3.0.0".parse().unwrap()),
                VersionConstraint::new(Comparator::LessThanOrEqual, "2.0.0".parse().unwrap()),
            ]
        );
        
        // After normalization, validate should pass
        match range.normalize_and_validate() {
            Ok(_) => {} ,
            Err(e) => panic!("{}", e),
        }
        
        // Check that redundant constraints were removed
        assert_eq!(range.constraints.len(), 2);
        assert_eq!(range.constraints[0].comparator, Comparator::GreaterThanOrEqual);
        assert_eq!(range.constraints[0].version, "1.0.0".parse().unwrap());
        assert_eq!(range.constraints[1].comparator, Comparator::LessThan);
        assert_eq!(range.constraints[1].version, "3.0.0".parse().unwrap());
    }
    
    #[test]
    fn test_contains_simple() {
        let range: VersionRange<SemVer> = parse("vers:npm/1.2.3").unwrap();
        assert!(contains(&range, &"1.2.3".parse().unwrap()).unwrap());
        assert!(!contains(&range, &"1.2.4".parse().unwrap()).unwrap());
    }
    
    #[test]
    fn test_contains_range() {
        let range: VersionRange<SemVer> = parse("vers:npm/>=1.0.0|<2.0.0").unwrap();
        assert!(contains(&range, &"1.0.0".parse().unwrap()).unwrap());
        assert!(contains(&range, &"1.5.0".parse().unwrap()).unwrap());
        assert!(!contains(&range, &"2.0.0".parse().unwrap()).unwrap());
        assert!(!contains(&range, &"0.9.0".parse().unwrap()).unwrap());
    }
    
    #[test]
    fn test_contains_star() {
        let range: VersionRange<SemVer> = parse("vers:npm/*").unwrap();
        assert!(contains(&range, &"1.0.0".parse().unwrap()).unwrap());
        assert!(contains(&range, &"2.0.0".parse().unwrap()).unwrap());
        assert!(contains(&range, &"0.0.1".parse().unwrap()).unwrap());
    }
    
    #[test]
    fn test_contains_not_equal() {
        let range: VersionRange<SemVer> = parse("vers:npm/!=1.2.3").unwrap();
        assert!(!contains(&range, &"1.2.3".parse().unwrap()).unwrap());
        assert!(contains(&range, &"1.2.4".parse().unwrap()).unwrap());
    }
    
    #[test]
    fn test_contains_complex() {
        // Test a complex range with multiple constraints
        let range: VersionRange<SemVer> = parse("vers:npm/>=1.0.0|<2.0.0|!=1.5.0").unwrap();
        assert!(contains(&range, &"1.0.0".parse().unwrap()).unwrap());
        assert!(contains(&range, &"1.7.0".parse().unwrap()).unwrap());
        assert!(!contains(&range, &"1.5.0".parse().unwrap()).unwrap());
        assert!(!contains(&range, &"2.0.0".parse().unwrap()).unwrap());
        assert!(!contains(&range, &"0.9.0".parse().unwrap()).unwrap());
    }
    
    #[test]
    fn test_display() {
        // Test that the Display implementation produces the correct string
        let range: VersionRange<SemVer> = parse("vers:npm/>=1.0.0|<2.0.0").unwrap();
        assert_eq!(range.to_string(), "vers:npm/>=1.0.0|<2.0.0");
        
        let range: VersionRange<SemVer> = parse("vers:npm/*").unwrap();
        assert_eq!(range.to_string(), "vers:npm/*");
        
        let range: VersionRange<SemVer> = parse("vers:npm/1.2.3").unwrap();
        assert_eq!(range.to_string(), "vers:npm/1.2.3");
    }
}