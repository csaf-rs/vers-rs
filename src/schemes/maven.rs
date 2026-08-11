//! Version constraint type for the Maven versioning scheme.
//!
//! This module contains the `MavenVersion` struct and its implementation of the
//! `NativeVersionConverter` trait, supporting Maven dependency specification rules
//! (inclusive/exclusive intervals, half-open ranges, open-ended bounds, exact versions,
//! wildcards, and explicit comparative ranges).

use crate::VersError;
use crate::VersionConstraint;
use crate::comparator::Comparator;
use crate::constraint::NativeVersionConverter;
use derive_more::Display;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::str::FromStr;

pub static MAVEN_SCHEME: &str = "maven";

#[derive(Display, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct MavenVersion(String);

#[derive(Debug, Clone, PartialEq, Eq)]
enum MavenItem {
    Long(i64),
    Str(String),
}

impl Default for MavenVersion {
    fn default() -> Self {
        MavenVersion("0.0.0".to_string())
    }
}

impl NativeVersionConverter for MavenVersion {
    const SCHEME_NAME: &'static str = "maven";

    fn from_native(raw: &str) -> Result<Vec<VersionConstraint<Self>>, VersError> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(VersError::EmptyConstraints);
        }

        // Split by pipe first for disjunctions
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
            // A segment could be multiple comma-separated intervals like [1.0,2.0],[3.0,4.0]
            // We need to split by comma only when outside of brackets/parentheses.
            let mut current_interval = String::new();
            let mut depth = 0;
            let mut sub_segments = Vec::new();

            for c in segment.chars() {
                match c {
                    '[' | '(' => {
                        depth += 1;
                        current_interval.push(c);
                    }
                    ']' | ')' => {
                        depth -= 1;
                        current_interval.push(c);
                    }
                    ',' if depth == 0 => {
                        sub_segments.push(current_interval.trim().to_string());
                        current_interval.clear();
                    }
                    _ => {
                        current_interval.push(c);
                    }
                }
            }
            if !current_interval.trim().is_empty() {
                sub_segments.push(current_interval.trim().to_string());
            }

            for sub in sub_segments {
                if !sub.is_empty() {
                    all_constraints.extend(Self::parse_maven_spec(&sub)?);
                }
            }
        }

        if all_constraints.is_empty() {
            return Err(VersError::EmptyConstraints);
        }

        Ok(all_constraints)
    }

    fn from_native_constraint(raw: &str) -> Result<VersionConstraint<Self>, VersError> {
        let constraints = Self::parse_maven_spec(raw)?;
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

impl MavenVersion {
    fn parse_maven_spec(raw: &str) -> Result<Vec<VersionConstraint<Self>>, VersError> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(VersError::EmptyConstraints);
        }

        if raw == "*" {
            return Ok(vec![VersionConstraint::new(
                Comparator::Any,
                MavenVersion::default(),
            )]);
        }

        // Handle Maven interval notation like [1.0,2.0], (1.0,2.0), or single [1.0]
        if (raw.starts_with('[') || raw.starts_with('('))
            && (raw.ends_with(']') || raw.ends_with(')'))
        {
            let inclusive_lower = raw.starts_with('[');
            let inclusive_upper = raw.ends_with(']');
            let inner = &raw[1..raw.len() - 1];

            let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
            if parts.is_empty() || parts.len() > 2 {
                return Err(VersError::InvalidConstraint(format!(
                    "Invalid Maven interval: {}",
                    raw
                )));
            }

            let mut constraints = Vec::new();

            if parts.len() == 1 {
                // Single version in brackets like [1.0] means (implicit) exact match
                let ver = MavenVersion(parts[0].to_string());
                let comp = if inclusive_lower && inclusive_upper {
                    Comparator::Equal
                } else if inclusive_lower {
                    Comparator::GreaterThanOrEqual
                } else {
                    Comparator::LessThanOrEqual
                };
                return Ok(vec![VersionConstraint::new(comp, ver)]);
            } else {
                // Lower bound
                if !parts[0].is_empty() {
                    let lower_ver = MavenVersion(parts[0].to_string());
                    let lower_comp = if inclusive_lower {
                        Comparator::GreaterThanOrEqual
                    } else {
                        Comparator::GreaterThan
                    };
                    constraints.push(VersionConstraint::new(lower_comp, lower_ver));
                }

                // Upper bound
                if !parts[1].is_empty() {
                    let upper_ver = MavenVersion(parts[1].to_string());
                    let upper_comp = if inclusive_upper {
                        Comparator::LessThanOrEqual
                    } else {
                        Comparator::LessThan
                    };
                    constraints.push(VersionConstraint::new(upper_comp, upper_ver));
                }
            }

            if constraints.is_empty() {
                return Err(VersError::InvalidConstraint(format!(
                    "Empty Maven interval: {}",
                    raw
                )));
            }

            return Ok(constraints);
        }

        // Standard single version or explicit comparator fallback
        if let Some(stripped) = raw.strip_prefix(">=") {
            Ok(vec![VersionConstraint::new(
                Comparator::GreaterThanOrEqual,
                MavenVersion(stripped.trim().to_string()),
            )])
        } else if let Some(stripped) = raw.strip_prefix("<=") {
            Ok(vec![VersionConstraint::new(
                Comparator::LessThanOrEqual,
                MavenVersion(stripped.trim().to_string()),
            )])
        } else if let Some(stripped) = raw.strip_prefix('>') {
            Ok(vec![VersionConstraint::new(
                Comparator::GreaterThan,
                MavenVersion(stripped.trim().to_string()),
            )])
        } else if let Some(stripped) = raw.strip_prefix('<') {
            Ok(vec![VersionConstraint::new(
                Comparator::LessThan,
                MavenVersion(stripped.trim().to_string()),
            )])
        } else if let Some(stripped) = raw.strip_prefix('=') {
            Ok(vec![VersionConstraint::new(
                Comparator::Equal,
                MavenVersion(stripped.trim().to_string()),
            )])
        } else {
            Ok(vec![VersionConstraint::new(
                Comparator::Equal,
                MavenVersion(raw.to_string()),
            )])
        }
    }
}

