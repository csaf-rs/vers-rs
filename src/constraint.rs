//! Version constraint type for the vers-rs library.
//!
//! This module contains the `VersionConstraint` struct, used to represent version constraints
//! in a version range specifier.
//!
//! The `VersionConstraint` struct represents a single version constraint with a comparator
//! and a version string. It defines a condition that a version must satisfy to be
//! considered within a version range.

use crate::{ALLOWED_OPERATORS, VersVersionRange};
use crate::{Comparator, VersError};
use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
use std::hash::Hash;
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
/// The default `from_native` splits on [`Self::NATIVE_SEPARATOR`] (`|` unless
/// overridden) and delegates to `from_native_constraint` for each segment. The
/// default `from_native_constraint` matches a constraint's leading operator
/// token against [`Self::native_operators`] (the standard vers operators
/// unless overridden), falling back to implicit equality when no operator
/// matches, and recognizes [`Self::native_wildcard`] (`*` by default, or
/// `None` to disable wildcards) as matching any version.
///
/// Schemes that only need different separator/operator tokens can override
/// `NATIVE_SEPARATOR`, `native_operators`, and/or `native_wildcard` alone (see
/// [`crate::schemes::deb::DebVersion`] for an example). Schemes with entirely
/// different range syntax (e.g. delegating to an external parser) can instead
/// override `from_native` and/or `from_native_constraint` directly (see
/// [`crate::schemes::cargo::CargoVersion`]).
pub trait NativeVersionConverter: VersionType
where
    Self::Err: Into<VersError>,
{
    /// The vers scheme identifier for this version type (e.g. `"deb"`, `"semver"`).
    const SCHEME_NAME: &'static str;

    /// The character used to separate multiple native constraints within a
    /// single native range string.
    ///
    /// Defaults to `|`, matching vers syntax. Override for schemes whose
    /// native syntax joins constraints with a different character (e.g. `,`
    /// or `;`). Only consulted by the default [`Self::from_native`]
    /// implementation; schemes that override `from_native` directly may
    /// ignore this constant entirely.
    const NATIVE_SEPARATOR: char = '|';

    /// The recognized native comparator operator tokens, paired with the
    /// [`Comparator`] each one represents.
    ///
    /// Tokens are matched in order as string prefixes of a trimmed constraint,
    /// so when one token is a prefix of another (e.g. `>` and `>=`), list the
    /// longer/more specific token first to avoid it being shadowed. A
    /// constraint that matches none of these tokens is treated as an implicit
    /// equality (the whole constraint is parsed as a bare version), matching
    /// standard vers syntax.
    ///
    /// Defaults to the standard vers operators (`>=`, `<=`, `!=`, `>`, `<`)
    /// plus an explicit `=` for equality (which, unlike the canonical vers
    /// string format, native syntax is free to allow). Override to support a
    /// scheme's own operator tokens, such as Debian's `<<`/`>>`.
    fn native_operators() -> &'static [(&'static str, Comparator)] {
        &[
            (">=", Comparator::GreaterThanOrEqual),
            ("<=", Comparator::LessThanOrEqual),
            ("!=", Comparator::NotEqual),
            (">", Comparator::GreaterThan),
            ("<", Comparator::LessThan),
            ("=", Comparator::Equal),
        ]
    }

    /// The token that denotes "any version" (a wildcard constraint), if this
    /// scheme's native syntax supports one.
    ///
    /// Defaults to `Some("*")`, matching standard vers syntax. Override to
    /// return `None` for schemes whose native syntax has no wildcard token
    /// (e.g. Debian version relations always require an explicit operator and
    /// version).
    fn native_wildcard() -> Option<&'static str> {
        Some("*")
    }

    /// Parse a native range string into a fully parsed `VersVersionRange`.
    ///
    /// This is the main entry point for converting native syntax into vers ranges.
    /// The default implementation calls [`Self::from_native`] to parse the
    /// constraints, wraps the result with the given `scheme` name, and runs
    /// [`VersVersionRange::normalize_and_validate`] to ensure the range is in
    /// canonical form. Schemes whose native syntax requires special handling
    /// can override this directly.
    ///
    /// # Arguments
    ///
    /// * `scheme` - The versioning scheme name (e.g. `"deb"`, `"semver"`, `"npm"`)
    /// * `raw` - The native range string
    fn from_native_string(scheme: &str, raw: &str) -> Result<VersVersionRange<Self>, VersError> {
        let mut range = VersVersionRange::new(scheme.to_string(), Self::from_native(raw)?);
        range.normalize_and_validate()?;
        Ok(range)
    }

    /// Parse a full native range string into one or more standard `VersionConstraint`s.
    ///
    /// This is called by [`Self::from_native_string`] (for bare native strings). It receives
    /// the entire constraint portion of the vers string (after the scheme prefix).
    ///
    /// The default implementation splits on [`Self::NATIVE_SEPARATOR`] and calls
    /// [`Self::from_native_constraint`] for each segment. Schemes whose native
    /// syntax doesn't use a single-character delimiter should override this
    /// method directly.
    fn from_native(raw: &str) -> Result<Vec<VersionConstraint<Self>>, VersError> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(VersError::EmptyConstraints);
        }

        let segments: Vec<&str> = raw
            .split(Self::NATIVE_SEPARATOR)
            .map(|s| s.trim())
            .collect();
        let mut constraints = Vec::new();
        for segment in segments {
            if segment.is_empty() {
                return Err(VersError::EmptyConstraints);
            }
            constraints.push(Self::from_native_constraint(segment)?);
        }

        Ok(constraints)
    }

    /// Parse a single native constraint string into a `VersionConstraint`.
    ///
    /// The default implementation first checks for [`Self::native_wildcard`],
    /// then matches a leading operator token from [`Self::native_operators`]
    /// (in order, so list longer tokens before their prefixes), and otherwise
    /// falls back to implicit equality by parsing the whole (trimmed) string
    /// as a bare version. Schemes with more involved native syntax (e.g.
    /// delegating to an external parser) can override this method directly
    /// instead of customizing the operator table.
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

            let version = version_str.parse::<Self>().map_err(Into::into)?;
            Ok(VersionConstraint::new(comparator, version))
        } else {
            return Err(VersError::InvalidConstraint(raw.into()));
        }
    }
}

