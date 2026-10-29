//! The empty `none` VERS scheme.
//!
//! The VERS specification defines `vers:none/*` as the sole valid form for a
//! range containing no versions.

use crate::{NativeVersionConverter, VersError, VersionConstraint, VersioningScheme};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

const INVALID_RANGE_ERROR: &str = "the none scheme only accepts a single '*' constraint";

/// A range that contains no candidate versions.
#[derive(Debug, Clone, Hash, PartialOrd, Ord, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct NoneVersion;

impl Default for NoneVersion {
    fn default() -> Self {
        NoneVersion
    }
}

impl Display for NoneVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "")
    }
}

impl NativeVersionConverter for NoneVersion {
    fn scheme_name() -> &'static str {
        VersioningScheme::None.as_ref()
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
        if raw != "*" {
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
        if s == "*" {
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
}
