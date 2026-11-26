//! Version constraint type for the RPM versioning scheme.
//!
//! This module contains the `RpmVersion` struct and its implementation of the
//! `NativeVersionConverter` trait, utilizing the `rpm-version` crate for specification
//! rules, epoch/version/release parsing, and correct RPM sorting semantics.

use crate::VersionConstraint;
use crate::constraint::NativeVersionConverter;
use crate::enums::comparator::Comparator;
use crate::{VersError, VersioningScheme};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ordering;
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

pub static RPM_SCHEME: &str = "rpm";

#[derive(Clone, Debug, Eq)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "wasm", tsify(type = "string"))]
pub struct RpmVersion {
    raw: String,
}

impl Default for RpmVersion {
    fn default() -> Self {
        RpmVersion {
            raw: "0-0".to_string(),
        }
    }
}

impl Serialize for RpmVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.raw)
    }
}

impl<'de> Deserialize<'de> for RpmVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl PartialEq for RpmVersion {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl PartialOrd for RpmVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RpmVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        let p1 = rpm_version::Evr::parse(&self.raw);
        let p2 = rpm_version::Evr::parse(&other.raw);
        p1.cmp(&p2)
    }
}

impl std::hash::Hash for RpmVersion {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let evr = rpm_version::Evr::parse(&self.raw);
        let epoch = if evr.epoch().is_empty() {
            "0"
        } else {
            evr.epoch()
        };
        rpm_version::version_sortkey(epoch).hash(state);
        rpm_version::version_sortkey(evr.version()).hash(state);
        rpm_version::version_sortkey(evr.release()).hash(state);
    }
}

impl Display for RpmVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

// --- Native Version Converter implementation ---
impl NativeVersionConverter for RpmVersion {
    fn scheme_name() -> &'static str {
        VersioningScheme::Rpm.as_ref()
    }

    fn versioning_schemes() -> Option<&'static [VersioningScheme]> {
        Some(&[VersioningScheme::Rpm])
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

        let clauses: Vec<&str> = raw
            .split(',')
            .map(|s| s.trim())
            //.filter(|s| !s.is_empty()) Do not discard empty comma-separated clauses.
            .collect();

        if clauses.is_empty() {
            return Err(VersError::EmptyConstraints);
        }

        let mut constraints = Vec::new();
        for clause in clauses {
            constraints.push(Self::from_native_constraint(clause)?);
        }

        intersect_rpm_constraints(constraints)
    }

    fn from_native_constraint(raw: &str) -> Result<VersionConstraint<Self>, VersError> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(VersError::InvalidConstraint("Empty constraint".to_string()));
        }

        if raw == "*" || raw == "==*" {
            return Ok(VersionConstraint::new(
                Comparator::Any,
                RpmVersion::default(),
            ));
        }

        let (comparator, ver_str) = if let Some(s) = raw.strip_prefix("===") {
            (Comparator::Equal, s)
        } else if let Some(s) = raw.strip_prefix("==") {
            (Comparator::Equal, s)
        } else if let Some(s) = raw.strip_prefix(">=") {
            (Comparator::GreaterThanOrEqual, s)
        } else if let Some(s) = raw.strip_prefix("<=") {
            (Comparator::LessThanOrEqual, s)
        } else if let Some(s) = raw.strip_prefix("!=") {
            (Comparator::NotEqual, s)
        } else if let Some(s) = raw.strip_prefix('>') {
            (Comparator::GreaterThan, s)
        } else if let Some(s) = raw.strip_prefix('<') {
            (Comparator::LessThan, s)
        } else if let Some(s) = raw.strip_prefix('=') {
            (Comparator::Equal, s)
        } else {
            (Comparator::Equal, raw)
        };

        let ver_str = ver_str.trim();
        if ver_str.is_empty() {
            return Err(VersError::MissingVersionInConstraint);
        }

        let parsed_version = RpmVersion::from_str(ver_str)?;

        Ok(VersionConstraint::new(comparator, parsed_version))
    }
}

impl FromStr for RpmVersion {
    type Err = VersError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.is_empty() {
            return Err(VersError::InvalidConstraint(
                "Empty version string".to_string(),
            ));
        }

        let evr = rpm_version::Evr::parse(s);
        if evr.version().is_empty()
            || (s.contains(':')
                && (evr.epoch().is_empty() || !evr.epoch().bytes().all(|b| b.is_ascii_digit())))
            || !s.is_ascii()
            || s.bytes().any(|b| {
                crate::util::ascii_whitespace::is_ascii_vers_whitespace(b)
                    || matches!(b, b'<' | b'>' | b'=' | b'!' | b'*' | b'|')
            })
        {
            return Err(VersError::InvalidVersionFormat(
                Self::scheme_name().to_string(),
                s.to_string(),
                "RPM versions require a nonempty ASCII version without whitespace, a numeric epoch when specified, and no embedded constraint operators or wildcards"
                    .to_string(),
            ));
        }

