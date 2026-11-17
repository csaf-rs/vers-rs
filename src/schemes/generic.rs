//! Version constraint type for the Generic versioning scheme.

use crate::constraint::NativeVersionConverter;
use crate::enums::comparator::Comparator;
use crate::{VersError, VersionConstraint, VersioningScheme};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ordering;
use std::fmt::{self, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::str::FromStr;

#[derive(Clone, Debug, Eq)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "wasm", tsify(type = "string"))]
pub struct GenericVersion {
    raw: String,
}

impl Default for GenericVersion {
    fn default() -> Self {
        GenericVersion {
            raw: "0".to_string(),
        }
    }
}

impl Serialize for GenericVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.raw)
    }
}

impl<'de> Deserialize<'de> for GenericVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl NativeVersionConverter for GenericVersion {
    fn scheme_name() -> &'static str {
        VersioningScheme::Generic.as_ref()
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

impl FromStr for GenericVersion {
    type Err = VersError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        validate_raw_text(s)?;
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(VersError::InvalidVersionFormat(
                Self::scheme_name().to_string(),
                s.to_string(),
                "Empty version string".to_string(),
            ));
        }
        Ok(GenericVersion {
            raw: trimmed.to_string(),
        })
    }
}

impl Display for GenericVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

impl Hash for GenericVersion {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.raw.to_ascii_lowercase().hash(state);
    }
}

impl PartialEq for GenericVersion {
    fn eq(&self, other: &Self) -> bool {
        self.raw.eq_ignore_ascii_case(&other.raw)
    }
}

impl PartialOrd for GenericVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for GenericVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        self.raw
            .to_ascii_lowercase()
            .cmp(&other.raw.to_ascii_lowercase())
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
    use std::collections::{HashMap, HashSet};

    #[test]
    fn test_generic_from_str_and_display() {
        let v = GenericVersion::from_str("  2026H2  ").unwrap();
        assert_eq!(v.to_string(), "2026H2");
        assert_eq!(GenericVersion::default().to_string(), "0");
    }

    #[test]
    fn test_generic_from_str_errors() {
        assert!(matches!(
            GenericVersion::from_str(""),
            Err(VersError::InvalidVersionFormat(_, _, _))
        ));
        assert!(matches!(
            GenericVersion::from_str("   "),
            Err(VersError::InvalidVersionFormat(_, _, _))
        ));
        assert!(matches!(
            GenericVersion::from_str("2026H2\n"),
            Err(VersError::InvalidConstraint(_))
        ));
        assert!(matches!(
            GenericVersion::from_str("2026\tH2"),
            Err(VersError::InvalidConstraint(_))
        ));
        assert!(matches!(
            GenericVersion::from_str("2026\rH2"),
            Err(VersError::InvalidConstraint(_))
        ));
    }

    #[test]
    fn test_generic_ordering_and_equality() {
        let v_h1 = GenericVersion::from_str("2026H1").unwrap();
        let v_h2 = GenericVersion::from_str("2026H2").unwrap();
        let v_h2_lower = GenericVersion::from_str("2026h2").unwrap();

        assert_eq!(v_h2, v_h2_lower);
        assert!(v_h2 >= v_h2_lower);
        assert!(v_h2 <= v_h2_lower);
        assert!(v_h1 < v_h2);
        assert!(v_h2 > v_h1);

        let mut sorted = vec![v_h2.clone(), v_h1.clone()];
        sorted.sort();
        assert_eq!(sorted, vec![v_h1, v_h2]);
    }

    #[test]
    fn test_generic_hash_consistency() {
        let v1 = GenericVersion::from_str("2026H2").unwrap();
        let v2 = GenericVersion::from_str("2026h2").unwrap();
        let v3 = GenericVersion::from_str("2026H1").unwrap();

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
        map.insert(v2, "half-2");
        assert_eq!(
            map.get(&GenericVersion::from_str("2026H2").unwrap()),
            Some(&"half-2")
        );
    }

    #[test]
    fn test_generic_serde_roundtrip() {
        let v = GenericVersion::from_str("2026H2").unwrap();
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, "\"2026H2\"");

        let decoded: GenericVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, v);
        assert_eq!(decoded.to_string(), "2026H2");
    }

    #[test]
    fn test_generic_native_constraints() {
        assert_eq!(GenericVersion::scheme_name(), "generic");

        let single = GenericVersion::from_native_constraint(">=2026H1").unwrap();
        assert_eq!(single.comparator, Comparator::GreaterThanOrEqual);
        assert_eq!(single.version.to_string(), "2026H1");

        let any = GenericVersion::from_native_constraint("*").unwrap();
        assert_eq!(any.comparator, Comparator::Any);

        let multi = GenericVersion::from_native(">=2025H1, <=2026H2, !=2026H1").unwrap();
        assert_eq!(multi.len(), 3);
        assert_eq!(multi[0].comparator, Comparator::GreaterThanOrEqual);
        assert_eq!(multi[1].comparator, Comparator::LessThanOrEqual);
        assert_eq!(multi[2].comparator, Comparator::NotEqual);
    }

    #[test]
    fn test_generic_native_errors() {
        assert!(matches!(
            GenericVersion::from_native(""),
            Err(VersError::EmptyConstraints)
        ));
        assert!(matches!(
            GenericVersion::from_native("   "),
            Err(VersError::EmptyConstraints)
        ));
        assert!(matches!(
            GenericVersion::from_native(">="),
            Err(VersError::MissingVersionInConstraint)
        ));
        assert!(matches!(
            GenericVersion::from_native_constraint(""),
            Err(VersError::InvalidConstraint(_))
        ));
        assert!(matches!(
            GenericVersion::from_native(">=2026H2\n"),
            Err(VersError::InvalidConstraint(_))
        ));
    }
}
