use crate::VersionConstraint;
use crate::constraint::NativeVersionConverter;
use crate::enums::comparator::Comparator;
use crate::{VersError, VersioningScheme};
use maven_version_range::MavenVersionRange;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ordering;
use std::fmt::{self, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::str::FromStr;

#[derive(Clone, Debug, Eq)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct MavenVersion {
    raw: String,
}

impl Default for MavenVersion {
    fn default() -> Self {
        MavenVersion {
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
    fn scheme_name() -> &'static str {
        VersioningScheme::Maven.as_ref()
    }

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

        // If it is an official Maven range in parentheses ([...], (...))
        if raw.starts_with('[') || raw.starts_with('(') {
            let _parsed_range = MavenVersionRange::from_str(raw).map_err(|e| {
                VersError::InvalidConstraint(format!(
                    "Failed to parse Maven version range '{}': {}",
                    raw, e
                ))
            })?;

            // Since `maven-version-range` parses the string, we can extract the bounds for `vers-rs`
            // or map them as equivalent constraints. Alternatively, we can parse the comma-separated parts directly.
            let inner_content = &raw[1..raw.len() - 1];
            let parts: Vec<&str> = inner_content.split(',').map(|s| s.trim()).collect();

            let mut constraints = Vec::new();
            if let Some(lower) = parts.first()
                && !lower.is_empty()
            {
                let inclusive = raw.starts_with('[');
                let comp = if inclusive {
                    Comparator::GreaterThanOrEqual
                } else {
                    Comparator::GreaterThan
                };
                constraints.push(VersionConstraint::new(comp, MavenVersion::from_str(lower)?));
            }
            if let Some(upper) = parts.get(1)
                && !upper.is_empty()
            {
                let inclusive = raw.ends_with(']');
                let comp = if inclusive {
                    Comparator::LessThanOrEqual
                } else {
                    Comparator::LessThan
                };
                constraints.push(VersionConstraint::new(comp, MavenVersion::from_str(upper)?));
            }

            if constraints.is_empty() {
                return Err(VersError::EmptyConstraints);
            }
            return Ok(constraints);
        }

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

        let parsed_version = MavenVersion {
            raw: version_str.to_string(),
        };

        Ok(VersionConstraint::new(comparator, parsed_version))
    }
}

impl FromStr for MavenVersion {
    type Err = VersError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        Ok(MavenVersion { raw: s.to_string() })
    }
}

impl Display for MavenVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

impl PartialEq for MavenVersion {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl Hash for MavenVersion {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}

impl PartialOrd for MavenVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MavenVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        // This is where the official validation via `MavenVersionRange` comes into play.
        let range_self = MavenVersionRange::from_str(&self.raw);
        let range_other = MavenVersionRange::from_str(&other.raw);

        match (range_self, range_other) {
            (Ok(a), Ok(b)) => format!("{:?}", a).cmp(&format!("{:?}", b)),
            _ => self.raw.cmp(&other.raw),
        }
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
