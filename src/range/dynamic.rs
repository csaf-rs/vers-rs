use crate::constraint::VT;
use crate::range::VersionRange;
use crate::schemes::semver::SemVer;
use crate::{GenericVersionRange, VersError, VersionConstraint};
use std::fmt;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

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
/// assert!(npm_range.contains("1.5.0").unwrap());
/// assert!(!npm_range.contains("2.0.0").unwrap());
/// ```
#[derive(PartialEq, Eq, Debug, Clone)]
pub enum DynamicVersionRange {
    /// SemVer-based range (for "semver" and "npm" schemes)
    SemVer(GenericVersionRange<SemVer>),
}

impl DynamicVersionRange {
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

impl VersionRange<&str> for DynamicVersionRange {
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
        match self {
            DynamicVersionRange::SemVer(range) => &range.versioning_scheme,
        }
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
    /// assert!(range.contains("1.5.0").unwrap());
    /// assert!(!range.contains("2.0.0").unwrap());
    /// ```
    fn contains(&self, version_str: &str) -> Result<bool, VersError> {
        match self {
            DynamicVersionRange::SemVer(range) => {
                let version: SemVer = version_str.parse()?;
                range.contains(&version)
            }
        }
    }

    /// Get the constraints in this range.
    ///
    /// # Returns
    ///
    /// A reference to the constraints Vec in this range
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
    fn constraints(&self) -> &Vec<VersionConstraint<impl VT>> {
        match self {
            DynamicVersionRange::SemVer(range) => &range.constraints,
        }
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
        let versioning_scheme = Self::extract_versioning_scheme(s)?;

        match versioning_scheme.as_str() {
            "semver" | "npm" => {
                let range: GenericVersionRange<SemVer> = s.parse()?;
                Ok(DynamicVersionRange::SemVer(range))
            }
            _ => Err(VersError::UnsupportedVersioningScheme(versioning_scheme)),
        }
    }
}

impl Display for DynamicVersionRange {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            DynamicVersionRange::SemVer(range) => write!(f, "{}", range),
        }
    }
}