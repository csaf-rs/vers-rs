//! Version constraint type for the Cargo versioning scheme.
//!
//! This module contains the `CargoVersion` struct and its implementation of the
//! `NativeVersionConverter` trait, providing Cargo version ordering semantics
//! conforming strictly to standard VERS syntax rules.

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

#[derive(Display, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(VersError::EmptyConstraints);
        }

        let clauses: Vec<&str> = raw.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        let mut constraints = Vec::new();

        for clause in clauses {
            constraints.push(Self::from_native_constraint(clause)?);
        }

        Ok(constraints)
    }

    fn from_native_constraint(raw: &str) -> Result<VersionConstraint<Self>, VersError> {
        let raw = raw.trim();

        if raw == "*" || raw == "==*" {
            return Ok(VersionConstraint::new(Comparator::Any, CargoVersion::default()));
        }

        let (comp, ver_str) = if let Some(s) = raw.strip_prefix(">=") {
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

        let v = parse_version_loose(ver_str.trim(), raw)?;
        Ok(VersionConstraint::new(comp, CargoVersion(v)))
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
        VersError::InvalidVersionFormat(CARGO_SCHEME.to_string(), original.to_string(), e.to_string())
    })
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