use crate::VersError;
use derive_more::Display;
use semver::Version;
use std::cmp::Ordering;
use std::str::FromStr;

pub static SEMVER_SCHEME: &str = "semver/npm";

#[derive(Display, Clone, Debug, PartialEq, Eq, PartialOrd)]
pub struct SemVer(Version);

impl Default for SemVer {
    fn default() -> Self {
        SemVer(Version::new(0, 0, 0))
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }

    fn max(self, other: Self) -> Self
    where
        Self: Sized
    {
        if self.0 >= other.0 {
            self
        } else {
            other
        }
    }

    fn min(self, other: Self) -> Self
    where
        Self: Sized
    {
        if self.0 <= other.0 {
            self
        } else {
            other
        }
    }

    fn clamp(self, min: Self, max: Self) -> Self
    where
        Self: Sized
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
        Ok(SemVer(Version::parse(s).map_err(|e| VersError::InvalidVersionFormat(
            SEMVER_SCHEME,
            s.to_string(),
            e.to_string(),
        ))?))
    }
}