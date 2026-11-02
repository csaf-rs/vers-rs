//! The empty `none` VERS scheme.
//!
//! The VERS specification defines `vers:none/*` as the sole valid form for a
//! range containing no versions.

use crate::{NativeVersionConverter, VersError, VersionConstraint, VersioningScheme};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

const INVALID_RANGE_ERROR: &str = "the none scheme only accepts a single '*' constraint";
const WILDCARD: &str = "*";

/// A range that contains no candidate versions.
#[derive(Debug, Clone, Hash, PartialOrd, Ord, PartialEq, Eq)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
// The wire form is the wildcard string, so override tsify's unit-struct
// rendering (`null`) with a string literal type.
#[cfg_attr(feature = "wasm", tsify(type = r#""*""#))]
pub struct NoneVersion;

impl Default for NoneVersion {
    fn default() -> Self {
        NoneVersion
    }
}

/// Serializes as the canonical native form: the wildcard string `"*"`, so a
/// `VersVersionRange<NoneVersion>` keeps a plain string `version` field like
/// every other scheme.
impl Serialize for NoneVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(WILDCARD)
    }
}

/// Accepts only the canonical wildcard `"*"`; any other string (or `null`) is
/// a non-canonical spelling and is rejected.
impl<'de> Deserialize<'de> for NoneVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::from_str(&raw).map_err(serde::de::Error::custom)
    }
}

impl Display for NoneVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "*")
    }
}

impl NativeVersionConverter for NoneVersion {
    fn scheme_name() -> &'static str {
        VersioningScheme::None.as_ref()
    }

    fn versioning_schemes() -> Option<&'static [VersioningScheme]> {
        Some(&[VersioningScheme::None])
    }

    fn from_native(raw: &str) -> Result<Vec<VersionConstraint<Self>>, VersError> {
        if raw
            .bytes()
            .any(|b| b != b' ' && crate::util::ascii_whitespace::is_ascii_vers_whitespace(b))
        {
            return Err(VersError::InvalidCharacters(
                "Control characters (tabs, newlines, carriage returns) are not permitted"
                    .to_string(),
            ));
        }
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(VersError::EmptyConstraints);
        }
        if raw != WILDCARD {
            Err(VersError::InvalidVersionFormat(
                Self::scheme_name().to_string(),
                raw.to_string(),
                INVALID_RANGE_ERROR.to_string(),
            ))
        } else {
            Ok(Vec::from([VersionConstraint {
                comparator: crate::Comparator::Any,
                version: NoneVersion,
            }]))
        }
    }
}

impl FromStr for NoneVersion {
    type Err = VersError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == WILDCARD {
            Ok(Self)
        } else {
            Err(VersError::InvalidVersionFormat(
                Self::scheme_name().to_string(),
                s.into(),
                INVALID_RANGE_ERROR.into(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use super::*;

    #[test]
    fn only_wildcard_is_valid() {
        assert!(NoneVersion::from_str("*").is_ok());
        assert!(matches!(
            NoneVersion::from_str("1.0.0"),
            Err(VersError::InvalidVersionFormat(_, _, _))
        ));
    }

    #[test]
    fn serde_rejects_noncanonical_range() {
        assert!(
            serde_json::from_str::<NoneVersion>(r#"{"versioning_scheme":"none","constraints":[]}"#)
                .is_err()
        );
    }

    #[test]
    fn serde_accepts_only_canonical_wildcard() {
        let version: NoneVersion = serde_json::from_str(r#""*""#).unwrap();
        assert_eq!(serde_json::to_string(&version).unwrap(), r#""*""#);

        assert!(serde_json::from_str::<NoneVersion>(r#""1.0.0""#).is_err());
        assert!(serde_json::from_str::<NoneVersion>("null").is_err());
    }

    // The typed map form must pair the version type with its own scheme:
    // `NoneVersion` only implements the "none" scheme.
    #[test]
    fn typed_serde_rejects_scheme_type_mismatch() {
        let result = serde_json::from_str::<crate::VersVersionRange<NoneVersion>>(
            r#"{"versioning_scheme":"deb","constraints":[{"comparator":"Any","version":"*"}]}"#,
        );
        assert_matches!(
            result,
            Err(err) if err.to_string().contains("expected one of: none")
        );
    }

    // `vers:none/*` is the only canonical form: the payload-less wildcard
    // version rejects any comparator other than `Any`.
    #[test]
    fn typed_serde_rejects_non_wildcard_comparator() {
        let result = serde_json::from_str::<crate::VersVersionRange<NoneVersion>>(
            r#"{"versioning_scheme":"none","constraints":[{"comparator":"LessThan","version":"*"}]}"#,
        );
        assert_matches!(
            result,
            Err(err) if err.to_string().contains("the none scheme only accepts a single '*' constraint")
        );
    }
}

#[cfg(all(test, feature = "wasm"))]
mod tsify_decl_check {
    use super::NoneVersion;
    use tsify::Tsify;

    /// The wire form is the wildcard string, not the `null` tsify would emit
    /// for a unit struct without an override.
    #[test]
    fn none_version_ts_decl_is_wildcard_string() {
        assert!(NoneVersion::DECL.ends_with(r#"export type NoneVersion = "*";"#));
    }
}
