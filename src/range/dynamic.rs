use crate::constraint::NativeVersionConverter;
use crate::range::VersionRange;
use crate::schemes::{cargo::CargoVersion, deb::DebVersion, intdot::IntDotVersion, semver::SemVer};
use crate::util::ascii_whitespace::is_ascii_vers_whitespace;
use crate::{
    VersError, VersVersionRange, VersionConstraint, VersioningScheme,
};
use std::fmt;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use std::sync::OnceLock;

/// Internal enum for the actual version range implementation
///
/// `Deserialize` for this enum is implemented manually below (see the `Deserialize` impl for
/// `DynamicVersionRange`) rather than derived: a derived `#[serde(tag = "versioning_scheme")]`
/// (internal tagging) would require serde to buffer the input via `deserialize_any`. That's
/// fine here since `DynamicVersionRange`'s `Deserialize` impl is explicitly JSON-only (see
/// its doc comment below), but it's why this can't be a plain derive.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
enum DynamicVersionRangeInner {
    /// SemVer-based range (for "semver" and "npm" schemes)
    SemVer(VersVersionRange<SemVer>),
    /// Debian dpkg-style versioning ("deb" scheme)
    Deb(VersVersionRange<DebVersion>),
    /// Cargo-based range ("cargo" scheme)
    Cargo(VersVersionRange<CargoVersion>),
    /// Generic dot-separated-integers range ("intdot" scheme)
    IntDot(VersVersionRange<IntDotVersion>),
}

/// A dynamic version range that automatically detects the versioning scheme.
///
/// This wrapper provides dynamic dispatch for version ranges, automatically
/// detecting the versioning scheme and constructing the appropriate typed
/// version range internally.
///
/// It currently supports the following schemes:
/// - "semver" and "npm" schemes using the `SemVer` version type
/// - "deb" using the `DebVersion` version type
/// - "cargo" using the `CargoVersion` version type
/// - "intdot" using the `IntDotVersion` version type
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
/// let cargo_range: DynamicVersionRange = "vers:cargo/>=1.0.0|<2.0.0".parse().unwrap();
///
/// // Check if versions are contained
/// assert!(npm_range.contains("1.5.0".to_string()).unwrap());
/// assert!(!npm_range.contains("2.0.0".to_string()).unwrap());
/// assert!(semver_range.contains("1.5.0".to_string()).unwrap());
/// assert!(!cargo_range.contains("2.0.0".to_string()).unwrap());
/// ```
#[derive(Debug, Eq)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
// `Serialize`/`Deserialize` are implemented manually below and delegate straight to the
// inner `VersVersionRange<SemVer>`/`VersVersionRange<DebVersion>` value (see `dispatch_inner!`
// in `serialize`), so the wire format is that flat union, not `{ inner: ... }` as the derive
// would otherwise infer from this struct's fields. Override the generated TS type to match.
#[cfg_attr(
    feature = "wasm",
    tsify(
        type = "VersVersionRange<SemVer> | VersVersionRange<DebVersion> | VersVersionRange<CargoVersion> | VersVersionRange<IntDotVersion>"
    )
)]
pub struct DynamicVersionRange {
    inner: DynamicVersionRangeInner,
    // Internal memoization cache, not part of the public data model: excluded from the
    // generated TypeScript type via `serde(skip)`, which `tsify::Tsify` also honors even
    // though `Serialize`/`Deserialize` are implemented manually below rather than derived.
    #[cfg_attr(feature = "wasm", serde(skip))]
    cached_constraints: OnceLock<Vec<VersionConstraint<String>>>,
}

// Custom PartialEq implementation that only compares the inner range, not the cache
impl PartialEq for DynamicVersionRange {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

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
            DynamicVersionRangeInner::Cargo($range) => $expr,
            DynamicVersionRangeInner::IntDot($range) => $expr,
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
        let scheme_kind = VersioningScheme::from_str(scheme)
            .map_err(|_| VersError::UnknownVersioningScheme(scheme.to_string()))?;

