use crate::constraint::NativeVersionConverter;
use crate::range::VersionRange;
use crate::schemes::deb::DebVersion;
use crate::schemes::semver::SemVer;
use crate::{VersError, VersVersionRange, VersionConstraint};
use std::fmt;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use std::sync::OnceLock;

/// Internal enum for the actual version range implementation
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(tag = "versioning_scheme")]
enum DynamicVersionRangeInner {
    /// SemVer-based range (for "semver" and "npm" schemes)
    #[serde(rename = "semver", alias = "npm")]
    SemVer(VersVersionRange<SemVer>),
    /// Debian dpkg-style versioning ("deb" scheme)
    #[serde(rename = "deb")]
    Deb(VersVersionRange<DebVersion>),
}

/// A dynamic version range that automatically detects the versioning scheme.
///
/// This wrapper provides dynamic dispatch for version ranges, automatically
/// detecting the versioning scheme and constructing the appropriate typed
/// version range internally.
///
/// It currently supports the following schemes:
/// - "semver" and "npm" schemes using SemVer version type
///
/// # Examples
///
/// ```
/// use vers_rs::range::dynamic::DynamicVersionRange;
/// use vers_rs::range::VersionRange;
///
/// // Parse ranges with different schemes
/// let npm_range: DynamicVersionRange = "vers:npm/>=1.0.0|<2.0.0".parse().unwrap();
/// let semver_range: DynamicVersionRange = "vers:semver/>=1.0.0|<2.0.0".parse().unwrap();
///
/// // Check if versions are contained
/// assert!(npm_range.contains("1.5.0".to_string()).unwrap());
/// assert!(!npm_range.contains("2.0.0".to_string()).unwrap());
/// ```
#[derive(Debug)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct DynamicVersionRange {
    inner: DynamicVersionRangeInner,
    cached_constraints: OnceLock<Vec<VersionConstraint<String>>>,
}

// Custom PartialEq and Eq implementations that only compare the inner range, not the cache
impl PartialEq for DynamicVersionRange {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for DynamicVersionRange {}

// Custom Clone implementation that clones the inner range but not the cache
impl Clone for DynamicVersionRange {
    fn clone(&self) -> Self {
        DynamicVersionRange {
            inner: self.inner.clone(),
            cached_constraints: OnceLock::new(),
        }
    }
}

/// Macro to eliminate repetition in match arms that do the same thing for all variants
macro_rules! dispatch_inner {
    ($inner:expr, $range:ident => $expr:expr) => {
        match $inner {
            DynamicVersionRangeInner::SemVer($range) => $expr,
            DynamicVersionRangeInner::Deb($range) => $expr,
        }
    };
}

impl DynamicVersionRange {
    /// Parse a native range string for the given versioning scheme into a `DynamicVersionRange`.
    ///
    /// Unlike `FromStr`, this does **not** require the `vers:scheme/` prefix. It accepts
    /// the scheme name and a native range string directly, delegating to the scheme's
    /// [`NativeVersionConverter`] implementation.
    ///
    /// # Arguments
    ///
    /// * `scheme` - The versioning scheme name (e.g. `"deb"`, `"semver"`, `"npm"`)
    /// * `raw` - The native range string (e.g. `"<<1.0"`, `">=1.0.0|<2.0.0"`)
    ///
    /// # Returns
    ///
    /// A `Result` containing the parsed `DynamicVersionRange` or an error.
    ///
    /// # Examples
    ///
    /// ```
    /// use vers_rs::range::dynamic::DynamicVersionRange;
    /// use vers_rs::range::VersionRange;
    ///
    /// let range = DynamicVersionRange::parse_native("deb", "<<1.0").unwrap();
    /// assert_eq!(range.versioning_scheme(), "deb");
    /// assert!(range.contains("0.9".to_string()).unwrap());
    /// ```
    pub fn parse_native(scheme: &str, raw: &str) -> Result<Self, VersError> {
        let inner = match scheme {
            "semver" | "npm" => DynamicVersionRangeInner::SemVer(SemVer::from_native_string(raw)?),
            "deb" => DynamicVersionRangeInner::Deb(DebVersion::from_native_string(raw)?),
            _ => return Err(VersError::UnsupportedVersioningScheme(scheme.to_string())),
        };

        Ok(DynamicVersionRange {
            inner,
            cached_constraints: OnceLock::new(),
        })
    }

    /// Extract the versioning scheme from a version range specifier string.
    ///
    /// This is a helper function used internally to determine which version type
    /// to use when parsing the range.
    fn extract_versioning_scheme(s: &str) -> Result<String, VersError> {
        // Remove all spaces and tabs
        let s = s.replace(|c: char| c.is_whitespace(), "");

        // Split on colon
        let parts: Vec<&str> = s.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(VersError::InvalidScheme);
        }