/// A trait alias for version types that can be used in version constraints and ranges.
pub trait VersionType:
    FromStr + Default + Ord + PartialOrd + Clone + Display + Debug + Serialize + Hash
{
}

/// Blanket implementation for any type that satisfies the bounds
impl<T> VersionType for T where
    T: FromStr + Default + Ord + PartialOrd + Clone + Display + Debug + Serialize + Hash
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
    pub fn parse(constraint_str: &str) -> Result<Self, VersError>
    where
        V::Err: Into<VersError>,
    {
        if constraint_str.is_empty() {
            return Err(VersError::InvalidConstraint("Empty constraint".to_string()));
        }

        if constraint_str == "*" {
            return Ok(Self {
                comparator: Comparator::Any,
                version: V::default(),
            });
        }

        // Explicit '=' prefix is forbidden per spec update #95;
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
            // without any prefix we assume Equal
            // Equal comparator is strictly implicit (e.g., "1.2.3", not "=1.2.3")
            (Comparator::Equal, constraint_str)
        };

        if constraint_str.starts_with('=') {
            return Err(VersError::InvalidConstraint(format!(
                "Explicit equality operator is not allowed; use a bare version without a leading '=': {}",
                constraint_str
            )));
        }

        if version.is_empty() && comparator != Comparator::Any {
            return Err(VersError::InvalidConstraint("Missing version".to_string()));
        }

        if ALLOWED_OPERATORS.iter().any(|op| version.contains(op)) {
            return Err(VersError::UnencodedOperator(version.to_string()));
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

        let parsed_version = version_str.parse::<V>().map_err(|e| e.into())?;

        Ok(Self {
            comparator,
            version: parsed_version,
        })
    }
}

