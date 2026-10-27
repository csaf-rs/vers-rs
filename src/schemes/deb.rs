use crate::VersionConstraint;
use crate::constraint::NativeVersionConverter;
use crate::is_ascii_vers_whitespace;
use crate::{Comparator, VersError, VersioningScheme};
use debversion::Version as DebVersionInner;
use derive_more::Display;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

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
    fn scheme_name() -> &'static str {
        VersioningScheme::Deb.as_ref()
    }

    /// Debian version relations (`<<`, `<=`, `=`, `>=`, `>>`) instead of the
    /// standard vers operators; notably no bare `<`/`>`/`!=`. Longer tokens
    /// are listed before their single-character prefixes (`<<` before `<=`,
    /// `>>` before `>=`) so they aren't shadowed.
    fn native_operators() -> &'static [(&'static str, Comparator)] {
        &[
            ("<<", Comparator::LessThan),
            ("<=", Comparator::LessThanOrEqual),
            (">>", Comparator::GreaterThan),
            (">=", Comparator::GreaterThanOrEqual),
            ("=", Comparator::Equal),
        ]
    }

    /// Debian version relations always require an explicit operator and
    /// version; there is no wildcard token.
    fn native_wildcard() -> Option<&'static str> {
        None
    }

    /// Debian version relations always require an explicit operator; there
    /// is no bare-version/implicit-equal form.
    fn native_allows_implicit_equal() -> bool {
        false
    }

    fn from_native(raw: &str) -> Result<Vec<VersionConstraint<Self>>, VersError> {
        if raw
            .bytes()
            .any(|b| b != b' ' && is_ascii_vers_whitespace(b))
        {
            return Err(VersError::InvalidCharacters(
                "Control characters (tabs, newlines, carriage returns) are not permitted"
                    .to_string(),
            ));
        }

        let raw = raw.trim();
        if raw.is_empty() {
            return Err(VersError::EmptyConstraints);
        }

        if raw.contains('|') || raw.contains(',') {
            return Err(VersError::InvalidConstraint(
                "Debian native version constraints support exactly one version relation; \
             '|' and ',' are package relationship separators"
                    .to_string(),
            ));
        }

        Ok(vec![Self::from_native_constraint(raw)?])
    }
}

impl FromStr for DebVersion {
    type Err = VersError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let inner = DebVersionInner::from_str(s).map_err(|e| {
            VersError::InvalidVersionFormat(
                Self::scheme_name().to_string(),
                s.to_string(),
                e.to_string(),
            )
        })?;
        Ok(DebVersion(inner))
    }
}

impl PartialEq for DebVersion {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Hash for DebVersion {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
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
    fn test_deb_native_compound_constraints_rejected() {
        for raw in [">>1.0|>>2.0", ">>1.0,>>2.0"] {
            assert!(
                DynamicVersionRange::parse_native("deb", raw).is_err(),
                "Debian native input should reject {raw:?}",
            );
        }
    }
}
