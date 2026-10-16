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

// u8::is_ascii_whitespace doesn't cover 0x0B so we define our own array of ASCII whitespace characters
const ASCII_WHITESPACE_CHARACTERS: [u8; 6] = [b' ', b'\t', b'\n', b'\r', 0x0B, 0x0C];
const ALLOWED_OPERATORS: [&str; 5] = [">", "<", "!=", "<=", ">="];

#[cfg(feature = "wasm")]
pub mod wasm;

/// All versioning schemes (VERS **types**) defined by the [VERS specification](https://github.com/package-url/vers-spec).
///
/// This includes both the package ecosystem VERS types (named after their
/// registered PURL type) and the general-purpose VERS types.
///
/// See <https://github.com/package-url/vers-spec/blob/main/docs/types/vers-types.md>
/// for the authoritative list and descriptions of each scheme.
pub const VERSIONING_SCHEMES: &[&str] = &[
    // Package ecosystem VERS types
    "alpm",
    "apk",
    "cargo",
    "composer",
    "cpan",
    "dart",
    "deb",
    "gem",
    "golang",
    "hackage",
    "maven",
    "nuget",
    "npm",
    "pypi",
    "rpm",
    // General VERS types
    "all",
    "datetime",
    "generic",
    "intdot",
    "lexicographic",
    "none",
    "semver",
];

/// Check if a string is a known versioning scheme (VERS **type**) as defined
/// by the [VERS specification](https://github.com/package-url/vers-spec).
///
/// The comparison is case-sensitive, matching the canonical scheme names
/// listed in [`VERSIONING_SCHEMES`].
///
/// # Examples
///
/// ```
/// use vers_rs::is_registered_scheme;
///
/// assert!(is_registered_scheme("npm"));
/// assert!(!is_registered_scheme("not-a-scheme"));
/// ```
pub fn is_registered_scheme(scheme: &str) -> bool {
    VERSIONING_SCHEMES.contains(&scheme)
}

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

/// Determine if a parsed vers string only consists of a single version.
///
/// A vers string is a single version when it has exactly one constraint using
/// the implicit equality comparator (e.g. `vers:npm/1.2.3`). Strings using `*`,
/// `!=`, or any comparison operator (even just one, e.g. `vers:npm/>=1.0.0`)
/// are not single versions, since they match more than one version.
///
/// # Arguments
///
/// * `s` - The version range specifier string to parse and check
///
/// # Returns
///
/// A `Result` containing a boolean indicating whether the string is a single version
///
/// # Examples
///
/// ```
/// use vers_rs::is_single_version;
///
/// assert!(is_single_version("vers:npm/1.2.3").unwrap());
/// assert!(!is_single_version("vers:npm/>=1.0.0|<2.0.0").unwrap());
/// assert!(!is_single_version("vers:npm/>=1.0.0").unwrap());
/// ```
pub fn is_single_version(s: &str) -> Result<bool, VersError> {
    Ok(parse(s)?.is_single_version())
}

/// Retrieve the single version of a parsed vers string, if there is one.
///
/// Returns `Some` with the version string when the range has exactly one
/// constraint using the implicit equality comparator (e.g. `vers:npm/1.2.3`).
/// Returns `None` for ranges using `*`, `!=`, or any comparison operator (even
/// just one, e.g. `vers:npm/>=1.0.0`), since those match more than one version.
///
/// # Arguments
///
/// * `s` - The version range specifier string to parse and check
///
/// # Returns
///
/// A `Result` containing `Some(version)` if the string is a single version, or `None` otherwise
///
/// # Examples
///
/// ```
/// use vers_rs::single_version;
///
/// assert_eq!(single_version("vers:npm/1.2.3").unwrap(), Some("1.2.3".to_string()));
/// assert_eq!(single_version("vers:npm/>=1.0.0|<2.0.0").unwrap(), None);
/// assert_eq!(single_version("vers:npm/>=1.0.0").unwrap(), None);
/// ```
pub fn single_version(s: &str) -> Result<Option<String>, VersError> {
    let range = parse(s)?;
    Ok(range.single_version().cloned())
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
pub fn parse_native(scheme: &str, raw: &str) -> Result<DynamicVersionRange, VersError> {
    DynamicVersionRange::parse_native(scheme, raw)
}