impl<V: VersionType> Display for VersionConstraint<V> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.comparator {
            Comparator::Equal => write!(f, "{}", self.version), // Implicit equal prints no operator
            _ => write!(f, "{}{}", self.comparator, self.version), // Others print operator + version
        }
    }
}

#[cfg(test)]
mod native_version_converter_tests {
    use super::*;

    /// A minimal version type wrapping `u32`, used only to prove that
    /// `NativeVersionConverter`'s separator, operator table, and wildcard
    /// token are independently configurable per scheme.
    #[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
    struct TestVersion(u32);

    impl Display for TestVersion {
        fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
            write!(f, "{}", self.0)
        }
    }

    impl FromStr for TestVersion {
        type Err = VersError;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            s.parse::<u32>().map(TestVersion).map_err(|_| {
                VersError::InvalidVersionFormat(
                    "test".to_string(),
                    s.to_string(),
                    "not a number".to_string(),
                )
            })
        }
    }

    /// Uses `;` as its constraint separator and word-based operator tokens
    /// (`gte`, `lte`, `gt`, `lt`, `eq`) instead of the standard vers symbols,
    /// and disables the wildcard token entirely.
    impl NativeVersionConverter for TestVersion {
        const SCHEME_NAME: &'static str = "test";
        const NATIVE_SEPARATOR: char = ';';

        fn native_operators() -> &'static [(&'static str, Comparator)] {
            &[
                ("gte", Comparator::GreaterThanOrEqual),
                ("lte", Comparator::LessThanOrEqual),
                ("gt", Comparator::GreaterThan),
                ("lt", Comparator::LessThan),
                ("eq", Comparator::Equal),
            ]
        }

        fn native_wildcard() -> Option<&'static str> {
            None
        }
    }

    #[test]
    fn custom_separator_splits_into_multiple_constraints() {
        let constraints = TestVersion::from_native("gte1;lt5").unwrap();
        assert_eq!(constraints.len(), 2);
        assert_eq!(constraints[0].comparator, Comparator::GreaterThanOrEqual);
        assert_eq!(constraints[0].version, TestVersion(1));
        assert_eq!(constraints[1].comparator, Comparator::LessThan);
        assert_eq!(constraints[1].version, TestVersion(5));
    }

    #[test]
    fn pipe_is_not_treated_as_a_separator() {
        // '|' has no special meaning for this scheme; it's just part of the
        // (invalid) version text, so parsing the whole segment as a version fails.
        let result = TestVersion::from_native("gte1|lt5");
        assert!(result.is_err());
    }

    #[test]
    fn custom_operator_tokens_are_recognized() {
        let constraint = TestVersion::from_native_constraint("eq42").unwrap();
        assert_eq!(constraint.comparator, Comparator::Equal);
        assert_eq!(constraint.version, TestVersion(42));
    }

    #[test]
    fn standard_vers_operators_are_not_recognized_for_this_scheme() {
        // ">=" isn't in this scheme's operator table, so it's treated as part
        // of an implicit-equal version string, which then fails to parse.
        let result = TestVersion::from_native_constraint(">=1");
        assert!(result.is_err());
    }

    #[test]
    fn wildcard_disabled_for_this_scheme() {
        // Default wildcard is "*", but this scheme disables it, so "*" is
        // just an (invalid) version string.
        let result = TestVersion::from_native_constraint("*");
        assert!(result.is_err());
    }

    #[test]
    fn default_operator_table_accepts_explicit_equal() {
        use crate::schemes::semver::SemVer;

        // Unlike canonical vers strings, native syntax allows an explicit '='.
        let constraint = SemVer::from_native_constraint("=1.2.3").unwrap();
        assert_eq!(constraint.comparator, Comparator::Equal);
        assert_eq!(constraint.version, "1.2.3".parse().unwrap());
    }

    #[test]
    fn default_wildcard_still_works_when_not_overridden() {
        use crate::schemes::semver::SemVer;

        let constraint = SemVer::from_native_constraint("*").unwrap();
        assert_eq!(constraint.comparator, Comparator::Any);
    }
}
