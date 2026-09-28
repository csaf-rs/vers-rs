//! Version constraint type for the Cargo versioning scheme.
//!
//! This module contains the `CargoVersion` struct and its implementation of the
//! `NativeVersionConverter` trait, supporting Cargo dependency specification rules
//! (caret, tilde, wildcards, exact, and comparative ranges).

use crate::VersError;
use crate::VersionConstraint;
use crate::comparator::Comparator;
use crate::constraint::NativeVersionConverter;
use derive_more::Display;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::str::FromStr;

pub const CARGO_SCHEME: &str = "cargo";

#[derive(Display, Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct CargoVersion(Version);

impl Default for CargoVersion {
    fn default() -> Self {
        CargoVersion(Version::new(0, 0, 0))
    }
}

impl NativeVersionConverter for CargoVersion {
    const SCHEME_NAME: &'static str = CARGO_SCHEME;

    fn from_native(raw: &str) -> Result<Vec<VersionConstraint<Self>>, VersError> {
        if raw.bytes().any(|b| b == b'\t' || b == b'\n' || b == b'\r') {
            return Err(VersError::InvalidConstraint(
                "Control characters (tabs, newlines, carriage returns) are not permitted in native version ranges".to_string(),
            ));
        }
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(VersError::EmptyConstraints);
        }

        let segments: Vec<&str> = raw.split(',').collect();
        if segments.is_empty() {
            return Err(VersError::EmptyConstraints);
        }

        let mut all_constraints = Vec::new();
        for part in segments {
            let part = part.trim();
            if part.is_empty() {
                return Err(VersError::InvalidConstraint(
                    "Empty constraint clause found".to_string(),
                ));
            }
            all_constraints.extend(Self::parse_single_cargo_spec(part)?);
        }

        if all_constraints.is_empty() {
            return Err(VersError::EmptyConstraints);
        }

        Ok(all_constraints)
    }

    fn from_native_constraint(raw: &str) -> Result<VersionConstraint<Self>, VersError> {
        let constraints = Self::parse_single_cargo_spec(raw)?;
        if constraints.len() == 1 {
            Ok(constraints.into_iter().next().unwrap())
        } else {
            Err(VersError::InvalidConstraint(format!(
                "Constraint '{}' expands to multiple bounds; please use from_native instead",
                raw
            )))
        }
    }
}

impl CargoVersion {
    fn parse_single_cargo_spec(raw: &str) -> Result<Vec<VersionConstraint<Self>>, VersError> {
        let raw = raw.trim();

        if raw.ends_with(".*") || raw == "*" {
            return expand_wildcard(raw);
        }

        if raw.starts_with("==") {
            return Err(VersError::InvalidConstraint(
                "Operator '==' is not supported by Cargo specification. Use '=' instead."
                    .to_string(),
            ));
        }

        if let Some(stripped) = raw.strip_prefix('~') {
            return expand_tilde(stripped.trim());
        }

        let is_caret = raw.starts_with('^');
        let version_part = if let Some(stripped) = raw.strip_prefix('^') {
            stripped.trim()
        } else {
            raw
        };

        if let Some(stripped) = version_part.strip_prefix(">=") {
            let v = parse_version_loose(stripped, raw)?;
            return Ok(vec![VersionConstraint::new(
                Comparator::GreaterThanOrEqual,
                CargoVersion(v),
            )]);
        }
        if let Some(stripped) = version_part.strip_prefix("<=") {
            let v = parse_version_loose(stripped, raw)?;
            return Ok(vec![VersionConstraint::new(
                Comparator::LessThanOrEqual,
                CargoVersion(v),
            )]);
        }
        if let Some(stripped) = version_part.strip_prefix('>') {
            let v = parse_version_loose(stripped, raw)?;
            return Ok(vec![VersionConstraint::new(
                Comparator::GreaterThan,
                CargoVersion(v),
            )]);
        }
        if let Some(stripped) = version_part.strip_prefix('<') {
            let v = parse_version_loose(stripped, raw)?;
            return Ok(vec![VersionConstraint::new(
                Comparator::LessThan,
                CargoVersion(v),
            )]);
        }
        if let Some(stripped) = version_part.strip_prefix('=') {
            let v = parse_version_loose(stripped, raw)?;
            return Ok(vec![VersionConstraint::new(
                Comparator::Equal,
                CargoVersion(v),
            )]);
        }

        // Cargo-Spezialregel für Pre-Releases ohne expliziten Operator (z.B. "1.0.0-alpha"):
        // Wenn die Version ein Pre-Release hat und KEIN Caret ('^') davor steht,
        // erzwingt Cargo eine exakte Übereinstimmung (kein automatisches Update auf z.B. 1.0.1-alpha).
        let v = parse_version_loose(version_part, raw)?;
        if !v.pre.is_empty() && !is_caret {
            return Ok(vec![VersionConstraint::new(
                Comparator::Equal,
                CargoVersion(v),
            )]);
        }

        expand_caret_or_default(version_part)
    }
}

