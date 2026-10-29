//! The universal `all` VERS scheme.
//!
//! The VERS specification defines `vers:all/*` as the sole valid form for a
//! range containing every version.

use crate::{NativeVersionConverter, VersError, VersionConstraint, VersioningScheme};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

const INVALID_RANGE_ERROR: &str = "the all scheme only accepts a single '*' constraint";

#[derive(Debug, Clone, Hash, PartialOrd, Ord, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct AllVersion;

impl Default for AllVersion {
    fn default() -> Self {
        AllVersion
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
        if raw != "*" {
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
}
