//! Version constraint type for the DateTime (RFC 3339) versioning scheme.

use crate::constraint::NativeVersionConverter;
use crate::enums::comparator::Comparator;
use crate::{VersError, VersionConstraint, VersioningScheme};
use jiff::Timestamp;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ordering;
use std::fmt::{self, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::str::FromStr;

#[derive(Clone, Debug, Eq)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "wasm", tsify(type = "string"))]
pub struct DateTimeVersion {
    timestamp: Timestamp,
    raw: String,
}

impl Default for DateTimeVersion {
    fn default() -> Self {
        DateTimeVersion {
            timestamp: Timestamp::UNIX_EPOCH,
            raw: "1970-01-01T00:00:00Z".to_string(),
        }
    }
}

impl Serialize for DateTimeVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.raw)
    }
}

impl<'de> Deserialize<'de> for DateTimeVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl NativeVersionConverter for DateTimeVersion {
    fn scheme_name() -> &'static str {
        VersioningScheme::Datetime.as_ref()
    }

    fn from_native(raw: &str) -> Result<Vec<VersionConstraint<Self>>, VersError> {
        validate_raw_text(raw)?;
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(VersError::EmptyConstraints);
        }

        let clauses: Vec<&str> = if trimmed.contains(',') {
            trimmed.split(',').collect()
        } else {
            vec![trimmed]
        };

        let mut constraints = Vec::new();
        for clause in clauses {
            let clause = clause.trim();
            if clause.is_empty() {
                continue;
            }
            constraints.push(Self::from_native_constraint(clause)?);
        }

        if constraints.is_empty() {
            return Err(VersError::EmptyConstraints);
        }

        Ok(constraints)
    }

    fn from_native_constraint(raw: &str) -> Result<VersionConstraint<Self>, VersError> {
        validate_raw_text(raw)?;
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(VersError::InvalidConstraint("Empty constraint".to_string()));
        }

        if Self::native_wildcard() == Some(trimmed) || trimmed == "==*" {
            return Ok(VersionConstraint::new(Comparator::Any, Self::default()));
        }

        let (comparator, version_str) = split_comparator_prefix(trimmed);
        let version_str = version_str.trim();
        if version_str.is_empty() {
            return Err(VersError::MissingVersionInConstraint);
        }

        let version = Self::from_str(version_str)?;
        Ok(VersionConstraint::new(comparator, version))
    }
}

impl FromStr for DateTimeVersion {
    type Err = VersError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        validate_raw_text(s)?;
        let trimmed = s.trim();
        let timestamp = Timestamp::from_str(trimmed).map_err(|e| {
            VersError::InvalidVersionFormat(
                Self::scheme_name().to_string(),
                s.to_string(),
                e.to_string(),
            )
        })?;

        Ok(DateTimeVersion {
            timestamp,
            raw: trimmed.to_string(),
        })
    }
}

impl Display for DateTimeVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

impl Hash for DateTimeVersion {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.timestamp.hash(state);
    }
}

impl PartialEq for DateTimeVersion {
    fn eq(&self, other: &Self) -> bool {
        self.timestamp == other.timestamp
    }
}

impl PartialOrd for DateTimeVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DateTimeVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        self.timestamp.cmp(&other.timestamp)
    }
}

fn validate_raw_text(s: &str) -> Result<(), VersError> {
    if s.bytes().any(|b| b == b'\t' || b == b'\n' || b == b'\r') {
        return Err(VersError::InvalidConstraint(
            "Control characters are not permitted in native version ranges".to_string(),
        ));
    }
    Ok(())
}