        Ok(RpmVersion { raw: s.to_string() })
    }
}

fn intersect_rpm_constraints(
    constraints: Vec<VersionConstraint<RpmVersion>>,
) -> Result<Vec<VersionConstraint<RpmVersion>>, VersError> {
    if constraints.is_empty() {
        return Err(VersError::EmptyConstraints);
    }

    if constraints.len() == 1 {
        return Ok(constraints);
    }

    let non_any: Vec<VersionConstraint<RpmVersion>> = constraints
        .into_iter()
        .filter(|c| c.comparator != Comparator::Any)
        .collect();

    if non_any.is_empty() {
        return Ok(vec![VersionConstraint::new(
            Comparator::Any,
            RpmVersion::default(),
        )]);
    }

    let mut best_lower: Option<VersionConstraint<RpmVersion>> = None;
    let mut best_upper: Option<VersionConstraint<RpmVersion>> = None;
    let mut equals: Vec<VersionConstraint<RpmVersion>> = Vec::new();
    let mut not_equals: Vec<VersionConstraint<RpmVersion>> = Vec::new();

    for c in non_any {
        match c.comparator {
            Comparator::GreaterThan | Comparator::GreaterThanOrEqual => {
                best_lower = Some(match best_lower {
                    None => c,
                    Some(prev) => match c.version.cmp(&prev.version) {
                        Ordering::Greater => c,
                        Ordering::Less => prev,
                        Ordering::Equal => {
                            if c.comparator == Comparator::GreaterThan {
                                c
                            } else {
                                prev
                            }
                        }
                    },
                });
            }
            Comparator::LessThan | Comparator::LessThanOrEqual => {
                best_upper = Some(match best_upper {
                    None => c,
                    Some(prev) => match c.version.cmp(&prev.version) {
                        Ordering::Less => c,
                        Ordering::Greater => prev,
                        Ordering::Equal => {
                            if c.comparator == Comparator::LessThan {
                                c
                            } else {
                                prev
                            }
                        }
                    },
                });
            }
            Comparator::Equal => {
                if let Some(existing) = equals.iter().find(|e| e.version == c.version) {
                    if existing.comparator != c.comparator {
                        return Err(VersError::InvalidRange(format!(
                            "Conflicting equality constraints for version '{}'",
                            c.version
                        )));
                    }
                } else if !equals.is_empty() {
                    return Err(VersError::InvalidRange(
                        "Multiple distinct equality constraints cannot all be satisfied"
                            .to_string(),
                    ));
                } else {
                    equals.push(c);
                }
            }
            Comparator::NotEqual => {
                if !not_equals.iter().any(|ne| ne.version == c.version) {
                    not_equals.push(c);
                }
            }
            Comparator::Any => unreachable!(),
        }
    }

    // Check lower vs upper bound satisfiability
    if let (Some(low), Some(high)) = (&best_lower, &best_upper) {
        match low.version.cmp(&high.version) {
            Ordering::Greater => {
                return Err(VersError::InvalidRange(format!(
                    "Empty RPM constraint intersection: lower bound '{}' exceeds upper bound '{}'",
                    low.version, high.version
                )));
            }
            Ordering::Equal => {
                if low.comparator != Comparator::GreaterThanOrEqual
                    || high.comparator != Comparator::LessThanOrEqual
                {
                    return Err(VersError::InvalidRange(format!(
                        "Empty RPM constraint intersection at version '{}'",
                        low.version
                    )));
                }
            }
            Ordering::Less => {}
        }
    }

    // Validate equality constraints against bounds and != constraints
    for eq in &equals {
        if let Some(low) = &best_lower {
            match eq.version.cmp(&low.version) {
                Ordering::Less => {
                    return Err(VersError::InvalidRange(format!(
                        "Equality version '{}' is below lower bound '{}'",
                        eq.version, low.version
                    )));
                }
                Ordering::Equal if low.comparator == Comparator::GreaterThan => {
                    return Err(VersError::InvalidRange(format!(
                        "Equality version '{}' excluded by strict lower bound",
                        eq.version
                    )));
                }
                _ => {}
            }
        }
        if let Some(high) = &best_upper {
            match eq.version.cmp(&high.version) {
                Ordering::Greater => {
                    return Err(VersError::InvalidRange(format!(
                        "Equality version '{}' is above upper bound '{}'",
                        eq.version, high.version
                    )));
                }
                Ordering::Equal if high.comparator == Comparator::LessThan => {
                    return Err(VersError::InvalidRange(format!(
                        "Equality version '{}' excluded by strict upper bound",
                        eq.version
                    )));
                }
                _ => {}
            }
        }
        if not_equals.iter().any(|ne| ne.version == eq.version) {
            return Err(VersError::InvalidRange(format!(
                "Equality version '{}' conflicts with '!=' constraint",
                eq.version
            )));
        }
    }

    let mut result = Vec::new();
    if let Some(low) = best_lower {
        result.push(low);
    }
    if let Some(high) = best_upper {
        result.push(high);
    }
    result.extend(equals);
    for ne in not_equals {
        if !result.iter().any(|r| r == &ne) {
            result.push(ne);
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VersVersionRange;
    use crate::range::VersionRange;

    #[test]
    fn test_rpm_version_parsing_valid() {
        let v1 = RpmVersion::from_str("1.0.0-1.el9").unwrap();
        assert_eq!(v1.to_string(), "1.0.0-1.el9");

        let v2 = RpmVersion::from_str("2:1.2.3~rc1-5").unwrap();
        assert_eq!(v2.to_string(), "2:1.2.3~rc1-5");
    }

    #[test]
    fn test_rpm_version_parsing_invalid() {
        assert!(RpmVersion::from_str("").is_err());
    }

    #[test]
    fn test_rpm_version_ordering_numeric() {
        let v9 = RpmVersion::from_str("1.9.0").unwrap();
        let v10 = RpmVersion::from_str("1.10.0").unwrap();
        assert!(v9 < v10);
    }

    #[test]
    fn test_rpm_version_ordering_tildes() {
        let v_rc = RpmVersion::from_str("1.0.0~rc1").unwrap();
        let v_release = RpmVersion::from_str("1.0.0").unwrap();
        assert!(v_rc < v_release);
    }

    #[test]
    fn test_rpm_version_ordering_epochs() {
        let v_low = RpmVersion::from_str("1:1.0.0").unwrap();
        let v_high = RpmVersion::from_str("2:0.9.0").unwrap();
        assert!(v_low < v_high);
    }

    #[test]
    fn test_native_converter_constraints() {
        let constraints = RpmVersion::from_native(">=1.0.0, <2.0.0").unwrap();
        assert_eq!(constraints.len(), 2);
        assert_eq!(constraints[0].comparator, Comparator::GreaterThanOrEqual);
        assert_eq!(constraints[0].version.to_string(), "1.0.0");
        assert_eq!(constraints[1].comparator, Comparator::LessThan);
        assert_eq!(constraints[1].version.to_string(), "2.0.0");
    }

    #[test]
    fn test_native_converter_star() {
        let constraints = RpmVersion::from_native("*").unwrap();
        assert_eq!(constraints.len(), 1);
        assert_eq!(constraints[0].comparator, Comparator::Any);
    }

    #[test]
    fn test_rpm_version_range_evaluation() {
        let range: VersVersionRange<RpmVersion> = "vers:rpm/>=1.0.0|<2.0.0".parse().unwrap();

        assert!(
            range
                .contains(RpmVersion::from_str("1.5.0").unwrap())
                .unwrap()
        );
        assert!(
            !range
                .contains(RpmVersion::from_str("2.0.0").unwrap())
                .unwrap()
        );
        assert!(
            !range
                .contains(RpmVersion::from_str("0.9.0").unwrap())
                .unwrap()
        );
    }

    #[test]
    fn test_native_intersection_lower_bounds() {
        let range = RpmVersion::from_native_string("rpm", ">=1.0.0, >=2.0.0").unwrap();
        assert!(
            !range
                .contains(RpmVersion::from_str("1.5.0").unwrap())
                .unwrap()
        );
        assert!(
            range
                .contains(RpmVersion::from_str("2.0.0").unwrap())
                .unwrap()
        );
        assert!(
            range
                .contains(RpmVersion::from_str("2.5.0").unwrap())
                .unwrap()
        );
    }

    #[test]
    fn test_native_intersection_disjoint_rejected() {
        let err = RpmVersion::from_native_string("rpm", ">2.0.0, <1.0.0");
        assert!(err.is_err());
    }

    #[test]
    fn test_serde_json_roundtrip() {
        let v = RpmVersion::from_str("1:2.4.1-3.el8").unwrap();
        let json = serde_json::to_string(&v).unwrap();
        let deserialized: RpmVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(v, deserialized);
        assert_eq!(v.to_string(), deserialized.to_string());
    }
}
