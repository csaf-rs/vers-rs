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
        // Strict canonical form parsing
        let v = Version::parse(s).map_err(|e| {
            VersError::InvalidVersionFormat(CARGO_SCHEME.to_string(), s.to_string(), e.to_string())
        })?;
        Ok(CargoVersion(v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DynamicVersionRange;

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
    fn test_cargo_native_partial_version_is_normalized() {
        let result = CargoVersion::from_native(">=1.2")
            .expect("Cargo-native parsing should accept partial versions");

        assert_eq!(result[0].comparator, Comparator::GreaterThanOrEqual);
        assert_eq!(result[0].version.0.major, 1);
        assert_eq!(result[0].version.0.minor, 2);
        assert_eq!(result[0].version.0.patch, 0);
    }

    #[test]
    fn test_vers_cargo_uri_partial_version_is_rejected() {
        let result: Result<DynamicVersionRange, VersError> = "vers:cargo/>=1.2".parse();

        result.expect_err("VERS Cargo URIs require a complete version");
    }

    #[test]
    fn test_dynamic_native_cargo_partial_version_is_canonicalized() {
        let range = DynamicVersionRange::parse_native("cargo", ">=1.2").unwrap();

        assert_eq!(range.to_string(), "vers:cargo/>=1.2.0");
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