        let inner = match scheme_kind {
            VersioningScheme::Semver | VersioningScheme::Npm => {
                DynamicVersionRangeInner::SemVer(SemVer::from_native_string(scheme, raw)?)
            }
            VersioningScheme::Deb => {
                DynamicVersionRangeInner::Deb(DebVersion::from_native_string(scheme, raw)?)
            }
            VersioningScheme::Cargo => {
                DynamicVersionRangeInner::Cargo(CargoVersion::from_native_string(scheme, raw)?)
            }
            VersioningScheme::Intdot => {
                DynamicVersionRangeInner::IntDot(IntDotVersion::from_native_string(scheme, raw)?)
            }
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
    pub(crate) fn extract_versioning_scheme(s: &str) -> Result<(String, String), VersError> {
        // Split on colon
        let parts: Vec<&str> = s.splitn(2, ':').collect();
        if parts.len() != 2 || parts[0] != "vers" {
            return Err(VersError::InvalidScheme);
        }

        // Split on slash
        let specifier_parts: Vec<&str> = parts[1].splitn(2, '/').collect();
        if specifier_parts.len() != 2 {
            return Err(VersError::MissingVersioningScheme);
        }

        // Get versioning scheme
        let versioning_scheme = specifier_parts[0].to_string();
        if versioning_scheme.is_empty() {
            return Err(VersError::MissingVersioningScheme);
        }

        Ok((versioning_scheme, specifier_parts[1].to_string()))
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
    /// For the `intdot` scheme, there is no separate canonical form, so the
    /// version string is always parsed leniently (tolerating leading zeros
    /// and an ignored suffix), same as the range itself.
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
            DynamicVersionRangeInner::Cargo(range) => {
                range.contains(version_str.parse::<CargoVersion>()?)
            }
            DynamicVersionRangeInner::IntDot(range) => {
                range.contains(version_str.parse::<IntDotVersion>()?)
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
    /// use vers_rs::VersError;
    ///
    /// let range: DynamicVersionRange = "vers:npm/>=1.0.0|<2.0.0".parse().unwrap();
    /// assert_eq!(range.versioning_scheme(), "npm");
    /// ```
    fn from_str(s: &str) -> Result<Self, VersError> {
        if !s.is_ascii() {
            return Err(VersError::InvalidCharacters(
                "A VERS string shall only contain ASCII characters".to_string(),
            ));
        }

        if s.bytes().any(is_ascii_vers_whitespace) {
            return Err(VersError::InvalidCharacters(
                "Control characters (tabs, newlines, carriage returns) are not permitted"
                    .to_string(),
            ));
        }

        // Extract the versioning scheme first to determine which type to use
        let (versioning_scheme, _) = DynamicVersionRange::extract_versioning_scheme(s)?;

        let scheme_kind = VersioningScheme::from_str(&versioning_scheme)
            .map_err(|_| VersError::UnknownVersioningScheme(versioning_scheme.to_string()))?;

        let inner = match scheme_kind {
            VersioningScheme::Semver | VersioningScheme::Npm => {
                DynamicVersionRangeInner::SemVer(s.parse()?)
            }
            VersioningScheme::Deb => DynamicVersionRangeInner::Deb(s.parse()?),
            VersioningScheme::Cargo => DynamicVersionRangeInner::Cargo(s.parse()?),
            VersioningScheme::Intdot => DynamicVersionRangeInner::IntDot(s.parse()?),
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

/// Manual `Deserialize` impl, **JSON-only**.
///
/// `DynamicVersionRangeInner`'s natural representation would be an internally tagged enum
/// (`#[serde(tag = "versioning_scheme")]`), but that fails here: since `VersVersionRange<V>`
/// (the variants' payload) *also* has its own `versioning_scheme` field, that field is no
/// longer available once the tag has been consumed, and deserialization fails with "missing
/// field `versioning_scheme`". This mirrors a known serde limitation (see
/// serde-rs/serde#1560).
///
/// The workaround below buffers the input into a `serde_json::Value` and re-dispatches based
/// on the `versioning_scheme` field. Building a `serde_json::Value` requires
/// `deserialize_any`, which non-self-describing formats (bincode, postcard, ...) do not
/// support, so this impl only works with self-describing formats and is intended for JSON
/// specifically; other formats should not rely on it.
impl<'de> serde::de::Deserialize<'de> for DynamicVersionRange {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let scheme = value
            .get("versioning_scheme")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| serde::de::Error::missing_field("versioning_scheme"))?;
        let scheme_kind = scheme.parse::<VersioningScheme>().map_err(|_| {
            serde::de::Error::custom(format!("unsupported versioning scheme: {scheme}"))
        })?;
        let inner = match scheme_kind {
            VersioningScheme::Semver | VersioningScheme::Npm => DynamicVersionRangeInner::SemVer(
                serde_json::from_value(value).map_err(serde::de::Error::custom)?,
            ),
            VersioningScheme::Deb => DynamicVersionRangeInner::Deb(
                serde_json::from_value(value).map_err(serde::de::Error::custom)?,
            ),
            VersioningScheme::Cargo => DynamicVersionRangeInner::Cargo(
                serde_json::from_value(value).map_err(serde::de::Error::custom)?,
            ),
            VersioningScheme::Intdot => DynamicVersionRangeInner::IntDot(
                serde_json::from_value(value).map_err(serde::de::Error::custom)?,
            ),
            _ => {
                return Err(serde::de::Error::custom(format!(
                    "unsupported versioning scheme: {scheme}"
                )));
            }
        };
        Ok(DynamicVersionRange {
            inner,
            cached_constraints: OnceLock::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches;
    use std::mem::discriminant;

    use rstest::rstest;

    use super::DynamicVersionRange;
    use crate::Comparator;
    use crate::VersError;
    use crate::range::VersionRange;

    #[rstest]
    #[case::invalid_tab("vers:npm\t/1.2.3", VersError::InvalidCharacters("blub".to_string()))]
    #[case::invalid_newline("vers:npm\n/1.2.3", VersError::InvalidCharacters("blub".to_string()))]
    #[case::invalid_carriage_return("vers:npm\r/1.2.3", VersError::InvalidCharacters("blub".to_string()))]
    #[case::invalid_form_feed("vers:npm/\x0c1.2.3", VersError::InvalidCharacters("blub".to_string()))]
    #[case::invalid_vertical_tab("vers:npm/\x0b1.2.3", VersError::InvalidCharacters("blub".to_string()))]
    #[case::invalid_space_in_type("vers:npm /1.2.3", VersError::InvalidCharacters("blub".to_string()))]
    #[case::invalid_space_in_constraint("vers:semver/1.2.3 |2.0.0", VersError::InvalidCharacters("blub".to_string()))]
    #[case::non_ascii_in_type("vers:npém/1.2.3", VersError::InvalidCharacters("blub".to_string()))]
    #[case::non_ascii_in_version("vers:npm/1.2.3é", VersError::InvalidCharacters("blub".to_string()))]
    #[case::non_ascii_whitespace("vers:npm/1.2.3\u{00A0}", VersError::InvalidCharacters("blub".to_string()))]
    #[case::non_ascii_fullwidth_digit("vers:npm/１.2.3", VersError::InvalidCharacters("blub".to_string()))]
    fn parse_with_disallowed_characters_should_fail(
        #[case] input: &str,
        #[case] expected: VersError,
    ) {
        let result: Result<DynamicVersionRange, VersError> = input.parse();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(discriminant(&err), discriminant(&expected));
    }

    #[rstest]
    #[case::invalid_uppercase_scheme("Vers:semver/*", VersError::InvalidScheme)]
    #[case::invalid_uppercase_type("vers:Semver/*", VersError::UnknownVersioningScheme("Unknown versioning scheme: Semver".to_string()))]
    #[case::semver_missing_patch("vers:semver/1.2", VersError::InvalidVersionFormat("semver".to_string(), "1.2".to_string(), "invalid version format: expected major.minor.patch".to_string()))]
    #[case::semver_leading_zeroes("vers:semver/01.02", VersError::InvalidVersionFormat("semver".to_string(), "01.02".to_string(), "invalid version format: expected major.minor.patch".to_string()))]
    #[case::semver_empty_prerelease("vers:semver/1.2-", VersError::InvalidVersionFormat("semver".to_string(), "1.2-".to_string(), "invalid version format: expected major.minor.patch".to_string()))]
    #[case::npm_missing_patch("vers:npm/1.2", VersError::InvalidVersionFormat("npm".to_string(), "1.2".to_string(), "invalid version format: expected major.minor.patch".to_string()))]
    #[case::npm_extra_segment("vers:npm/1.2.3.4", VersError::InvalidVersionFormat("npm".to_string(), "1.2.3.4".to_string(), "invalid version format: expected major.minor.patch".to_string()))]
    #[case::npm_v_prefix("vers:npm/v1.2.3.4", VersError::InvalidVersionFormat("npm".to_string(), "v1.2.3.4".to_string(), "invalid version format: expected major.minor.patch".to_string()))]
    #[case::deb_underscore("vers:deb/1.2_build", VersError::InvalidVersionFormat("deb".to_string(), "1.2_build".to_string(), "invalid version format: expected major.minor.patch".to_string()))]
    #[case::cargo_missing_patch("vers:cargo/1.2", VersError::InvalidVersionFormat("cargo".to_string(), "1.2".to_string(), "invalid version format: expected major.minor.patch".to_string()))]
    fn non_canonical_version_should_fail(#[case] input: &str, #[case] expected: VersError) {
        let result: Result<DynamicVersionRange, VersError> = input.parse();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(discriminant(&err), discriminant(&expected));
    }

    #[test]
    fn duplicate_versions_are_not_allowed() {
        let result: Result<DynamicVersionRange, VersError> =
            "vers:npm/<1.2.3|1.2.3|<2.0.0|2.0.0".parse();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, VersError::DuplicateVersions(_)));
        assert!(err.to_string().contains("1.2.3"));
        assert!(err.to_string().contains("2.0.0"));
    }

    #[test]
    fn unordered_versions_are_not_allowed() {
        let result: Result<DynamicVersionRange, VersError> = "vers:semver/2.0.0|1.0.0".parse();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, VersError::UnorderedVersions(_, _)));
    }

    #[test]
    fn comparators_within_versions_are_not_allowed() {
        let result: Result<DynamicVersionRange, VersError> = "vers:semver/>2.0>1".parse();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_matches!(err, VersError::UnencodedOperator(_));
    }

    #[test]
    fn missing_version_is_not_allowed() {
        let result: Result<DynamicVersionRange, VersError> = "vers:semver/>".parse();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_matches!(err, VersError::MissingVersionInConstraint);
    }

    #[test]
    fn invalid_url_encoding_should_fail() {
        let result: Result<DynamicVersionRange, VersError> = "vers:semver/>2.0%3X1".parse();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_matches!(err, VersError::InvalidConstraint(_));
    }

    #[test]
    fn percent_encoded_whitespace_other_than_space_should_fail() {
        let result: Result<DynamicVersionRange, VersError> = "vers:cargo/%091.2.3".parse();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_matches!(err, VersError::InvalidConstraint(_));
    }

    #[test]
    fn multiple_asterisks_are_not_allowed() {
        let result: Result<DynamicVersionRange, VersError> = "vers:semver/1.0.*|2.*".parse();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, VersError::MultipleAsterisks(_)));
    }

    #[rstest]
    #[case::asterisk_at_first_position("vers:semver/*|1.2.3")]
    #[case::asterisk_at_last_position("vers:semver/1.2.3|*")]
    fn asterisk_with_other_constraints_is_not_allowed(#[case] input: &str) {
        let result: Result<DynamicVersionRange, VersError> = input.parse();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, VersError::InvalidRange(_)));
    }

    #[rstest]
    #[case::equal_lesser("vers:semver/1.0.0|<2.0.0")]
    #[case::equal_lesser_equal("vers:semver/1.0.0|<=2.0.0")]
    #[case::equal_non_equal_lesser("vers:semver/1.0.0|!=1.2.0|<2.0.0")]
    #[case::equal_non_equal_lesser_equal("vers:semver/1.0.0|!=1.2.0|<=2.0.0")]
    #[case::equal_equal_lesser("vers:semver/1.0.0|1.2.0|<2.0.0")]
    #[case::equal_equal_lesser_equal("vers:semver/1.0.0|1.2.0|<=2.0.0")]
    fn equal_followed_by_unallowed_successor_should_fail(#[case] input: &str) {
        let result: Result<DynamicVersionRange, VersError> = input.parse();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, VersError::ConsecutiveOperators(_)));
    }

    #[rstest]
    #[case("vers:semver/>1.0.0|1.5.0")]
    #[case("vers:semver/>1.0.0|1.5.0|>2.0.0")]
    #[case("vers:semver/1.0.0|>1.5.0|2.0.0|>2.5.0")]
    fn greater_followed_by_equals_should_fail(#[case] input: &str) {
        let result: Result<DynamicVersionRange, VersError> = input.parse();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, VersError::ConsecutiveOperators(_)));
    }

    #[rstest]
    #[case("vers:semver/>1.0.0|!=1.5.0|>2.0.0")]
    #[case("vers:semver/<1.0.0|<2.0.0")]
    #[case("vers:semver/<1.0.0|!=1.5.0|<2.0.0")]
    fn greater_or_lesser_should_not_be_followed_by_another_greater_or_lesser(#[case] input: &str) {
        let result: Result<DynamicVersionRange, VersError> = input.parse();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, VersError::ConsecutiveOperators(_)));
    }

    #[test]
    fn equal_followed_by_equal_is_allowed() {
        let result: Result<DynamicVersionRange, VersError> = "vers:semver/1.0.0|1.5.0".parse();
        assert!(result.is_ok());
    }

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
    fn test_is_open_ended_via_trait_default() {
        let open: DynamicVersionRange = "vers:npm/>=1.0.0".parse().unwrap();
        assert!(open.is_open_ended());

        let bounded: DynamicVersionRange = "vers:npm/>=1.0.0|<2.0.0".parse().unwrap();
        assert!(!bounded.is_open_ended());

        let star: DynamicVersionRange = "vers:npm/*".parse().unwrap();
        assert!(!star.is_open_ended());
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
        // "pypi" is a valid VERS scheme per the spec, but not yet implemented.
        let range: Result<DynamicVersionRange, VersError> = "vers:pypi/>=1.0.0|<2.0.0".parse();
        assert!(range.is_err());
        assert!(matches!(
            range.unwrap_err(),
            VersError::UnsupportedVersioningScheme(_)
        ));
    }

    #[test]
    fn test_dynamic_parse_unknown() {
        // "not-a-scheme" is not defined by the VERS spec at all.
        let range: Result<DynamicVersionRange, VersError> =
            "vers:not-a-scheme/>=1.0.0|<2.0.0".parse();
        assert!(range.is_err());
        assert!(matches!(
            range.unwrap_err(),
            VersError::UnknownVersioningScheme(_)
        ));
    }

    #[test]
    fn test_parse_native_unknown_scheme() {
        let result = DynamicVersionRange::parse_native("not-a-scheme", ">=1.0.0");
        assert!(matches!(result, Err(VersError::UnknownVersioningScheme(_))));
    }

    #[test]
    fn test_parse_native_unsupported_scheme() {
        let result = DynamicVersionRange::parse_native("pypi", ">=1.0.0");
        assert!(matches!(
            result,
            Err(VersError::UnsupportedVersioningScheme(_))
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
    fn test_dynamic_serde_json_roundtrip() {
        for input in [
            "vers:npm/>=1.0.0|<2.0.0",
            // Debian version format requires a complete version including revision (e.g., 1.0-0)
            "vers:deb/>=1.0-0",
            "vers:cargo/>=1.0.0|<2.0.0",
            "vers:intdot/>=1.2|<2.0",
            "vers:intdot/>=1.2beta|<2.0alpha",
        ] {
            let range: DynamicVersionRange = input.parse().unwrap();
            let json = serde_json::to_string(&range).unwrap();
            let roundtripped: DynamicVersionRange = serde_json::from_str(&json).unwrap();
            assert_eq!(range, roundtripped);
            assert_eq!(input, range.to_string());
        }
    }

    #[test]
    fn test_dynamic_serde_json_roundtrip_leading_zeros() {
        // Unlike suffixes, leading zeros are normalized away at the *first*
        // parse (per spec, `01.002` is non-canonical spelling of `1.2`), so
        // the exact input spelling is not preserved. The parsed range is
        // still stable across serde and re-parsing roundtrips.
        let range: DynamicVersionRange = "vers:intdot/>=01.2|<2.0".parse().unwrap();
        assert_eq!(range.to_string(), "vers:intdot/>=1.2|<2.0");

        let json = serde_json::to_string(&range).unwrap();
        let roundtripped: DynamicVersionRange = serde_json::from_str(&json).unwrap();
        assert_eq!(range, roundtripped);
        assert_eq!(roundtripped.to_string(), "vers:intdot/>=1.2|<2.0");

        // Re-parsing the normalized display form is a fixpoint: no further
        // drift is possible.
        let reparsed: DynamicVersionRange = roundtripped.to_string().parse().unwrap();
        assert_eq!(roundtripped, reparsed);
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

    #[rstest]
    #[case("vers:npm/*")]
    #[case("vers:deb/*")]
    #[case("vers:cargo/*")]
    #[case("vers:intdot/*")]
    fn test_contains_star(#[case] range: &str) {
        let range: DynamicVersionRange = range.parse().unwrap();
        assert!(range.contains("1.0.0".to_string()).unwrap());
        assert!(range.contains("2.0.0".to_string()).unwrap());
        assert!(range.contains("0.0.1".to_string()).unwrap());
        assert!(range.contains("999.999.999".to_string()).unwrap());
    }

    #[test]
    fn test_contains_not_equal() {
        let range: DynamicVersionRange = "vers:npm/!=1.2.3".parse().unwrap();
        assert!(!range.contains("1.2.3".to_string()).unwrap());
        assert!(range.contains("1.2.4".to_string()).unwrap());
    }

    #[test]
    fn test_contains_complex() {
        let range: DynamicVersionRange = "vers:npm/>=1.0.0|!=1.5.0|<2.0.0".parse().unwrap();
        assert!(range.contains("1.0.0".to_string()).unwrap());
        assert!(range.contains("1.7.0".to_string()).unwrap());
        assert!(!range.contains("1.5.0".to_string()).unwrap());
        assert!(!range.contains("2.0.0".to_string()).unwrap());
        assert!(!range.contains("0.9.0".to_string()).unwrap());
    }

    #[test]
    fn test_parse_native_preserves_npm_scheme() {
        // The "npm" label must be preserved, not converted to "semver"
        let range = DynamicVersionRange::parse_native("npm", ">=1.0.0|<2.0.0").unwrap();
        assert_eq!(range.versioning_scheme(), "npm");
    }

    #[test]
    fn test_parse_native_preserves_semver_scheme() {
        let range = DynamicVersionRange::parse_native("semver", ">=1.0.0|<2.0.0").unwrap();
        assert_eq!(range.versioning_scheme(), "semver");
    }

    #[rstest]
    #[case(" >=1.0.0")]
    #[case(">= 1.0.0")]
    #[case(">=1.0.0 ")]
    #[case(">=1.0.0| <2.0.0")]
    #[case(">=1.0.0 | <2.0.0")]
    #[case(">=1.0.0 | <2.0.0 ")]
    #[case(">=1.0.0 | < 2.0.0 ")]
    fn test_parse_native_npm_accepts_whitespaces(#[case] input: &str) {
        let range = DynamicVersionRange::parse_native("npm", input).unwrap();
        assert_eq!(range.versioning_scheme(), "npm");
    }

    #[test]
    fn test_parse_native_normalizes() {
        // parse_native should run normalize_and_validate:
        // >1.0.0|>2.0.0 simplifies to >1.0.0
        let range = DynamicVersionRange::parse_native("npm", ">1.0.0|>2.0.0").unwrap();
        assert_eq!(range.constraints().len(), 1);
        assert_eq!(range.constraints()[0].comparator, Comparator::GreaterThan);
        assert_eq!(range.constraints()[0].version.to_string(), "1.0.0");
    }

    #[test]
    fn test_parse_native_roundtrip() {
        let range = DynamicVersionRange::parse_native("npm", ">=1.0.0|<2.0.0").unwrap();
        assert_eq!(range.to_string(), "vers:npm/>=1.0.0|<2.0.0");
    }

    #[test]
    fn parse_native_deb_without_equals_should_fail() {
        let result = DynamicVersionRange::parse_native("deb", "1.0");
        assert!(result.is_err());
    }

    #[cfg(feature = "wasm")]
    #[test]
    fn test_cached_constraints_excluded_from_ts_decl() {
        use tsify::Tsify;

        assert!(!DynamicVersionRange::DECL.contains("cached_constraints"));
    }

    #[cfg(feature = "wasm")]
    #[test]
    fn test_ts_decl_matches_serialized_union_shape() {
        use tsify::Tsify;

        assert!(
            DynamicVersionRange::DECL.contains(
                "export type DynamicVersionRange = VersVersionRange<SemVer> | VersVersionRange<DebVersion> | VersVersionRange<CargoVersion> | VersVersionRange<IntDotVersion>;"
            )
        );
    }
}
