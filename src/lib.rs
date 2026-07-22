// Module declarations
pub mod comparator;
pub mod constraint;
pub mod error;
pub mod range;
pub mod schemes;

pub use comparator::Comparator;
pub use constraint::NativeVersionConverter;
pub use constraint::VersionConstraint;
pub use error::VersError;
pub use range::VersionRange;
pub use range::dynamic::DynamicVersionRange;
pub use range::vers::VersVersionRange;

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
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn parse(s: &str) -> Result<DynamicVersionRange, VersError> {
    s.parse()
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

/// Parse a native range string for the given versioning scheme into a `DynamicVersionRange`.
///
/// This function accepts a scheme name and a native range string directly, without
/// requiring the `vers:scheme/` prefix. It delegates to the scheme's
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
/// use vers_rs::parse_native;
/// use vers_rs::range::VersionRange;
///
/// let range = parse_native("deb", "<<1.0").unwrap();
/// assert_eq!(range.versioning_scheme(), "deb");
/// assert!(range.contains("0.9".to_string()).unwrap());
/// ```
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn parse_native(scheme: &str, raw: &str) -> Result<DynamicVersionRange, VersError> {
    DynamicVersionRange::parse_native(scheme, raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::range::VersionRange;

    #[test]
    fn test_parse_dynamic_function() {
        let range = parse("vers:npm/>=1.0.0|<2.0.0").unwrap();
        assert_eq!(range.versioning_scheme(), "npm");
        assert_eq!(range.constraints().len(), 2);
    }
}
