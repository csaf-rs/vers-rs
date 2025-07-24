use derive_more::{Display, FromStr};
use semver::Version;
use std::cmp::Ordering;

#[derive(Display, FromStr, Clone, Debug, PartialEq, Eq, PartialOrd)]
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