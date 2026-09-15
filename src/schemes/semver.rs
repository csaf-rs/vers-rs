use crate::VersError;
use crate::constraint::NativeVersionConverter;
use derive_more::Display;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::str::FromStr;

pub const SEMVER_SCHEME: &str = "semver/npm";

#[derive(Display, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
// `Version` comes from the external `semver` crate and doesn't implement `Tsify`, so the
// derive can't generate a meaningful TS type for it (it would emit an unresolved `Version`
// reference). `semver::Version` serializes as a plain string (see its `Serialize` impl), so
// override the generated TS type to match what actually crosses the ABI.
#[cfg_attr(feature = "wasm", tsify(type = "string"))]
pub struct SemVer(Version);

impl NativeVersionConverter for SemVer {
    const SCHEME_NAME: &'static str = "semver";
}

impl Default for SemVer {
    fn default() -> Self {
        SemVer(Version::new(0, 0, 0))
    }
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }

    fn max(self, other: Self) -> Self
    where
        Self: Sized,
    {
        if self.0 >= other.0 { self } else { other }
    }

    fn min(self, other: Self) -> Self
    where
        Self: Sized,
    {
        if self.0 <= other.0 { self } else { other }
    }

    fn clamp(self, min: Self, max: Self) -> Self
    where
        Self: Sized,
    {
        if self.0 < min.0 {
            min
        } else if self.0 > max.0 {
            max
        } else {
            self
        }
    }
}

impl FromStr for SemVer {
    type Err = VersError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(SemVer(Version::parse(s).map_err(|e| {
            VersError::InvalidVersionFormat(SEMVER_SCHEME.to_string(), s.to_string(), e.to_string())
        })?))
    }
}

#[cfg(all(test, feature = "wasm"))]
mod tsify_decl_check {
    use super::SemVer;
    use tsify::Tsify;

    #[test]
    fn semver_ts_decl_is_string() {
        assert_eq!(SemVer::DECL, "export type SemVer = string;");
    }
}
