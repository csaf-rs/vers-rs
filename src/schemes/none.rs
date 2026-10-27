//! The empty `none` VERS scheme.
//!
//! The VERS specification defines `vers:none/*` as the sole valid form for a
//! range containing no versions.

use crate::range::VersionRange;
use crate::range::wildcard::{RawWildcardRange, WildcardRange};
use crate::{VersError, VersionConstraint};
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

pub const NONE_SCHEME: &str = "none";
const INVALID_RANGE_ERROR: &str = "the none scheme only accepts the '*' constraint";

/// A range that contains no candidate versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "RawWildcardRange")]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct NoneVersionRange;

impl TryFrom<RawWildcardRange> for NoneVersionRange {
    type Error = VersError;

    fn try_from(raw: RawWildcardRange) -> Result<Self, Self::Error> {
        WildcardRange::validate_raw(raw, NONE_SCHEME, INVALID_RANGE_ERROR)?;
        Ok(Self)
    }
}

impl NoneVersionRange {
    /// Parse the constraint portion of a `none` range.
    pub fn parse(raw: &str) -> Result<Self, VersError> {
        WildcardRange::parse_constraint(raw, INVALID_RANGE_ERROR)?;
        Ok(Self)
    }
}

impl Serialize for NoneVersionRange {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        WildcardRange::serialize(serializer, NONE_SCHEME)
    }
}

impl FromStr for NoneVersionRange {
    type Err = VersError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        WildcardRange::parse_vers(s, NONE_SCHEME, INVALID_RANGE_ERROR).map(|()| Self)
    }
}

impl VersionRange<String> for NoneVersionRange {
    fn versioning_scheme(&self) -> &str {
        NONE_SCHEME
    }

    fn contains(&self, _: String) -> Result<bool, VersError> {
        Ok(false)
    }

    fn constraints(&self) -> &Vec<VersionConstraint<String>> {
        WildcardRange::constraints()
    }
}

impl Display for NoneVersionRange {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "vers:none/*")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_wildcard_is_valid() {
        assert!(NoneVersionRange::parse("*").is_ok());
        assert!(matches!(
            NoneVersionRange::parse("1.0.0"),
            Err(VersError::InvalidRange(_))
        ));
    }

    #[test]
    fn parses_canonical_vers_string() {
        let range: NoneVersionRange = "vers:none/*".parse().unwrap();
        assert_eq!(range.to_string(), "vers:none/*");
    }

    #[test]
    fn serde_rejects_noncanonical_range() {
        assert!(
            serde_json::from_str::<NoneVersionRange>(
                r#"{"versioning_scheme":"none","constraints":[]}"#
            )
            .is_err()
        );
    }
}