fn parse_maven_version_items(s: &str) -> Vec<MavenItem> {
    let mut items = Vec::new();
    let s = s.trim();
    if s.is_empty() {
        return items;
    }

    // Standard Maven normalization: replace '-', '_', '.' with boundaries, and lowercase qualifiers
    let normalized = s.to_ascii_lowercase();
    let bytes = normalized.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let num_str = &normalized[start..i];
            if let Ok(val) = num_str.parse::<i64>() {
                items.push(MavenItem::Long(val));
            } else {
                items.push(MavenItem::Str(num_str.to_string()));
            }
        } else if b.is_ascii_alphabetic() {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphabetic() || bytes[i].is_ascii_digit()) {
                i += 1;
            }
            let word = &normalized[start..i];
            items.push(MavenItem::Str(word.to_string()));
        } else {
            // Separators like '-', '_', '.'
            i += 1;
        }
    }

    items
}

// Maven qualifier canonical weight mapping
// According to official Maven precedence rules, the weights must be ordered as:
// alpha < beta < milestone < rc/cr < snapshot < ga/final/"empty" < sp
fn qualifier_weight(name: &str) -> i32 {
    match name {
        "alpha" => 1,
        "beta" => 2,
        "milestone" | "m" => 3,
        "rc" | "cr" => 4,
        "snapshot" => 5,
        "" | "ga" | "final" => 6,
        "sp" => 7,
        _ => 0, // custom qualifiers
    }
}

impl PartialOrd for MavenVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MavenVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        let left_items = parse_maven_version_items(&self.0);
        let right_items = parse_maven_version_items(&other.0);

        let max_len = std::cmp::max(left_items.len(), right_items.len());
        for i in 0..max_len {
            let l = left_items.get(i);
            let r = right_items.get(i);

            match (l, r) {
                (None, None) => {}
                (None, Some(ritem)) => {
                    // Left ran out (e.g. "1.0"), right has a trailing item (e.g. "1.0-SNAPSHOT")
                    return match ritem {
                        MavenItem::Str(s) if qualifier_weight(s) < 6 => Ordering::Greater,
                        _ => Ordering::Less,
                    };
                }
                (Some(litem), None) => {
                    // Right ran out, left has a trailing item
                    return match litem {
                        MavenItem::Str(s) if qualifier_weight(s) < 6 => Ordering::Less,
                        _ => Ordering::Greater,
                    };
                }
                (Some(MavenItem::Long(lv)), Some(MavenItem::Long(rv))) => {
                    let ord = lv.cmp(rv);
                    if ord != Ordering::Equal {
                        return ord;
                    }
                }
                (Some(MavenItem::Str(ls)), Some(MavenItem::Str(rs))) => {
                    let lw = qualifier_weight(ls);
                    let rw = qualifier_weight(rs);
                    if lw != 0 && rw != 0 {
                        let ord = lw.cmp(&rw);
                        if ord != Ordering::Equal {
                            return ord;
                        }
                    }
                    let ord = ls.cmp(rs);
                    if ord != Ordering::Equal {
                        return ord;
                    }
                }
                (Some(MavenItem::Long(_)), Some(MavenItem::Str(_))) => {
                    return Ordering::Greater;
                }
                (Some(MavenItem::Str(_)), Some(MavenItem::Long(_))) => {
                    return Ordering::Less;
                }
            }
        }

        Ordering::Equal
    }
}

impl FromStr for MavenVersion {
    type Err = VersError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(MavenVersion(s.trim().to_string()))
    }
}
