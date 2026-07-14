//! Version constraint type for the vers-rs library.
//!
//! This module contains the `VersionConstraint` struct, used to represent version constraints
//! in a version range specifier.
//!
//! The `VersionConstraint` struct represents a single version constraint with a comparator
//! and a version string. It defines a condition that a version must satisfy to be
//! considered within a version range.

use crate::{Comparator, VersError};
use crate::VersVersionRange;
use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};
use std::str::FromStr;

/// Trait for version types that support native (scheme-specific) syntax.
///
/// Some versioning schemes define their own syntax that differs from the standard
/// vers pipe-delimited format. For example:
/// - Debian defines `>>` (strictly greater) and `<<` (strictly less)
/// - Future schemes may define interval notation like `[1.0;2.0)` meaning `>=1.0|<2.0`
///
/// This trait provides three entry points:
/// - `from_native_string`: parses a native range string into a full `VersVersionRange`
/// - `from_native`: parses a full native range string into vers constraints
/// - `from_native_constraint`: parses a single native constraint into a vers constraint
///
/// The default `from_native` splits on `|` and delegates to `from_native_constraint`
/// for each segment. Schemes with entirely different range syntax can override
/// `from_native` directly.
pub trait NativeVersionConverter: VersionType {
    /// The vers scheme identifier for this version type (e.g. `"deb"`, `"semver"`).
    const SCHEME_NAME: &'static str;

    /// Parse a native range string into a fully parsed `VersVersionRange`.
    ///
    /// This is the main entry point for converting native syntax into vers ranges.
    /// The default implementation calls [`Self::from_native`] and wraps
    /// the result with [`Self::SCHEME_NAME`]. Schemes whose native syntax
    /// requires special handling can override this directly.
    fn from_native_string(raw: &str) -> Result<VersVersionRange<Self>, VersError> {
        Ok(VersVersionRange::new(
            Self::SCHEME_NAME.to_string(),
            Self::from_native(raw)?,
        ))
    }

    /// Parse a full native range string into one or more standard `VersionConstraint`s.
    ///
    /// This is called by `VersVersionRange::from_str` (for the `vers:scheme/...` format)
    /// and by [`Self::from_native_string`] (for bare native strings). It receives
    /// the entire constraint portion of the vers string (after the scheme prefix).
    ///
    /// The default implementation splits on `|` and calls [`Self::from_native_constraint`] for
    /// each segment. Schemes whose native syntax doesn't use `|` as a delimiter
    /// should override this method.
    fn from_native(raw: &str) -> Result<Vec<VersionConstraint<Self>>, VersError> {
        let segments: Vec<&str> = raw
            .trim_matches('|')
            .split('|')
            .filter(|s| !s.is_empty())
            .collect();

        if segments.is_empty() {
            return Err(VersError::EmptyConstraints);
        }

        segments.iter().map(|s| Self::from_native_constraint(s)).collect()
    }

    /// Parse a single native constraint string into one or more `VersionConstraint`s.
    ///
    /// The default implementation delegates to the standard vers constraint parser,
    /// assuming a single constraint. Schemes with native operators (e.g. `<<`, `>>`)
    /// override this to handle their own syntax.
    fn from_native_constraint(raw: &str) -> Result<VersionConstraint<Self>, VersError> {
        VersionConstraint::<Self>::parse(raw)
    }
}

/// A trait alias for version types that can be used in version constraints and ranges.
pub trait VersionType:
    FromStr + Default + Ord + PartialOrd + Clone + Display + Debug + Serialize
{
}

/// Blanket implementation for any type that satisfies the bounds
impl<T> VersionType for T where
    T: FromStr + Default + Ord + PartialOrd + Clone + Display + Debug + Serialize
{
}

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct VersionConstraint<V: VersionType> {
    /// The comparator for this constraint
    pub comparator: Comparator,

    /// The version string for this constraint
    pub version: V,
}

impl<V: VersionType> VersionConstraint<V> {
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
        Self {
            comparator,
            version,
        }
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

        let (comparator, version) = if let Some(stripped) = constraint_str.strip_prefix(">=") {
            (Comparator::GreaterThanOrEqual, stripped)
        } else if let Some(stripped) = constraint_str.strip_prefix("<=") {
            (Comparator::LessThanOrEqual, stripped)
        } else if let Some(stripped) = constraint_str.strip_prefix("!=") {
            (Comparator::NotEqual, stripped)
        } else if let Some(stripped) = constraint_str.strip_prefix('>') {
            (Comparator::GreaterThan, stripped)
        } else if let Some(stripped) = constraint_str.strip_prefix('<') {
            (Comparator::LessThan, stripped)
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
                Err(_) => {
                    return Err(VersError::InvalidConstraint(format!(
                        "Invalid URL encoding: {}",
                        version
                    )));
                }
            }
        } else {
            version.to_string()
        };

        let parsed_version = version_str.parse::<V>().map_err(|_| {
            VersError::InvalidConstraint(format!("Failed to parse version: {}", version_str))
        })?;

        Ok(Self {
            comparator,
            version: parsed_version,
        })
    }
}