        // Validate URI scheme
        let scheme = parts[0];
        if scheme != "vers" {
            return Err(VersError::InvalidScheme);
        }

        // Split on slash
        let specifier_parts: Vec<&str> = parts[1].splitn(2, '/').collect();
        if specifier_parts.len() != 2 {
            return Err(VersError::MissingVersioningScheme);
        }

        // Get versioning scheme
        let versioning_scheme = specifier_parts[0].to_lowercase();
        if versioning_scheme.is_empty() {
            return Err(VersError::MissingVersioningScheme);
        }

        Ok(versioning_scheme)
    }
}

impl VersionRange<String> for DynamicVersionRange {
    /// Get the versioning scheme used by this range.
    ///
    /// # Returns
    ///
    /// The versioning scheme string (e.g., "npm", "semver")
    ///
    /// # Examples
    ///
    /// ```
    /// use vers_rs::range::dynamic::DynamicVersionRange;
    /// use vers_rs::range::VersionRange;
    ///
    /// let range: DynamicVersionRange = "vers:npm/>=1.0.0".parse().unwrap();
    /// assert_eq!(range.versioning_scheme(), "npm");
    /// ```
    fn versioning_scheme(&self) -> &str {
        dispatch_inner!(&self.inner, range => &range.versioning_scheme)
    }

    /// Check if a version string is contained within this range.
    ///
    /// This method automatically parses the version string using the appropriate
    /// version type based on the detected versioning scheme.
    ///
    /// # Arguments
    ///
    /// * `version_str` - The version string to check
    ///
    /// # Returns
    ///
    /// A `Result` containing a boolean indicating whether the version is in the range
    ///
    /// # Examples
    ///
    /// ```
    /// use vers_rs::range::dynamic::DynamicVersionRange;
    /// use vers_rs::range::VersionRange;
    ///
    /// let range: DynamicVersionRange = "vers:npm/>=1.0.0|<2.0.0".parse().unwrap();
    /// assert!(range.contains("1.5.0".to_string()).unwrap());
    /// assert!(!range.contains("2.0.0".to_string()).unwrap());
    /// ```
    fn contains(&self, version_str: String) -> Result<bool, VersError> {
        match &self.inner {
            DynamicVersionRangeInner::SemVer(range) => {
                range.contains(version_str.parse::<SemVer>()?)
            }
            DynamicVersionRangeInner::Deb(range) => {
                range.contains(version_str.parse::<DebVersion>()?)
            }
        }
    }

    /// Get the constraints in this range as Strings.
    ///
    /// # Returns
    ///
    /// A vector of type-erased version constraints
    ///
    /// # Examples
    ///
    /// ```
    /// use vers_rs::range::dynamic::DynamicVersionRange;
    /// use vers_rs::range::VersionRange;
    ///
    /// let range: DynamicVersionRange = "vers:npm/>=1.0.0|<2.0.0".parse().unwrap();
    /// assert_eq!(range.constraints().len(), 2);
    /// ```
    fn constraints(&self) -> &Vec<VersionConstraint<String>> {
        self.cached_constraints.get_or_init(|| {
            dispatch_inner!(&self.inner, range => {
                range
                    .constraints
                    .iter()
                    .map(|c| VersionConstraint::new(c.comparator, c.version.to_string()))
                    .collect()
            })
        })
    }
}

impl FromStr for DynamicVersionRange {
    type Err = VersError;

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
    /// use vers_rs::range::dynamic::DynamicVersionRange;
    /// use vers_rs::range::VersionRange;
    ///
    /// let range: DynamicVersionRange = "vers:npm/>=1.0.0|<2.0.0".parse().unwrap();
    /// assert_eq!(range.versioning_scheme(), "npm");
    /// ```
    fn from_str(s: &str) -> Result<Self, VersError> {
        // Extract the versioning scheme first to determine which type to use
        let versioning_scheme = DynamicVersionRange::extract_versioning_scheme(s)?;

        let inner = match versioning_scheme.as_str() {
            "semver" | "npm" => DynamicVersionRangeInner::SemVer(s.parse()?),
            "deb" => DynamicVersionRangeInner::Deb(s.parse()?),
            _ => return Err(VersError::UnsupportedVersioningScheme(versioning_scheme)),
        };

        Ok(DynamicVersionRange {
            inner,
            cached_constraints: OnceLock::new(),
        })
    }
}

