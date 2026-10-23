use crate::VersError;
use crate::VersionConstraint;
use crate::comparator::Comparator;
use crate::constraint::NativeVersionConverter;
use maven_version_range::{Version as MavenVersionInner, VersionRange as MavenRangeInner};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ordering;
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

pub const MAVEN_SCHEME: &str = "maven";

#[derive(Clone, Debug, Eq)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct MavenVersion {
    inner: MavenVersionInner,
    raw: String,
}

impl Default for MavenVersion {
    fn default() -> Self {
        MavenVersion {
            inner: MavenVersionInner::from_str("0").unwrap(),
            raw: "0".to_string(),
        }
    }
}

impl Serialize for MavenVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.raw)
    }
}

impl<'de> Deserialize<'de> for MavenVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl NativeVersionConverter for MavenVersion {
    const SCHEME_NAME: &'static str = MAVEN_SCHEME;

    fn from_native(raw: &str) -> Result<Vec<VersionConstraint<Self>>, VersError> {
        if raw.bytes().any(|b| b == b'\t' || b == b'\n' || b == b'\r') {
            return Err(VersError::InvalidConstraint(
                "Control characters are not permitted in native version ranges".to_string(),
            ));
        }
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(VersError::EmptyConstraints);
        }

        // Maven native ranges often use comma-separated clauses or Maven range syntax like [1.0,2.0)
        // If it's a Maven range bracket expression, we can parse it directly or map clauses.
        if raw.starts_with('[') || raw.starts_with('(') {
            let range_inner = MavenRangeInner::from_str(raw).map_err(|e| {
                VersError::InvalidConstraint(format!(
                    "Failed to parse Maven version range '{}': {}",
                    raw, e
                ))
            })?;

            // Convert Maven range bounds into standard vers VersionConstraints
            // Maven ranges define lower/upper bounds with inclusive/exclusive flags.
            let mut constraints = Vec::new();
            if let Some(lower) = range_inner.lower_bound() {
                let comp = if range_inner.is_lower_inclusive() {
                    Comparator::GreaterThanOrEqual
                } else {
                    Comparator::GreaterThan
                };
                let v = MavenVersion::from_str(&lower.to_string())?;
                constraints.push(VersionConstraint::new(comp, v));
            }
            if let Some(upper) = range_inner.upper_bound() {
                let comp = if range_inner.is_upper_inclusive() {
                    Comparator::LessThanOrEqual
                } else {
                    Comparator::LessThan
                };
                let v = MavenVersion::from_str(&upper.to_string())?;
                constraints.push(VersionConstraint::new(comp, v));
            }

            if constraints.is_empty() {
                return Err(VersError::EmptyConstraints);
            }
            return Ok(constraints);
        }

        // Fallback to standard clause splitting if not bracketed
        let clauses: Vec<&str> = if raw.contains(',') {
            raw.split(',').collect()
        } else {
            vec![raw]
        };

        let mut constraints = Vec::new();
        for clause in clauses {
            let constraint = Self::from_native_constraint(clause.trim())?;
            constraints.push(constraint);
        }

        if constraints.is_empty() {
            return Err(VersError::EmptyConstraints);
        }

        Ok(constraints)
    }

    fn from_native_constraint(raw: &str) -> Result<VersionConstraint<Self>, VersError> {
        let raw = raw.trim();

        if raw.is_empty() {
            return Err(VersError::InvalidConstraint("Empty constraint".to_string()));
        }

        let (comparator, version_str) = if let Some(stripped) = raw.strip_prefix(">=") {
            (Comparator::GreaterThanOrEqual, stripped)
        } else if let Some(stripped) = raw.strip_prefix("<=") {
            (Comparator::LessThanOrEqual, stripped)
        } else if let Some(stripped) = raw.strip_prefix('>') {
            (Comparator::GreaterThan, stripped)
        } else if let Some(stripped) = raw.strip_prefix('<') {
            (Comparator::LessThan, stripped)
        } else if let Some(stripped) = raw.strip_prefix("==") {
            (Comparator::Equal, stripped)
        } else if let Some(stripped) = raw.strip_prefix('=') {
            (Comparator::Equal, stripped)
        } else {
            (Comparator::Equal, raw)
        };

        let version_str = version_str.trim();
        if version_str.is_empty() {
            return Err(VersError::InvalidConstraint("Missing version".to_string()));
        }

        let inner = MavenVersionInner::from_str(version_str).map_err(|e| {
            VersError::InvalidConstraint(format!(
                "Failed to parse version '{}': {}",
                version_str, e
            ))
        })?;

        let parsed_version = MavenVersion {
            inner,
            raw: version_str.to_string(),
        };

        Ok(VersionConstraint::new(comparator, parsed_version))
    }
}

impl FromStr for MavenVersion {
    type Err = VersError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let original_raw = s.to_string();

        let inner = MavenVersionInner::from_str(s).map_err(|e| {
            VersError::InvalidVersionFormat(MAVEN_SCHEME.to_string(), s.to_string(), e.to_string())
        })?;

        Ok(MavenVersion {
            inner,
            raw: original_raw,
        })
    }
}

impl Display for MavenVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

impl PartialEq for MavenVersion {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl PartialOrd for MavenVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MavenVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        self.inner.cmp(&other.inner)
    }
}

#[cfg(test)]
mod tests {
    use super::MavenVersion;
    use crate::Comparator;
    use crate::range::VersionRange;
    use crate::range::dynamic::DynamicVersionRange;
    use std::str::FromStr;

    #[test]
    fn test_maven_parse_simple() {
        let range = DynamicVersionRange::parse_native("maven", "1.0.0").unwrap();
        assert_eq!(range.versioning_scheme(), "maven");
        assert_eq!(range.constraints().len(), 1);
        assert_eq!(range.constraints()[0].comparator, Comparator::Equal);
        assert_eq!(range.constraints()[0].version.to_string(), "1.0.0");
    }

    #[test]
    fn test_maven_version_ordering() {
        let v1 = MavenVersion::from_str("1.0.0").unwrap();
        let v2 = MavenVersion::from_str("1.1.0").unwrap();
        assert!(v1 < v2);
    }
}