fn parse_version_loose(s: &str, original: &str) -> Result<Version, VersError> {
    let (core_part, dot_count, pre_release, build) = extract_core_and_dots(s);

    let normalized_core = if dot_count == 1 {
        format!("{}.0", core_part)
    } else if dot_count == 0 {
        format!("{}.0.0", core_part)
    } else {
        core_part
    };

    let mut normalized = normalized_core;
    if let Some(pre) = pre_release {
        normalized.push('-');
        normalized.push_str(pre);
    }
    if let Some(b) = build {
        normalized.push('+');
        normalized.push_str(b);
    }

    Version::parse(&normalized).map_err(|e| {
        VersError::InvalidVersionFormat(
            CARGO_SCHEME.to_string(),
            original.to_string(),
            e.to_string(),
        )
    })
}

fn expand_tilde(s: &str) -> Result<Vec<VersionConstraint<CargoVersion>>, VersError> {
    let (_, dots, _, _) = extract_core_and_dots(s);
    let v = parse_version_loose(s, s)?;
    if dots >= 1 {
        Ok(vec![
            VersionConstraint::new(Comparator::GreaterThanOrEqual, CargoVersion(v.clone())),
            VersionConstraint::new(
                Comparator::LessThan,
                CargoVersion(Version::new(v.major, v.minor + 1, 0)),
            ),
        ])
    } else {
        Ok(vec![
            VersionConstraint::new(Comparator::GreaterThanOrEqual, CargoVersion(v.clone())),
            VersionConstraint::new(
                Comparator::LessThan,
                CargoVersion(Version::new(v.major + 1, 0, 0)),
            ),
        ])
    }
}

fn expand_caret_or_default(s: &str) -> Result<Vec<VersionConstraint<CargoVersion>>, VersError> {
    let (_, dots, pre_release, _) = extract_core_and_dots(s);
    let v = parse_version_loose(s, s)?;

    // If the request has a pre-release, it is restricted to the same patch branch
    if pre_release.is_some() {
        let upper = Version::new(v.major, v.minor, v.patch);
        return Ok(vec![
            VersionConstraint::new(Comparator::GreaterThanOrEqual, CargoVersion(v)),
            VersionConstraint::new(Comparator::LessThanOrEqual, CargoVersion(upper)),
        ]);
    }

    if dots == 2 {
        let upper = if v.major > 0 {
            Version::new(v.major + 1, 0, 0)
        } else if v.minor > 0 {
            Version::new(0, v.minor + 1, 0)
        } else {
            Version::new(0, 0, v.patch + 1)
        };
        Ok(vec![
            VersionConstraint::new(Comparator::GreaterThanOrEqual, CargoVersion(v)),
            VersionConstraint::new(Comparator::LessThan, CargoVersion(upper)),
        ])
    } else if dots == 1 {
        let upper = if v.major > 0 {
            Version::new(v.major + 1, 0, 0)
        } else {
            Version::new(0, v.minor + 1, 0)
        };
        Ok(vec![
            VersionConstraint::new(Comparator::GreaterThanOrEqual, CargoVersion(v)),
            VersionConstraint::new(Comparator::LessThan, CargoVersion(upper)),
        ])
    } else {
        let upper = v.major + 1;
        Ok(vec![
            VersionConstraint::new(Comparator::GreaterThanOrEqual, CargoVersion(v)),
            VersionConstraint::new(
                Comparator::LessThan,
                CargoVersion(Version::new(upper, 0, 0)),
            ),
        ])
    }
}

fn expand_wildcard(raw: &str) -> Result<Vec<VersionConstraint<CargoVersion>>, VersError> {
    if raw == "*" {
        return Ok(vec![VersionConstraint::new(
            Comparator::Any,
            CargoVersion::default(),
        )]);
    }
    let base = &raw[..raw.len() - 2];
    let parts: Vec<&str> = base.split('.').collect();
    match parts.len() {
        1 => {
            let major = parts[0]
                .parse::<u64>()
                .map_err(|_| VersError::InvalidConstraint(raw.to_string()))?;
            Ok(vec![
                VersionConstraint::new(
                    Comparator::GreaterThanOrEqual,
                    CargoVersion(Version::new(major, 0, 0)),
                ),
                VersionConstraint::new(
                    Comparator::LessThan,
                    CargoVersion(Version::new(major + 1, 0, 0)),
                ),
            ])
        }
        2 => {
            let major = parts[0]
                .parse::<u64>()
                .map_err(|_| VersError::InvalidConstraint(raw.to_string()))?;
            let minor = parts[1]
                .parse::<u64>()
                .map_err(|_| VersError::InvalidConstraint(raw.to_string()))?;
            Ok(vec![
                VersionConstraint::new(
                    Comparator::GreaterThanOrEqual,
                    CargoVersion(Version::new(major, minor, 0)),
                ),
                VersionConstraint::new(
                    Comparator::LessThan,
                    CargoVersion(Version::new(major, minor + 1, 0)),
                ),
            ])
        }
        _ => Err(VersError::InvalidConstraint(raw.to_string())),
    }
}

