//! Version constraint type for the PyPI versioning scheme.
//!
use crate::VersError;
use crate::VersionConstraint;
use crate::comparator::Comparator;
use crate::constraint::NativeVersionConverter;
use pep440_rs::{Version as PypiVersionInner, VersionSpecifiers};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ordering;
use std::fmt::{self, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::str::FromStr;

pub const PYPI_SCHEME: &str = "pypi";

#[derive(Clone, Debug)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct PypiVersion {
    inner: PypiVersionInner,
    raw: String,
}

impl Default for PypiVersion {
    fn default() -> Self {
        PypiVersion {
            inner: PypiVersionInner::from_str("0.0.0").unwrap(),
            raw: "0.0.0".to_string(),
        }
    }
}

impl Serialize for PypiVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.raw)
    }
}

impl<'de> Deserialize<'de> for PypiVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl NativeVersionConverter for PypiVersion {
    const SCHEME_NAME: &'static str = PYPI_SCHEME;

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

        if raw == "*" || raw == "==*" {
            return Ok(vec![VersionConstraint::new(
                Comparator::Any,
                PypiVersion::default(),
            )]);
        }

        // Arbitrary equality (===) handling
        if let Some(stripped) = raw.strip_prefix("===") {
            let ver_str = stripped.trim();
            let version = PypiVersion::from_str(ver_str)?;
            return Ok(vec![VersionConstraint::new(Comparator::Equal, version)]);
        }

        // Wildcard support: ==1.4.* -> expand to >=1.4.0, <1.5
        if let Some(stripped) = raw.strip_prefix("==") {
            let ver_str = stripped.trim();
            if let Some(base) = ver_str.strip_suffix(".*") {
                let upper = calculate_pypi_wildcard_upper_bound(base);
                return Ok(vec![
                    VersionConstraint::new(
                        Comparator::GreaterThanOrEqual,
                        PypiVersion::from_str(&format!("{}.0", base))?,
                    ),
                    VersionConstraint::new(
                        Comparator::LessThan,
                        PypiVersion::from_str(&upper)?,
                    ),
                ]);
            }
        }

        let specifiers = VersionSpecifiers::from_str(raw).map_err(|e| {
            VersError::InvalidConstraint(format!(
                "Failed to parse PyPI version specifiers '{}': {}",
                raw, e
            ))
        })?;

        let mut constraints = Vec::new();
        for spec in specifiers.iter() {
            let op_str = spec.operator().to_string();
            let ver_str = spec.version().to_string();

            if op_str == "~=" {
                let base_ver = PypiVersion::from_str(&ver_str)?;
                let upper_ver_str = calculate_pypi_compatible_upper_bound(&ver_str);
                let upper_ver = PypiVersion::from_str(&upper_ver_str)?;
                constraints.push(VersionConstraint::new(Comparator::GreaterThanOrEqual, base_ver));
                constraints.push(VersionConstraint::new(Comparator::LessThan, upper_ver));
                continue;
            }

            let comparator = match op_str.as_str() {
                "==" | "===" => Comparator::Equal,
                "!=" => Comparator::NotEqual,
                ">=" => Comparator::GreaterThanOrEqual,
                "<=" => Comparator::LessThanOrEqual,
                ">" => Comparator::GreaterThan,
                "<" => Comparator::LessThan,
                _ => Comparator::Equal,
            };

            let version = PypiVersion::from_str(&ver_str)?;
            constraints.push(VersionConstraint::new(comparator, version));
        }

        if constraints.is_empty() {
            return Err(VersError::EmptyConstraints);
        }

        Ok(constraints)
    }

    fn from_native_constraint(raw: &str) -> Result<VersionConstraint<Self>, VersError> {
        if let Some(stripped) = raw.strip_prefix("===") {
            let ver_str = stripped.trim();
            let version = PypiVersion::from_str(ver_str)?;
            return Ok(VersionConstraint::new(Comparator::Equal, version));
        }

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

impl FromStr for PypiVersion {
    type Err = VersError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let original_raw = s.to_string();

        let inner = match PypiVersionInner::from_str(s) {
            Ok(v) => v,
            Err(_) => {
                let clean = s.strip_prefix('v').unwrap_or(s);
                let base = clean.split('-').next().unwrap_or(clean);
                PypiVersionInner::from_str(base).map_err(|e| {
                    VersError::InvalidVersionFormat(
                        PYPI_SCHEME.to_string(),
                        s.to_string(),
                        e.to_string(),
                    )
                })?
            }
        };

        Ok(PypiVersion {
            inner,
            raw: original_raw,
        })
    }
}

impl Display for PypiVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

impl PartialEq for PypiVersion {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for PypiVersion {}

impl Hash for PypiVersion {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state);
    }
}

impl PartialOrd for PypiVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PypiVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        self.inner.cmp(&other.inner)
    }
}

fn calculate_pypi_compatible_upper_bound(v: &str) -> String {
    let core = v.split(['-', '+']).next().unwrap_or(v);
    let parts: Vec<&str> = core.split('.').collect();

    if parts.len() >= 2 {
        if let Ok(mut minor) = parts[parts.len() - 2].parse::<u64>() {
            minor += 1;
            let minor_str = minor.to_string();
            let mut new_parts: Vec<String> = parts[..parts.len() - 2]
                .iter()
                .map(|s| s.to_string())
                .collect();
            new_parts.push(minor_str);
            return new_parts.join(".");
        }
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