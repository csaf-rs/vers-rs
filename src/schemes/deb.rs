use crate::VersError;
use crate::VersionConstraint;
use crate::comparator::Comparator;
use crate::constraint::NativeVersionConverter;
use debversion::Version as DebVersionInner;
use derive_more::Display;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::str::FromStr;

/// Scheme identifier string for Debian versions
pub const DEB_SCHEME: &str = "deb";

/// Debian version wrapper implementing `NativeVersionConverter` using the `debversion` crate.
#[derive(Display, Clone, Debug, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct DebVersion(DebVersionInner);

impl Default for DebVersion {
    fn default() -> Self {
        DebVersion(DebVersionInner::from_str("0").unwrap())
    }
}

impl NativeVersionConverter for DebVersion {
    const SCHEME_NAME: &'static str = DEB_SCHEME;

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

        let clauses: Vec<&str> = if raw.contains('|') {
            raw.split('|').collect()
        } else if raw.contains(',') {
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

        let (comparator, version_str) = if let Some(stripped) = raw.strip_prefix("<<") {
            (Comparator::LessThan, stripped)
        } else if let Some(stripped) = raw.strip_prefix("<=") {
            (Comparator::LessThanOrEqual, stripped)
        } else if let Some(stripped) = raw.strip_prefix(">>") {
            (Comparator::GreaterThan, stripped)
        } else if let Some(stripped) = raw.strip_prefix(">=") {
            (Comparator::GreaterThanOrEqual, stripped)
        } else if let Some(stripped) = raw.strip_prefix('=') {
            (Comparator::Equal, stripped)
        } else {
            return Err(VersError::InvalidConstraint(format!(
                "invalid Debian comparator in '{}': valid comparators are <<, <=, =, >=, >>",
                raw
            )));
        };

        let version_str = version_str.trim();
        if version_str.is_empty() {
            return Err(VersError::InvalidConstraint("Missing version".to_string()));
        }

        let parsed_version = version_str.parse::<DebVersion>().map_err(|e| {
            VersError::InvalidConstraint(format!(
                "Failed to parse version '{}': {}",
                version_str, e
            ))
        })?;

        Ok(VersionConstraint::new(comparator, parsed_version))
    }
}

impl FromStr for DebVersion {
    type Err = VersError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let inner = DebVersionInner::from_str(s).map_err(|e| {
            VersError::InvalidVersionFormat(DEB_SCHEME.to_string(), s.to_string(), e.to_string())
        })?;
        Ok(DebVersion(inner))
    }
}

impl PartialEq for DebVersion {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl PartialOrd for DebVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DebVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

#[cfg(test)]
mod tests {
    use crate::Comparator;
    use crate::range::VersionRange;
    use crate::range::dynamic::DynamicVersionRange;
    use std::str::FromStr;

    #[test]
    fn test_dynamic_parse_deb() {
        // Native parsing supports <<
        let range = DynamicVersionRange::parse_native("deb", "<<1.0").unwrap();
        assert_eq!(range.versioning_scheme(), "deb");
        assert_eq!(range.constraints().len(), 1);
        assert_eq!(range.constraints()[0].comparator, Comparator::LessThan);
        assert_eq!(range.constraints()[0].version.to_string(), "1.0");
    }

    #[test]
    fn test_deb_version_ordering_basic() {
        let range = DynamicVersionRange::parse_native("deb", "<<1.0").unwrap();
        assert!(range.contains("0.9".to_string()).unwrap());
        assert!(!range.contains("1.0".to_string()).unwrap());
    }

    #[test]
    fn test_deb_version_ordering_tilde_and_epoch() {
        // 1.0~beta < 1.0
        let range1 = DynamicVersionRange::parse_native("deb", "<<1.0").unwrap();
        assert!(range1.contains("1.0~beta".to_string()).unwrap());

        let range2 = DynamicVersionRange::parse_native("deb", ">>2.0").unwrap();
        // 1:1.0 > 2.0 because epoch 1 > 0
        assert!(range2.contains("1:1.0".to_string()).unwrap());
        assert!(!range2.contains("2.0".to_string()).unwrap());
    }

    #[test]
    fn test_deb_valid_comparators() {
        // Test native comparators via parse_native
        let range_lt = DynamicVersionRange::parse_native("deb", "<<1.0").unwrap();
        assert_eq!(range_lt.constraints()[0].comparator, Comparator::LessThan);

        let range_lte = DynamicVersionRange::parse_native("deb", "<=1.0").unwrap();
        assert_eq!(
            range_lte.constraints()[0].comparator,
            Comparator::LessThanOrEqual
        );

        let range_eq = DynamicVersionRange::parse_native("deb", "=1.0").unwrap();
        assert_eq!(range_eq.constraints()[0].comparator, Comparator::Equal);

        let range_gte = DynamicVersionRange::parse_native("deb", ">=1.0").unwrap();
        assert_eq!(
            range_gte.constraints()[0].comparator,
            Comparator::GreaterThanOrEqual
        );

        let range_gt = DynamicVersionRange::parse_native("deb", ">>1.0").unwrap();
        assert_eq!(
            range_gt.constraints()[0].comparator,
            Comparator::GreaterThan
        );
    }

    #[test]
    fn test_deb_invalid_comparators_rejected() {
        // Single < is not a valid Debian native comparator
        let result = DynamicVersionRange::parse_native("deb", "<1.0");
        assert!(result.is_err());

        // Single > is not a valid Debian native comparator
        let result = DynamicVersionRange::parse_native("deb", ">1.0");
        assert!(result.is_err());

        // != is not a valid Debian native comparator
        let result = DynamicVersionRange::parse_native("deb", "!=1.0");
        assert!(result.is_err());

        // >>= is not a valid Debian native comparator
        let result = DynamicVersionRange::parse_native("deb", ">>=1.0");
        assert!(result.is_err());

        // <<= is not a valid Debian native comparator
        let result = DynamicVersionRange::parse_native("deb", "<<=1.0");
        assert!(result.is_err());
    }

    #[test]
    fn test_deb_equality_consistent_with_ordering() {
        use super::DebVersion;

        let a = DebVersion::from_str("1.0").unwrap();
        let b = DebVersion::from_str("1.0-0").unwrap();

        assert_eq!(a, b);
        assert!(!(a < b));
        assert!(!(a > b));
    }

    #[test]
    fn test_deb_parse_native_preserves_scheme() {
        let range = DynamicVersionRange::parse_native("deb", "<<1.0").unwrap();
        assert_eq!(range.versioning_scheme(), "deb");
    }

    #[test]
    fn test_deb_parse_native_normalizes() {
        let range = DynamicVersionRange::parse_native("deb", ">>1.0|>>2.0").unwrap();
        assert_eq!(range.constraints().len(), 1);
        assert_eq!(range.constraints()[0].comparator, Comparator::GreaterThan);
        assert_eq!(range.constraints()[0].version.to_string(), "1.0");
    }
}
