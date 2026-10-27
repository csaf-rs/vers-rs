//! The universal `all` VERS scheme.
//!
//! The VERS specification defines `vers:all/*` as the sole valid form for a
//! range containing every version.

use crate::range::VersionRange;
use crate::range::wildcard::{RawWildcardRange, WildcardRange};
use crate::{VersError, VersionConstraint};
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

pub const ALL_SCHEME: &str = "all";
const INVALID_RANGE_ERROR: &str = "the all scheme only accepts a single '*' constraint";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "RawWildcardRange")]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct AllVersionRange;

impl AllVersionRange {
    pub fn parse(raw: &str) -> Result<Self, VersError> {
        WildcardRange::parse_constraint(raw, INVALID_RANGE_ERROR)?;
        Ok(Self)
    }
}

impl TryFrom<RawWildcardRange> for AllVersionRange {
    type Error = VersError;

    fn try_from(raw: RawWildcardRange) -> Result<Self, Self::Error> {
        WildcardRange::validate_raw(raw, ALL_SCHEME, INVALID_RANGE_ERROR)?;
        Ok(Self)
    }
}

impl Serialize for AllVersionRange {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        WildcardRange::serialize(serializer, ALL_SCHEME)
    }
}

impl FromStr for AllVersionRange {
    type Err = VersError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        WildcardRange::parse_vers(s, ALL_SCHEME, INVALID_RANGE_ERROR).map(|()| Self)
    }
}

impl VersionRange<String> for AllVersionRange {
    fn versioning_scheme(&self) -> &str {
        ALL_SCHEME
    }

    fn contains(&self, _: String) -> Result<bool, VersError> {
        Ok(true)
    }

    fn constraints(&self) -> &Vec<VersionConstraint<String>> {
        WildcardRange::constraints()
    }
}

impl Display for AllVersionRange {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "vers:all/*")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_wildcard_is_valid() {
        assert!(AllVersionRange::parse("*").is_ok());
        assert!(matches!(
            AllVersionRange::parse("1.0.0"),
            Err(VersError::InvalidRange(_))
        ));
    }

    #[test]
    fn parses_canonical_vers_string() {
        let range: AllVersionRange = "vers:all/*".parse().unwrap();
        assert_eq!(range.to_string(), "vers:all/*");
    }

    #[test]
    fn serde_rejects_noncanonical_range() {
        assert!(
            serde_json::from_str::<AllVersionRange>(
                r#"{"versioning_scheme":"all","constraints":[]}"#
            )
            .is_err()
        );
    }
}
