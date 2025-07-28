//! Version constraint type for the vers-rs library.
//!
//! This module contains the `VersionConstraint` struct, used to represent version constraints
//! in a version range specifier.
//!
//! The `VersionConstraint` struct represents a single version constraint with a comparator
//! and a version string. It defines a condition that a version must satisfy to be
//! considered within a version range.

use std::fmt::{Debug, Display};
use std::str::FromStr;
use percent_encoding::percent_decode_str;
use crate::{Comparator, VersError};

/// A trait alias for version types that can be used in version constraints and ranges.
pub trait VT: FromStr + Default + Ord + Clone + Display + Debug {}

// Blanket implementation for any type that satisfies the bounds
impl<T> VT for T where T: FromStr + Default + Ord + Clone + Display + Debug {}

/// A single version constraint with a comparator and version.
///
/// A version constraint consists of a comparator (such as =, !=, <, <=, >, >=, or *)
/// and a version string. It defines a condition that a version must satisfy to be
/// considered within a version range.
///
/// Examples:
/// - `1.2.3` (implicit equal)
/// - `>=1.0.0` (greater than or equal)
/// - `<2.0.0` (less than)
/// - `!=1.2.3` (not equal)
/// - `*` (any version)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionConstraint<V : VT> {
    /// The comparator for this constraint
    pub comparator: Comparator,

    /// The version string for this constraint
    pub version: V,
}

impl<V : VT> VersionConstraint<V> {
    /// Create a new version constraint with the given comparator and version.
    ///
    /// # Arguments
    ///
    /// * `comparator` - The comparator to use for this constraint
    /// * `version` - The version string for this constraint
    ///
    /// # Returns
    ///
    /// A new `VersionConstraint` instance
    pub fn new(comparator: Comparator, version: V) -> Self {
        Self { comparator, version }
    }

    /// Parse a version constraint string into a `VersionConstraint`.
    ///
    /// This function parses a string like ">=1.0.0" into a `VersionConstraint`
    /// with the appropriate comparator and version.
    ///
    /// # Arguments
    ///
    /// * `constraint_str` - The constraint string to parse
    ///
    /// # Returns
    ///
    /// A `Result` containing either the parsed `VersionConstraint` or an error
    ///
    /// # Examples
    ///
    /// ```
    /// use vers_rs::schemes::semver::SemVer;
    /// use vers_rs::VersionConstraint;
    ///
    /// let constraint: VersionConstraint<SemVer> = VersionConstraint::parse(">=1.0.0").unwrap();
    /// assert_eq!(constraint.comparator.to_string(), ">=");
    /// assert_eq!(constraint.version, "1.0.0".parse().unwrap());
    /// ```
    pub fn parse(constraint_str: &str) -> Result<Self, VersError> {
        if constraint_str.is_empty() {
            return Err(VersError::InvalidConstraint("Empty constraint".to_string()));
        }

        if constraint_str == "*" {
            return Ok(Self {
                comparator: Comparator::Any,
                version: V::default(),
            });
        }

        let (comparator, version) = if constraint_str.starts_with(">=") {
            (Comparator::GreaterThanOrEqual, &constraint_str[2..])
        } else if constraint_str.starts_with("<=") {
            (Comparator::LessThanOrEqual, &constraint_str[2..])
        } else if constraint_str.starts_with("!=") {
            (Comparator::NotEqual, &constraint_str[2..])
        } else if constraint_str.starts_with('>') {
            (Comparator::GreaterThan, &constraint_str[1..])
        } else if constraint_str.starts_with('<') {
            (Comparator::LessThan, &constraint_str[1..])
        } else {
            (Comparator::Equal, constraint_str)
        };

        let version = version.trim();
        if version.is_empty() && comparator != Comparator::Any {
            return Err(VersError::InvalidConstraint("Missing version".to_string()));
        }

        // Handle URL percent encoding if needed
        let version_str = if version.contains('%') {
            match percent_decode_str(version).decode_utf8() {
                Ok(decoded) => decoded.to_string(),
                Err(_) => return Err(VersError::InvalidConstraint(format!("Invalid URL encoding: {}", version))),
            }
        } else {
            version.to_string()
        };

        let parsed_version = version_str.parse::<V>()
            .map_err(|_| VersError::InvalidConstraint(format!("Failed to parse version: {}", version_str)))?;

        Ok(Self { comparator, version: parsed_version })
    }
}