//! Version constraint type for the RPM versioning scheme.
//!
//! This module contains the `RpmVersion` struct and its implementation of the
//! `NativeVersionConverter` trait, supporting RPM specification rules (`rpmvercmp`),
//! including epochs, version segments, release suffixes, tilde (`~`) pre-releases,
//! and range/constraint expansion.

use crate::VersError;
use crate::VersionConstraint;
use crate::comparator::Comparator;
use crate::constraint::NativeVersionConverter;
use derive_more::Display;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::str::FromStr;

pub static RPM_SCHEME: &str = "rpm";

#[derive(Display, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct RpmVersion(String);

impl Default for RpmVersion {
    fn default() -> Self {
        RpmVersion("0-0".to_string())
    }
}

// --- RPM rpmvercmp Algorithm Implementation ---

fn rpmvercmp(a: &str, b: &str) -> Ordering {
    if a == b {
        return Ordering::Equal;
    }

    let mut chars_a = a.chars().peekable();
    let mut chars_b = b.chars().peekable();

    loop {
        // 1. Skip non-alphanumeric and non-tilde characters
        while let Some(&c) = chars_a.peek() {
            if c.is_ascii_alphanumeric() || c == '~' {
                break;
            }
            chars_a.next();
        }
        while let Some(&c) = chars_b.peek() {
            if c.is_ascii_alphanumeric() || c == '~' {
                break;
            }
            chars_b.next();
        }

        let a_peek = chars_a.peek().copied();
        let b_peek = chars_b.peek().copied();

        // If both reached the end, they are equal
        if a_peek.is_none() && b_peek.is_none() {
            return Ordering::Equal;
        }
        if a_peek.is_none() {
            return if b_peek == Some('~') {
                Ordering::Greater
            } else {
                Ordering::Less
            };
        }
        if b_peek.is_none() {
            return if a_peek == Some('~') {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }

        let ca = a_peek.unwrap();
        let cb = b_peek.unwrap();

        // 2. Handle tilde (~) logic explicitly per rpmvercmp.c spec
        if ca == '~' || cb == '~' {
            if ca != '~' {
                return Ordering::Greater; // a does not have tilde, so a > b
            }
            if cb != '~' {
                return Ordering::Less; // b does not have tilde, so a < b
            }
            chars_a.next();
            chars_b.next();
            continue;
        }

        // 3. Extract segments (numeric vs alphabetic)
        let is_numeric = ca.is_ascii_digit();

        let mut seg_a = String::new();
        while let Some(&c) = chars_a.peek() {
            if c == '~'
                || !c.is_ascii_alphanumeric()
                || !is_numeric && c.is_ascii_digit()
                || is_numeric && !c.is_ascii_digit()
            {
                break;
            }
            seg_a.push(chars_a.next().unwrap());
        }

        let mut seg_b = String::new();
        while let Some(&c) = chars_b.peek() {
            if c == '~'
                || !c.is_ascii_alphanumeric()
                || !is_numeric && c.is_ascii_digit()
                || is_numeric && !c.is_ascii_digit()
            {
                break;
            }
            seg_b.push(chars_b.next().unwrap());
        }

        if seg_b.is_empty() {
            return if is_numeric {
                Ordering::Greater
            } else {
                Ordering::Less
            };
        }
        if seg_a.is_empty() {
            return if is_numeric {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }

        if is_numeric {
            let trimmed_a = seg_a.trim_start_matches('0');
            let trimmed_b = seg_b.trim_start_matches('0');

            if trimmed_a.len() != trimmed_b.len() {
                return trimmed_a.len().cmp(&trimmed_b.len());
            }

            let ord = trimmed_a.cmp(trimmed_b);
            if ord != Ordering::Equal {
                return ord;
            }

            let ord_len = seg_a.len().cmp(&seg_b.len());
            if ord_len != Ordering::Equal {
                return ord_len;
            }
        } else {
            let ord = seg_a.cmp(&seg_b);
            if ord != Ordering::Equal {
                return ord;
            }
        }
    }
}

/// Parsed EVR components of an RPM package version string
struct ParsedRpm {
    epoch: i64,
    version: String,
    release: String,
}

fn parse_rpm_version(s: &str) -> ParsedRpm {
    let s = s.trim();

    // 1. Epoch parsing (e.g. 1:1.0-1)
    let (epoch, rest) = if let Some(idx) = s.find(':') {
        let e = s[..idx].parse::<i64>().unwrap_or(0);
        (e, &s[idx + 1..])
    } else {
        (0, s)
    };

    // 2. Release parsing (e.g. 1.0-1.el9 -> version = 1.0, release = 1.el9)
    let (version, release) = if let Some(idx) = rest.find('-') {
        (&rest[..idx], &rest[idx + 1..])
    } else {
        (rest, "")
    };

    ParsedRpm {
        epoch,
        version: version.to_string(),
        release: release.to_string(),
    }
}

impl PartialOrd for RpmVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RpmVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        let p1 = parse_rpm_version(&self.0);
        let p2 = parse_rpm_version(&other.0);

        // Compare Epochs numerically
        if p1.epoch != p2.epoch {
            return p1.epoch.cmp(&p2.epoch);
        }

        // Compare Versions using rpmvercmp
        let v_ord = rpmvercmp(&p1.version, &p2.version);
        if v_ord != Ordering::Equal {
            return v_ord;
        }

        // Compare Releases using rpmvercmp
        rpmvercmp(&p1.release, &p2.release)
    }
}

// --- Native Version Converter implementation ---
impl NativeVersionConverter for RpmVersion {
    const SCHEME_NAME: &'static str = "rpm";

    fn from_native(raw: &str) -> Result<Vec<VersionConstraint<Self>>, VersError> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(VersError::EmptyConstraints);
        }

        let clauses: Vec<&str> = raw
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        let mut constraints = Vec::new();

        for clause in clauses {
            constraints.extend(Self::parse_rpm_spec_expanded(clause)?);
        }

        Ok(constraints)
    }

    fn from_native_constraint(raw: &str) -> Result<VersionConstraint<Self>, VersError> {
        let constraints = Self::parse_rpm_spec_expanded(raw)?;
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

impl RpmVersion {
    fn parse_rpm_spec_expanded(raw: &str) -> Result<Vec<VersionConstraint<Self>>, VersError> {
        let raw = raw.trim();

        if raw == "*" || raw == "==*" {
            return Ok(vec![VersionConstraint::new(
                Comparator::Any,
                RpmVersion::default(),
            )]);
        }

        let (comp, ver_str) = if let Some(s) = raw.strip_prefix("===") {
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
        } else {
            (Comparator::Equal, raw)
        };

        Ok(vec![VersionConstraint::new(
            comp,
            RpmVersion(ver_str.trim().to_string()),
        )])
    }
}

impl FromStr for RpmVersion {
    type Err = VersError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(RpmVersion(s.trim().to_string()))
    }
}
