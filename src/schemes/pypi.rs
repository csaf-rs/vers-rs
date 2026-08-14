//! Version constraint type for the PyPI versioning scheme.
//!
//! This module contains the `PypiVersion` struct and its implementation of the
//! `NativeVersionConverter` trait, supporting PEP 440 specification rules
//! (exact, comparative, compatible releases, arbitrary equality, wildcards, local identifiers, and epochs).
//!
use crate::VersError;
use crate::VersionConstraint;
use crate::comparator::Comparator;
use crate::constraint::NativeVersionConverter;
use derive_more::Display;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::str::FromStr;

pub static PYPI_SCHEME: &str = "pypi";

#[derive(Display, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct PypiVersion(String);

impl Default for PypiVersion {
    fn default() -> Self {
        PypiVersion("0.0.0".to_string())
    }
}

// --- PEP 440 Version Ordering & Parsing ---
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum PypiItem {
    Epoch(i64),
    Release(Vec<i64>),
    Dev(i64),
    Pre(String, i64),
    Post(i64),
    Local(Vec<LocalItem>),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum LocalItem {
    Numeric(i64),
    Alpha(String),
}

fn parse_local_identifier(s: &str) -> Vec<LocalItem> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|tok| !tok.is_empty())
        .map(|tok| {
            if let Ok(n) = tok.parse::<i64>() {
                LocalItem::Numeric(n)
            } else {
                LocalItem::Alpha(tok.to_ascii_lowercase())
            }
        })
        .collect()
}

fn parse_pypi_version(s: &str) -> Vec<PypiItem> {
    let s = s.trim().to_ascii_lowercase();
    let mut items = Vec::new();

    // Epoch parsing (e.g. 1!1.0.0)
    let rest = if let Some(idx) = s.find('!') {
        let epoch = s[..idx].parse::<i64>().unwrap_or(0);
        items.push(PypiItem::Epoch(epoch));
        &s[idx + 1..]
    } else {
        items.push(PypiItem::Epoch(0));
        &s
    };

    // Split local version (+...)
    let (rest, local) = if let Some(idx) = rest.find('+') {
        let l_str = &rest[idx + 1..];
        (&rest[..idx], Some(parse_local_identifier(l_str)))
    } else {
        (rest, None)
    };

    // Split dev (.devN)
    let (rest, dev) = if let Some(idx) = rest.find(".dev") {
        let d_str = &rest[idx + 4..];
        let d_val = d_str
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse::<i64>()
            .unwrap_or(0);
        (&rest[..idx], Some(d_val))
    } else {
        (rest, None)
    };

    // Split post (.postN)
    let (rest, post) = if let Some(idx) = rest.find(".post") {
        let p_str = &rest[idx + 5..];
        let p_val = p_str
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse::<i64>()
            .unwrap_or(0);
        (&rest[..idx], Some(p_val))
    } else {
        (rest, None)
    };

    // Split pre-release (aN, bN, rcN)
    let pre_markers = ["rc", "preview", "c", "b", "beta", "a", "alpha", "pre"];
    let mut pre = None;
    let mut release_part = rest;

    for marker in &pre_markers {
        if let Some(idx) = rest.find(marker) {
            let pre_str = &rest[idx + marker.len()..];
            let p_val = pre_str
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse::<i64>()
                .unwrap_or(0);
            let canonical_pre = match *marker {
                "alpha" | "a" => "a",
                "beta" | "b" => "b",
                _ => "rc",
            };
            pre = Some((canonical_pre.to_string(), p_val));
            release_part = &rest[..idx];
            break;
        }
    }

    let release_nums = release_part
        .split('.')
        .map(|tok| {
            tok.chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse::<i64>()
                .unwrap_or(0)
        })
        .collect();

    items.push(PypiItem::Release(release_nums));

    if let Some(d) = dev {
        items.push(PypiItem::Dev(d));
    }

    if let Some((p_name, p_val)) = pre {
        items.push(PypiItem::Pre(p_name, p_val));
    }

    if let Some(p) = post {
        items.push(PypiItem::Post(p));
    }

    if let Some(l) = local {
        items.push(PypiItem::Local(l));
    }

    items
}