impl Display for DynamicVersionRange {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        dispatch_inner!(&self.inner, range => write!(f, "{}", range))
    }
}

impl serde::ser::Serialize for DynamicVersionRange {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        dispatch_inner!(&self.inner, range => range.serialize(serializer))
    }
}

impl<'de> serde::de::Deserialize<'de> for DynamicVersionRange {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let inner = DynamicVersionRangeInner::deserialize(deserializer)?;
        Ok(DynamicVersionRange {
            inner,
            cached_constraints: OnceLock::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::DynamicVersionRange;
    use crate::Comparator;
    use crate::VersError;
    use crate::range::VersionRange;

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
        let range: DynamicVersionRange = "vers:npm/*".parse().unwrap();
        assert_eq!(range.versioning_scheme(), "npm");
        assert_eq!(range.constraints().len(), 1);
        assert_eq!(range.constraints()[0].comparator, Comparator::Any);
        assert_eq!(range.constraints()[0].version.to_string(), "0.0.0");
    }

    #[test]
    fn test_parse_with_spaces() {
        let range: DynamicVersionRange = "vers:npm/ >= 1.0.0 | < 2.0.0 ".parse().unwrap();
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
        let range: DynamicVersionRange = "vers:npm/1.0.0%2Bbuild.1".parse().unwrap();
        assert_eq!(range.versioning_scheme(), "npm");
        assert_eq!(range.constraints().len(), 1);
        assert_eq!(range.constraints()[0].comparator, Comparator::Equal);
        assert_eq!(range.constraints()[0].version.to_string(), "1.0.0+build.1");
    }

    #[test]
    fn test_invalid_constraint_simplification() {
        let result: DynamicVersionRange = "vers:npm/1.2.3|<2.0.0".parse().unwrap();
        assert_eq!(result.to_string(), "vers:npm/<2.0.0");

        let result: DynamicVersionRange = "vers:npm/>1.0.0|>2.0.0".parse().unwrap();
        assert_eq!(result.to_string(), "vers:npm/>1.0.0");

        let result: DynamicVersionRange = "vers:npm/<1.0.0|<2.0.0".parse().unwrap();
        assert_eq!(result.to_string(), "vers:npm/<2.0.0");
    }

    #[test]
    fn test_display() {
        let range: DynamicVersionRange = "vers:npm/>=1.0.0|<2.0.0".parse().unwrap();
        assert_eq!(range.to_string(), "vers:npm/>=1.0.0|<2.0.0");

        let range: DynamicVersionRange = "vers:npm/*".parse().unwrap();
        assert_eq!(range.to_string(), "vers:npm/*");

        let range: DynamicVersionRange = "vers:npm/1.2.3".parse().unwrap();
        assert_eq!(range.to_string(), "vers:npm/1.2.3");
    }

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
    fn test_contains_simple() {
        let range: DynamicVersionRange = "vers:npm/1.2.3".parse().unwrap();
        assert!(range.contains("1.2.3".to_string()).unwrap());
        assert!(!range.contains("1.2.4".to_string()).unwrap());
    }

    #[test]
    fn test_contains_range() {
        let range: DynamicVersionRange = "vers:npm/>=1.0.0|<2.0.0".parse().unwrap();
        assert!(range.contains("1.0.0".to_string()).unwrap());
        assert!(range.contains("1.5.0".to_string()).unwrap());
        assert!(!range.contains("2.0.0".to_string()).unwrap());
        assert!(!range.contains("0.9.0".to_string()).unwrap());
    }

    #[test]
    fn test_contains_star() {
        let range: DynamicVersionRange = "vers:npm/*".parse().unwrap();
        assert!(range.contains("1.0.0".to_string()).unwrap());
        assert!(range.contains("2.0.0".to_string()).unwrap());
        assert!(range.contains("0.0.1".to_string()).unwrap());
    }

    #[test]
    fn test_contains_not_equal() {
        let range: DynamicVersionRange = "vers:npm/!=1.2.3".parse().unwrap();
        assert!(!range.contains("1.2.3".to_string()).unwrap());
        assert!(range.contains("1.2.4".to_string()).unwrap());
    }

    #[test]
    fn test_contains_complex() {
        let range: DynamicVersionRange = "vers:npm/>=1.0.0|<2.0.0|!=1.5.0".parse().unwrap();
        assert!(range.contains("1.0.0".to_string()).unwrap());
        assert!(range.contains("1.7.0".to_string()).unwrap());
        assert!(!range.contains("1.5.0".to_string()).unwrap());
        assert!(!range.contains("2.0.0".to_string()).unwrap());
        assert!(!range.contains("0.9.0".to_string()).unwrap());
    }
}