fn extract_core_and_dots(s: &str) -> (String, usize, Option<&str>, Option<&str>) {
    let s = s.trim();
    let mut parts_iter = s.splitn(2, '+');
    let version_and_pre = parts_iter.next().unwrap_or(s);
    let build = parts_iter.next();

    let mut vp_iter = version_and_pre.splitn(2, '-');
    let core_part = vp_iter.next().unwrap_or(version_and_pre);
    let pre_release = vp_iter.next();

    let dots = core_part.matches('.').count();
    (core_part.to_string(), dots, pre_release, build)
}

impl PartialEq for CargoVersion {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl Eq for CargoVersion {}

impl PartialOrd for CargoVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CargoVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        let v1 = &self.0;
        let v2 = &other.0;

        let v1_has_pre = !v1.pre.is_empty();
        let v2_has_pre = !v2.pre.is_empty();

        // Cargo rule: A pre-release is strictly smaller than a stable version,
        // if the requirement itself does not have a pre-release.
        if !v1_has_pre && v2_has_pre {
            // If v1 (threshold) is stable and v2 (target) is a pre-release,
            // we treat v2 as a smaller value so that `>= v1` fails.
            return Ordering::Less;
        }
        if v1_has_pre && !v2_has_pre {
            return Ordering::Greater;
        }

        v1.cmp(v2)
    }
}

impl FromStr for CargoVersion {
    type Err = VersError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let v = parse_version_loose(s, s)?;
        Ok(CargoVersion(v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::range::VersionRange;
    use crate::{DynamicVersionRange, VersVersionRange};

    #[test]
    fn test_vers_cargo_explicit_equals_fails() {
        let result: Result<VersVersionRange<CargoVersion>, _> = "vers:cargo/=1.2.3".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_vers_cargo_implicit_equals_succeeds() {
        let result: Result<VersVersionRange<CargoVersion>, _> = "vers:cargo/1.2.3".parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_cargo_percent_decoding() {
        let constraint = CargoVersion::from_native_constraint(">=1.2.3").unwrap();
        assert_eq!(constraint.version.0.major, 1);
    }

    #[test]
    fn test_cargo_malformed_clauses_fails() {
        let result = CargoVersion::from_native("    1.2.3,,2.0.0");
        assert!(result.is_err());
    }

    #[test]
    fn test_cargo_literal_tab_fails() {
        let result = CargoVersion::from_native(">=1.2.3,\t<2.0.0");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_version_loose_partial_and_prerelease() {
        // Major only with prerelease
        let v1 = parse_version_loose("1-alpha", "1-alpha").unwrap();
        assert_eq!(v1, Version::parse("1.0.0-alpha").unwrap());

        // Major.minor with prerelease
        let v2 = parse_version_loose("1.2-beta.2", "1.2-beta.2").unwrap();
        assert_eq!(v2, Version::parse("1.2.0-beta.2").unwrap());

        // Full semver remains untouched
        let v3 = parse_version_loose("1.2.3+build.123", "1.2.3+build.123").unwrap();
        assert_eq!(v3, Version::parse("1.2.3+build.123").unwrap());
    }

    #[test]
    fn lower_bound_version_range_should_not_contain_higher_prerelease_version() {
        // Version requirements exclude pre-release versions, such as 1.0.0-alpha, unless specifically asked for.
        let target_version = CargoVersion::from_str("1.5.0-alpha").unwrap();
        let lower_bound_version_range =
            CargoVersion::from_native_string("cargo", "^1.2.4").unwrap();
        assert!(!lower_bound_version_range.contains(target_version).unwrap());
    }

    #[test]
    fn prerelease_version_range_should_not_allow_version_update_to_higher_prerelease() {
        // Note that this only works on the same release version, foo = "1.0.0-alpha" will not allow updating to foo = "1.0.1-alpha" or foo = "1.0.1-beta".
        let target_version = CargoVersion::from_str("1.0.1-alpha").unwrap();
        let lower_bound_version_range =
            CargoVersion::from_native_string("cargo", "1.0.0-alpha").unwrap();

        assert!(!lower_bound_version_range.contains(target_version).unwrap());
    }

    #[test]
    fn test_cargo_with_prerelease_should_not_contain() {
        let range: DynamicVersionRange = "vers:cargo/>=1.0.0|<2.0.0".parse().unwrap();
        let should_be_false = range
            .contains("2.0.0-alpha".parse().unwrap())
            .expect("contains should succeed");
        assert!(!should_be_false);
    }
}