impl PartialOrd for PypiVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PypiVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        let items1 = parse_pypi_version(&self.0);
        let items2 = parse_pypi_version(&other.0);

        let mut epoch1 = 0;
        let mut rel1 = vec![];
        let mut dev1 = None;
        let mut pre1 = None;
        let mut post1 = None;
        let mut local1 = None;

        for item in items1 {
            match item {
                PypiItem::Epoch(e) => epoch1 = e,
                PypiItem::Release(r) => rel1 = r,
                PypiItem::Dev(d) => dev1 = Some(d),
                PypiItem::Pre(s, v) => pre1 = Some((s, v)),
                PypiItem::Post(p) => post1 = Some(p),
                PypiItem::Local(l) => local1 = Some(l),
            }
        }

        let mut epoch2 = 0;
        let mut rel2 = vec![];
        let mut dev2 = None;
        let mut pre2 = None;
        let mut post2 = None;
        let mut local2 = None;

        for item in items2 {
            match item {
                PypiItem::Epoch(e) => epoch2 = e,
                PypiItem::Release(r) => rel2 = r,
                PypiItem::Dev(d) => dev2 = Some(d),
                PypiItem::Pre(s, v) => pre2 = Some((s, v)),
                PypiItem::Post(p) => post2 = Some(p),
                PypiItem::Local(l) => local2 = Some(l),
            }
        }

        if epoch1 != epoch2 {
            return epoch1.cmp(&epoch2);
        }
        let rel_ord = compare_releases(&rel1, &rel2);
        if rel_ord != Ordering::Equal {
            return rel_ord;
        }

        // PEP 440: Dev-release < Pre-release < Release < Post-release
        match (&dev1, &dev2) {
            (Some(d1), Some(d2)) => {
                if d1 != d2 {
                    return d1.cmp(d2);
                }
            }
            (Some(_), None) => return Ordering::Less,
            (None, Some(_)) => return Ordering::Greater,
            _ => {}
        }

        match (&pre1, &pre2) {
            (Some(p1), Some(p2)) => {
                if p1.0 != p2.0 {
                    return p1.0.cmp(&p2.0); // 'a' < 'b' < 'rc'
                }
                if p1.1 != p2.1 {
                    return p1.1.cmp(&p2.1);
                }
            }
            (Some(_), None) => return Ordering::Less,
            (None, Some(_)) => return Ordering::Greater,
            _ => {}
        }

        match (&post1, &post2) {
            (Some(p1), Some(p2)) => {
                if p1 != p2 {
                    return p1.cmp(p2);
                }
            }
            (Some(_), None) => return Ordering::Greater,
            (None, Some(_)) => return Ordering::Less,
            _ => {}
        }

        match (&local1, &local2) {
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(l1), Some(l2)) => l1.cmp(l2),
            (None, None) => Ordering::Equal,
        }
    }
}

// --- Native Version Converter implementation ---
impl NativeVersionConverter for PypiVersion {
    const SCHEME_NAME: &'static str = "pypi";

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
            constraints.extend(Self::parse_pypi_spec_expanded(clause)?);
        }

        Ok(constraints)
    }

    fn from_native_constraint(raw: &str) -> Result<VersionConstraint<Self>, VersError> {
        let constraints = Self::parse_pypi_spec_expanded(raw)?;
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

impl PypiVersion {
    fn parse_pypi_spec_expanded(raw: &str) -> Result<Vec<VersionConstraint<Self>>, VersError> {
        let raw = raw.trim();

        if raw == "*" || raw == "==*" {
            return Ok(vec![VersionConstraint::new(
                Comparator::Any,
                PypiVersion::default(),
            )]);
        }

        // Wildcard support: ==1.4.* -> >=1.4.0,<1.5.0
        if let Some(stripped) = raw.strip_prefix("==") {
            let ver_str = stripped.trim();
            if let Some(base) = ver_str.strip_suffix(".*") {
                let upper = calculate_pypi_wildcard_upper_bound(base);
                return Ok(vec![
                    VersionConstraint::new(
                        Comparator::GreaterThanOrEqual,
                        PypiVersion(format!("{}.0", base)),
                    ),
                    VersionConstraint::new(Comparator::LessThan, PypiVersion(upper)),
                ]);
            }
        }

        // Compatible release operator (~=1.4.2) -> expands to >=1.4.2, <1.5.0
        if let Some(stripped) = raw.strip_prefix("~=") {
            let ver_str = stripped.trim();
            let base_ver = PypiVersion(ver_str.to_string());
            let upper_ver_str = calculate_pypi_compatible_upper_bound(ver_str);

            return Ok(vec![
                VersionConstraint::new(Comparator::GreaterThanOrEqual, base_ver),
                VersionConstraint::new(Comparator::LessThan, PypiVersion(upper_ver_str)),
            ]);
        }

        let (comp, ver_str) = if let Some(s) = raw.strip_prefix("===") {
            (Comparator::Equal, s) // Arbitrary equality (===)
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
            PypiVersion(ver_str.trim().to_string()),
        )])
    }
}

fn calculate_pypi_compatible_upper_bound(v: &str) -> String {
    let core = v.split(['-', '+']).next().unwrap_or(v);
    let parts: Vec<&str> = core.split('.').collect();

    if parts.len() >= 2
        && let Ok(mut minor) = parts[parts.len() - 2].parse::<u64>()
    {
        minor += 1;
        let minor_str = minor.to_string();
        let mut new_parts: Vec<String> = parts[..parts.len() - 2]
            .iter()
            .map(|s| s.to_string())
            .collect();
        new_parts.push(minor_str);
        return new_parts.join(".");
    }
    v.to_string()
}

fn calculate_pypi_wildcard_upper_bound(v: &str) -> String {
    let parts: Vec<&str> = v.split('.').collect();
    if let Ok(mut major) = parts[0].parse::<u64>() {
        major += 1;
        return major.to_string();
    }
    format!("{}.0", v)
}

fn compare_releases(r1: &[i64], r2: &[i64]) -> Ordering {
    let max_len = std::cmp::max(r1.len(), r2.len());
    for i in 0..max_len {
        let v1 = r1.get(i).copied().unwrap_or(0);
        let v2 = r2.get(i).copied().unwrap_or(0);
        if v1 != v2 {
            return v1.cmp(&v2);
        }
    }
    Ordering::Equal
}

impl FromStr for PypiVersion {
    type Err = VersError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(PypiVersion(s.trim().to_string()))
    }
}
