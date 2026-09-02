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

pub static CARGO_SCHEME: &str = "cargo";

#[derive(Display, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct CargoVersion(Version);

impl Default for CargoVersion {
    fn default() -> Self {
        CargoVersion(Version::new(0, 0, 0))
    }
}

impl NativeVersionConverter for CargoVersion {
    const SCHEME_NAME: &'static str = "cargo";

    /// Parse a full native range string into one or more standard `VersionConstraint`s.
    ///
    /// Cargo native constraints can use commas (`,`) for conjunction within a segment
    /// and pipes (`|`) for disjunction between segments. Because a single Cargo spec
    /// like `^1.2.3` or `1.2.*` or `~1.2` can expand into multiple constraints (e.g. `>=1.2.3, <2.0.0`),
    /// we override `from_native` to correctly flatten all segments and comma-separated clauses.
    fn from_native(raw: &str) -> Result<Vec<VersionConstraint<Self>>, VersError> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(VersError::EmptyConstraints);
        }

        let segments: Vec<&str> = raw
            .trim_matches('|')
            .split('|')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        if segments.is_empty() {
            return Err(VersError::EmptyConstraints);
        }

        let mut all_constraints = Vec::new();
        for segment in segments {
            if segment.contains(',') {
                for part in segment.split(',') {
                    let part = part.trim();
                    if !part.is_empty() {
                        all_constraints.extend(Self::parse_single_cargo_spec(part)?);
                    }
                }
            } else {
                all_constraints.extend(Self::parse_single_cargo_spec(segment)?);
            }
        }

        if all_constraints.is_empty() {
            return Err(VersError::EmptyConstraints);
        }

        Ok(all_constraints)
    }

    /// Parse a single native constraint string into a single `VersionConstraint`.
    /// If the single constraint expands into multiple bounds (like a caret or tilde range),
    /// we return an error or handle it appropriately for single-constraint contexts.
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

        if let Some(stripped) = raw.strip_prefix('~') {
            return expand_tilde(stripped.trim());
        }

        let (is_explicit_caret, version_part) = if let Some(stripped) = raw.strip_prefix('^') {
            (true, stripped.trim())
        } else {
            (false, raw)
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

        expand_caret_or_default(version_part, is_explicit_caret)
    }
}

fn parse_version_loose(s: &str, original: &str) -> Result<Version, VersError> {
    let s = s.trim();
    let core_part = s.split(['-', '+']).next().unwrap_or(s);
    let dot_count = core_part.matches('.').count();

    let normalized = if dot_count == 1 {
        format!("{}.0", s)
    } else if dot_count == 0 {
        format!("{}.0.0", s)
    } else {
        s.to_string()
    };
    Version::parse(&normalized).map_err(|e| {
        VersError::InvalidVersionFormat(CARGO_SCHEME, original.to_string(), e.to_string())
    })
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

fn expand_tilde(s: &str) -> Result<Vec<VersionConstraint<CargoVersion>>, VersError> {
    let core_part = s.split(['-', '+']).next().unwrap_or(s);
    let dots = core_part.matches('.').count();
    let v = parse_version_loose(s, s)?;
    if dots == 2 || dots == 1 {
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

fn expand_caret_or_default(
    s: &str,
    _explicit: bool,
) -> Result<Vec<VersionConstraint<CargoVersion>>, VersError> {
    let core_part = s.split(['-', '+']).next().unwrap_or(s);
    let dots = core_part.matches('.').count();
    let v = parse_version_loose(s, s)?;

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
        let v = parse_version_loose(s, s)?;
        Ok(CargoVersion(v))
    }
}
