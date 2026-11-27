//! Version constraint type for the Lexicographic versioning scheme.

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
pub struct LexicographicVersion {
    raw: String,
}

impl Default for LexicographicVersion {
    fn default() -> Self {
        LexicographicVersion {
            raw: "0".to_string(),
        }
    }
}

impl Serialize for LexicographicVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.raw)
    }
}

impl<'de> Deserialize<'de> for LexicographicVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl NativeVersionConverter for LexicographicVersion {
    fn scheme_name() -> &'static str {
        VersioningScheme::Lexicographic.as_ref()
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

impl FromStr for LexicographicVersion {
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
        Ok(LexicographicVersion {
            raw: trimmed.to_string(),
        })
    }
}

impl Display for LexicographicVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

impl Hash for LexicographicVersion {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}

impl PartialEq for LexicographicVersion {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl PartialOrd for LexicographicVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LexicographicVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        self.raw.cmp(&other.raw)
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
    fn test_lexicographic_from_str_and_display() {
        let v = LexicographicVersion::from_str("  v34h2  ").unwrap();
        assert_eq!(v.to_string(), "v34h2");
        assert_eq!(LexicographicVersion::default().to_string(), "0");
    }

    #[test]
    fn test_lexicographic_from_str_errors() {
        assert!(matches!(
            LexicographicVersion::from_str(""),
            Err(VersError::InvalidVersionFormat(_, _, _))
        ));
        assert!(matches!(
            LexicographicVersion::from_str("   "),
            Err(VersError::InvalidVersionFormat(_, _, _))
        ));
        assert!(matches!(
            LexicographicVersion::from_str("v34h2\n"),
            Err(VersError::InvalidConstraint(_))
        ));
        assert!(matches!(
            LexicographicVersion::from_str("v34\th2"),
            Err(VersError::InvalidConstraint(_))
        ));
        assert!(matches!(
            LexicographicVersion::from_str("v34\rh2"),
            Err(VersError::InvalidConstraint(_))
        ));
    }

    #[test]
    fn test_lexicographic_ordering_and_equality() {
        let v_a = LexicographicVersion::from_str("v34h2").unwrap();
        let v_a_dup = LexicographicVersion::from_str("v34h2").unwrap();
        let v_b = LexicographicVersion::from_str("v34h3").unwrap();
        let v_upper = LexicographicVersion::from_str("V34H2").unwrap();

        assert_eq!(v_a, v_a_dup);
        assert!(v_a <= v_a_dup);
        assert!(v_a >= v_a_dup);
        assert!(v_a < v_b);
        assert!(v_b > v_a);
        // ASCII byte ordering: 'V' (0x56) < 'v' (0x76)
        assert!(v_upper < v_a);

        let mut items = vec![v_b.clone(), v_a.clone(), v_upper.clone()];
        items.sort();
        assert_eq!(items, vec![v_upper, v_a, v_b]);
    }

    #[test]
    fn test_lexicographic_hash_consistency() {
        let v1 = LexicographicVersion::from_str("v34h2").unwrap();
        let v2 = LexicographicVersion::from_str("v34h2").unwrap();
        let v3 = LexicographicVersion::from_str("v34h3").unwrap();

        let mut hasher1 = std::collections::hash_map::DefaultHasher::new();
        let mut hasher2 = std::collections::hash_map::DefaultHasher::new();
        v1.hash(&mut hasher1);
        v2.hash(&mut hasher2);
        assert_eq!(
            std::hash::Hasher::finish(&hasher1),
            std::hash::Hasher::finish(&hasher2)
        );

        let mut set = HashSet::new();
        set.insert(v1);
        assert!(set.contains(&v2));
        assert!(!set.contains(&v3));

        let mut map = HashMap::new();
        map.insert(v2, "release");
        assert_eq!(
            map.get(&LexicographicVersion::from_str("v34h2").unwrap()),
            Some(&"release")
        );
    }

    #[test]
    fn test_lexicographic_serde_roundtrip() {
        let v = LexicographicVersion::from_str("v34h2").unwrap();
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, "\"v34h2\"");

        let decoded: LexicographicVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, v);
        assert_eq!(decoded.to_string(), "v34h2");
    }

    #[test]
    fn test_lexicographic_native_constraints_and_range() {
        assert_eq!(LexicographicVersion::scheme_name(), "lexicographic");

        let single = LexicographicVersion::from_native_constraint(">=v34h2").unwrap();
        assert_eq!(single.comparator, Comparator::GreaterThanOrEqual);
        assert_eq!(single.version.to_string(), "v34h2");

        let bare = LexicographicVersion::from_native_constraint("v34h2").unwrap();
        assert_eq!(bare.comparator, Comparator::Equal);
        assert_eq!(bare.version.to_string(), "v34h2");

        let any = LexicographicVersion::from_native_constraint("*").unwrap();
        assert_eq!(any.comparator, Comparator::Any);

        let multi = LexicographicVersion::from_native(">=v34h1, <v34h5, !=v34h3").unwrap();
        assert_eq!(multi.len(), 3);
        assert_eq!(multi[0].comparator, Comparator::GreaterThanOrEqual);
        assert_eq!(multi[1].comparator, Comparator::LessThan);
        assert_eq!(multi[2].comparator, Comparator::NotEqual);

        let range = DynamicVersionRange::parse_native("lexicographic", ">=v34h1, <v34h5").unwrap();
        assert_eq!(range.versioning_scheme(), "lexicographic");
        assert_eq!(range.constraints().len(), 2);
    }

    #[test]
    fn test_lexicographic_native_errors() {
        assert!(matches!(
            LexicographicVersion::from_native(""),
            Err(VersError::EmptyConstraints)
        ));
        assert!(matches!(
            LexicographicVersion::from_native("   "),
            Err(VersError::EmptyConstraints)
        ));
        assert!(matches!(
            LexicographicVersion::from_native(">="),
            Err(VersError::MissingVersionInConstraint)
        ));
        assert!(matches!(
            LexicographicVersion::from_native_constraint(""),
            Err(VersError::InvalidConstraint(_))
        ));
        assert!(matches!(
            LexicographicVersion::from_native(">=v34h2\n"),
            Err(VersError::InvalidConstraint(_))
        ));
    }
}
