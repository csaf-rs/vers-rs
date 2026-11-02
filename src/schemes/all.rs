//! The universal `all` VERS scheme.
//!
//! The VERS specification defines `vers:all/*` as the sole valid form for a
//! range containing every version.

use crate::{NativeVersionConverter, VersError, VersionConstraint, VersioningScheme};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

const INVALID_RANGE_ERROR: &str = "the all scheme only accepts a single '*' constraint";
const WILDCARD: &str = "*";

#[derive(Debug, Clone, Hash, PartialOrd, Ord, PartialEq, Eq)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
// The wire form is the wildcard string, so override tsify's unit-struct
// rendering (`null`) with a string literal type.
#[cfg_attr(feature = "wasm", tsify(type = r#""*""#))]
pub struct AllVersion;

impl Default for AllVersion {
    fn default() -> Self {
        AllVersion
    }
}

/// Serializes as the canonical native form: the wildcard string `"*"`, so a
/// `VersVersionRange<AllVersion>` keeps a plain string `version` field like
/// every other scheme.
impl Serialize for AllVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(WILDCARD)
    }
}

/// Accepts only the canonical wildcard `"*"`; any other string (or `null`) is
/// a non-canonical spelling and is rejected.
impl<'de> Deserialize<'de> for AllVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::from_str(&raw).map_err(serde::de::Error::custom)
    }
}

impl Display for AllVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "")
    }
}

impl NativeVersionConverter for AllVersion {
    fn scheme_name() -> &'static str {
        VersioningScheme::All.as_ref()
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
                version: AllVersion,
            }]))
        }
    }
}

impl FromStr for AllVersion {
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
    use super::*;

    #[test]
    fn only_wildcard_is_valid() {
        assert!(AllVersion::from_str("*").is_ok());
        assert!(matches!(
            AllVersion::from_str("1.0.0"),
            Err(VersError::InvalidVersionFormat(_, _, _))
        ));
    }

    #[test]
    fn serde_rejects_noncanonical_range() {
        assert!(
            serde_json::from_str::<AllVersion>(r#"{"versioning_scheme":"all","constraints":[]}"#)
                .is_err()
        );
    }

    #[test]
    fn serde_accepts_only_canonical_wildcard() {
        let version: AllVersion = serde_json::from_str(r#""*""#).unwrap();
        assert_eq!(serde_json::to_string(&version).unwrap(), r#""*""#);

        assert!(serde_json::from_str::<AllVersion>(r#""1.0.0""#).is_err());
        assert!(serde_json::from_str::<AllVersion>("null").is_err());
    }
}

#[cfg(all(test, feature = "wasm"))]
mod tsify_decl_check {
    use super::AllVersion;
    use tsify::Tsify;

    /// The wire form is the wildcard string, not the `null` tsify would emit
    /// for a unit struct without an override.
    #[test]
    fn all_version_ts_decl_is_wildcard_string() {
        assert_eq!(AllVersion::DECL, r#"export type AllVersion = "*";"#);
    }
}