fn split_comparator_prefix(raw: &str) -> (Comparator, &str) {
    if let Some(rest) = raw.strip_prefix(">=") {
        (Comparator::GreaterThanOrEqual, rest)
    } else if let Some(rest) = raw.strip_prefix("<=") {
        (Comparator::LessThanOrEqual, rest)
    } else if let Some(rest) = raw.strip_prefix("!=") {
        (Comparator::NotEqual, rest)
    } else if let Some(rest) = raw.strip_prefix("==") {
        (Comparator::Equal, rest)
    } else if let Some(rest) = raw.strip_prefix('=') {
        (Comparator::Equal, rest)
    } else if let Some(rest) = raw.strip_prefix('>') {
        (Comparator::GreaterThan, rest)
    } else if let Some(rest) = raw.strip_prefix('<') {
        (Comparator::LessThan, rest)
    } else {
        (Comparator::Equal, raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::range::VersionRange;
    use crate::range::dynamic::DynamicVersionRange;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn test_datetime_from_str_and_display() {
        let v = DateTimeVersion::from_str("  2024-01-24T10:00:00.000Z  ").unwrap();
        assert_eq!(v.to_string(), "2024-01-24T10:00:00.000Z");
        assert_eq!(
            DateTimeVersion::default().to_string(),
            "1970-01-01T00:00:00Z"
        );
    }

    #[test]
    fn test_datetime_from_str_errors() {
        assert!(matches!(
            DateTimeVersion::from_str(""),
            Err(VersError::InvalidVersionFormat(_, _, _))
        ));
        assert!(matches!(
            DateTimeVersion::from_str("not-a-timestamp"),
            Err(VersError::InvalidVersionFormat(_, _, _))
        ));
        assert!(matches!(
            DateTimeVersion::from_str("2024-01-24"),
            Err(VersError::InvalidVersionFormat(_, _, _))
        ));
        assert!(matches!(
            DateTimeVersion::from_str("2024-01-24T10:00:00.000Z\n"),
            Err(VersError::InvalidConstraint(_))
        ));
        assert!(matches!(
            DateTimeVersion::from_str("2024-01-24T10:00:00.000Z\t"),
            Err(VersError::InvalidConstraint(_))
        ));
        assert!(matches!(
            DateTimeVersion::from_str("2024-01-24T10:00:00.000Z\r"),
            Err(VersError::InvalidConstraint(_))
        ));
        assert!(matches!(
            DateTimeVersion::from_str("2024-01-24T10:00:00.000z\r"),
            Err(VersError::InvalidConstraint(_))
        ));
        assert!(matches!(
            DateTimeVersion::from_str("2024-01-24t10:00:00.000Z\r"),
            Err(VersError::InvalidConstraint(_))
        ));
        assert!(matches!(
            DateTimeVersion::from_str("2024-01-24T10%3A00:00.000Z\r"),
            Err(VersError::InvalidConstraint(_))
        ));

    }

    #[test]
    fn test_datetime_ordering_and_offset_equivalence() {
        let utc = DateTimeVersion::from_str("2024-01-24T10:00:00.000Z").unwrap();
        let plus_two = DateTimeVersion::from_str("2024-01-24T12:00:00.000+02:00").unwrap();
        let minus_five = DateTimeVersion::from_str("2024-01-24T05:00:00.000-05:00").unwrap();
        let later = DateTimeVersion::from_str("2024-01-24T10:00:00.001Z").unwrap();
        let earlier = DateTimeVersion::from_str("2024-01-23T23:59:59Z").unwrap();

        assert_eq!(utc, plus_two);
        assert_eq!(utc, minus_five);
        assert!(utc <= plus_two);
        assert!(utc >= minus_five);
        assert!(earlier < utc);
        assert!(utc < later);
        assert!(later > earlier);

        let mut list = vec![later.clone(), earlier.clone(), plus_two.clone()];
        list.sort();
        assert_eq!(list, vec![earlier, plus_two, later]);
    }

    #[test]
    fn test_datetime_hash_consistency() {
        let v1 = DateTimeVersion::from_str("2024-01-24T10:00:00.000Z").unwrap();
        let v2 = DateTimeVersion::from_str("2024-01-24T12:00:00.000+02:00").unwrap();
        let v3 = DateTimeVersion::from_str("2024-01-24T10:00:01.000Z").unwrap();

        let mut h1 = std::collections::hash_map::DefaultHasher::new();
        let mut h2 = std::collections::hash_map::DefaultHasher::new();
        v1.hash(&mut h1);
        v2.hash(&mut h2);
        assert_eq!(
            std::hash::Hasher::finish(&h1),
            std::hash::Hasher::finish(&h2)
        );

        let mut set = HashSet::new();
        set.insert(v1);
        assert!(set.contains(&v2));
        assert!(!set.contains(&v3));

        let mut map = HashMap::new();
        map.insert(v2, "csaf-v1");
        assert_eq!(
            map.get(&DateTimeVersion::from_str("2024-01-24T10:00:00.000Z").unwrap()),
            Some(&"csaf-v1")
        );
    }

    #[test]
    fn test_datetime_serde_roundtrip() {
        let v = DateTimeVersion::from_str("2024-01-24T10:00:00.000Z").unwrap();
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, "\"2024-01-24T10:00:00.000Z\"");

        let decoded: DateTimeVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, v);
        assert_eq!(decoded.to_string(), "2024-01-24T10:00:00.000Z");
    }

    #[test]
    fn test_datetime_native_constraints_and_range() {
        assert_eq!(DateTimeVersion::scheme_name(), "datetime");

        let c = DateTimeVersion::from_native_constraint(">2024-01-24T10:00:00.000Z").unwrap();
        assert_eq!(c.comparator, Comparator::GreaterThan);
        assert_eq!(c.version.to_string(), "2024-01-24T10:00:00.000Z");

        let any = DateTimeVersion::from_native_constraint("*").unwrap();
        assert_eq!(any.comparator, Comparator::Any);

        let multi =
            DateTimeVersion::from_native(">=2024-01-01T00:00:00Z, <2025-01-01T00:00:00Z").unwrap();
        assert_eq!(multi.len(), 2);
        assert_eq!(multi[0].comparator, Comparator::GreaterThanOrEqual);
        assert_eq!(multi[1].comparator, Comparator::LessThan);

        let range =
            DynamicVersionRange::parse_native("datetime", ">2024-01-24T10:00:00.000Z").unwrap();
        assert_eq!(range.versioning_scheme(), "datetime");
        assert_eq!(range.constraints().len(), 1);
    }

    #[test]
    fn test_datetime_native_errors() {
        assert!(matches!(
            DateTimeVersion::from_native(""),
            Err(VersError::EmptyConstraints)
        ));
        assert!(matches!(
            DateTimeVersion::from_native("   "),
            Err(VersError::EmptyConstraints)
        ));
        assert!(matches!(
            DateTimeVersion::from_native(">"),
            Err(VersError::MissingVersionInConstraint)
        ));
        assert!(matches!(
            DateTimeVersion::from_native_constraint(""),
            Err(VersError::InvalidConstraint(_))
        ));
        assert!(matches!(
            DateTimeVersion::from_native(">2024-01-24T10:00:00.000Z\n"),
            Err(VersError::InvalidConstraint(_))
        ));
    }
}
