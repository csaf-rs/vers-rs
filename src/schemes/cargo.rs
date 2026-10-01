//! Version constraint type for the Cargo versioning scheme.
//!
//! This module contains the `CargoVersion` struct and its implementation of the
//! `NativeVersionConverter` trait, supporting Cargo dependency specification rules
//! based on semver rules.

use crate::VersError;
use crate::VersionConstraint;
use crate::comparator::Comparator;
use crate::constraint::NativeVersionConverter;
use derive_more::Display;
use semver::{Version, VersionReq};
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
                "Control characters are not permitted in native version ranges".to_string(),
            ));
        }
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(VersError::EmptyConstraints);
        }

        let normalized_raw = normalize_cargo_req(raw);
        let req = VersionReq::parse(&normalized_raw).map_err(|e| {
            VersError::InvalidVersionFormat(
                CARGO_SCHEME.to_string(),
                raw.to_string(),
                e.to_string(),
            )
        })?;

        let mut constraints = Vec::new();
        for pred in req.comparators {
            let comparator = match pred.op {
                semver::Op::Exact | semver::Op::Wildcard => Comparator::Equal,
                semver::Op::Greater => Comparator::GreaterThan,
                semver::Op::GreaterEq => Comparator::GreaterThanOrEqual,
                semver::Op::Less => Comparator::LessThan,
                semver::Op::LessEq => Comparator::LessThanOrEqual,
                _ => Comparator::Equal,
            };

            let v = Version {
                major: pred.major,
                minor: pred.minor.unwrap_or(0),
                patch: pred.patch.unwrap_or(0),
                pre: pred.pre.clone(),
                build: semver::BuildMetadata::EMPTY,
            };

            constraints.push(VersionConstraint::new(comparator, CargoVersion(v)));
        }

        if constraints.is_empty() {
            return Err(VersError::EmptyConstraints);
        }

        Ok(constraints)
    }

    fn from_native_constraint(raw: &str) -> Result<VersionConstraint<Self>, VersError> {
        let constraints = Self::from_native(raw)?;
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

// --- Hilfsfunktionen ---

fn normalize_cargo_req(raw: &str) -> String {
    let mut result = String::new();
    let mut current_num = String::new();

    for c in raw.chars() {
        if c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c.is_alphanumeric() {
            current_num.push(c);
        } else {
            if !current_num.is_empty() {
                result.push_str(&normalize_version_token(&current_num));
                current_num.clear();
            }
            result.push(c);
        }
    }
    if !current_num.is_empty() {
        result.push_str(&normalize_version_token(&current_num));
    }
    result
}

fn normalize_version_token(token: &str) -> String {
    let dots = token.matches('.').count();
    if dots == 1 && token.chars().all(|c| c.is_ascii_digit() || c == '.') {
        format!("{}.0", token)
    } else if dots == 0 && token.chars().all(|c| c.is_ascii_digit()) {
        format!("{}.0.0", token)
    } else {
        token.to_string()
    }
}

fn normalize_version_string(s: &str) -> String {
    let mut base = s;
    let mut suffix: &str = "";
    if let Some(idx) = s.find('+') {
        base = &s[..idx];
        suffix = &s[idx..];
    }
    let mut pre: &str = "";
    if let Some(idx) = base.find('-') {
        pre = &base[idx..];
        base = &base[..idx];
    }

    let dots: usize = base.matches('.').count();
    let core = if dots == 1 {
        format!("{}.0", base)
    } else if dots == 0 {
        format!("{}.0.0", base)
    } else {
        base.to_string()
    };

    format!("{}{}{}", core, pre, suffix)
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
        self.0.cmp(&other.0)
    }
}

impl FromStr for CargoVersion {
    type Err = VersError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let normalized = normalize_version_string(s);
        let v = Version::parse(&normalized).map_err(|e| {
            VersError::InvalidVersionFormat(CARGO_SCHEME.to_string(), s.to_string(), e.to_string())
        })?;
        Ok(CargoVersion(v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DynamicVersionRange;
    use crate::range::VersionRange;

    #[test]
    fn test_vers_cargo_explicit_equals_fails() {
        let result: Result<DynamicVersionRange, _> = "vers:cargo/=1.2.3".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_vers_cargo_implicit_equals_succeeds() {
        let result: Result<DynamicVersionRange, _> = "vers:cargo/1.2.3".parse();
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
    fn lower_bound_version_range_should_contain_higher_prerelease_version() {
        let target_version = CargoVersion::from_str("1.5.0-alpha").unwrap();
        let lower_bound_version_range =
            CargoVersion::from_native_string("cargo", ">=1.2.4").unwrap();
        assert!(lower_bound_version_range.contains(target_version).unwrap());
    }

    #[test]
    fn test_cargo_explicit_prerelease_range_should_contain_same_patch() {
        let range: DynamicVersionRange = "vers:cargo/>=1.0.0-alpha".parse().unwrap();
        let should_be_true = range
            .contains("1.0.0-beta".parse().unwrap())
            .expect("contains should succeed");
        assert!(should_be_true);
    }

    #[test]
    fn test_cargo_prerelease_ordering() {
        let v_alpha = CargoVersion::from_str("1.0.0-alpha").unwrap();
        let v_stable = CargoVersion::from_str("1.0.0").unwrap();
        let v_higher = CargoVersion::from_str("1.1.0").unwrap();

        assert!(v_alpha < v_stable, "1.0.0-alpha should be less than 1.0.0");
        assert!(v_alpha < v_higher, "1.0.0-alpha should be less than 1.1.0");
    }
}
