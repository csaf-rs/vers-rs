//! Version constraint type for the PyPI versioning scheme.

use crate::VersionConstraint;
use crate::constraint::NativeVersionConverter;
use crate::enums::comparator::Comparator;
use crate::{VersError, VersioningScheme};
use pep440_rs::{Version as PypiVersionInner, VersionSpecifiers};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ordering;
use std::fmt::{self, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::str::FromStr;

#[derive(Clone, Debug)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "wasm", tsify(type = "string"))]
pub struct PypiVersion {
    inner: Option<PypiVersionInner>,
    raw: String,
}

impl PypiVersion {
    fn from_arbitrary(raw: &str) -> Self {
        PypiVersion {
            inner: PypiVersionInner::from_str(raw).ok(),
            raw: raw.to_string(),
        }
    }
}

impl Default for PypiVersion {
    fn default() -> Self {
        PypiVersion {
            inner: PypiVersionInner::from_str("0.0.0").ok(),
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
    fn scheme_name() -> &'static str {
        VersioningScheme::Pypi.as_ref()
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

        if raw == "*" || raw == "==*" {
            return Ok(vec![VersionConstraint::new(
                Comparator::Any,
                PypiVersion::default(),
            )]);
        }

        // Arbitrary equality (===) handling
        if let Some(stripped) = raw.strip_prefix("===") {
            let ver_str = stripped.trim();
            if ver_str.is_empty() {
                return Err(VersError::MissingVersionInConstraint);
            }
            let version = PypiVersion::from_arbitrary(ver_str);
            return Ok(vec![VersionConstraint::new(Comparator::Equal, version)]);
        }

        // Wildcard support: ==1.4.* -> >=1.4.0, <2
        if let Some(stripped) = raw.strip_prefix("==") {
            let ver_str = stripped.trim();
            if let Some(base) = ver_str.strip_suffix(".*") {
                let upper = calculate_pypi_wildcard_upper_bound(base);
                return Ok(vec![
                    VersionConstraint::new(
                        Comparator::GreaterThanOrEqual,
                        PypiVersion::from_str(&format!("{}.0", base))?,
                    ),
                    VersionConstraint::new(Comparator::LessThan, PypiVersion::from_str(&upper)?),
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
                constraints.push(VersionConstraint::new(
                    Comparator::GreaterThanOrEqual,
                    base_ver,
                ));
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
            if ver_str.is_empty() {
                return Err(VersError::MissingVersionInConstraint);
            }
            let version = PypiVersion::from_arbitrary(ver_str);
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

        let inner = PypiVersionInner::from_str(s).map_err(|e| {
            VersError::InvalidVersionFormat(
                Self::scheme_name().to_string(),
                s.to_string(),
                e.to_string(),
            )
        })?;

        Ok(PypiVersion {
            inner: Some(inner),
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
        match (&self.inner, &other.inner) {
            (Some(a), Some(b)) => a == b,
            _ => self.raw == other.raw,
        }
    }
}

impl Eq for PypiVersion {}

impl Hash for PypiVersion {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match &self.inner {
            Some(inner) => inner.hash(state),
            None => self.raw.hash(state),
        }
    }
}

impl PartialOrd for PypiVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PypiVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        match (&self.inner, &other.inner) {
            (Some(a), Some(b)) => a.cmp(b),
            _ => self.raw.cmp(&other.raw),
        }
    }
}

fn calculate_pypi_compatible_upper_bound(v: &str) -> String {
    let Ok(version) = PypiVersionInner::from_str(v) else {
        return v.to_string();
    };
    let mut release = version.release().to_vec();
    if release.len() < 2 {
        return v.to_string();
    }
    release.pop();
    let Some(last) = release.pop() else {
        return v.to_string();
    };
    let mut upper: Vec<String> = release.iter().map(|part| part.to_string()).collect();
    upper.push((u128::from(last) + 1).to_string());
    let upper = upper.join(".");
    if version.epoch() == 0 {
        upper
    } else {
        format!("{}!{}", version.epoch(), upper)
    }
}

fn calculate_pypi_wildcard_upper_bound(v: &str) -> String {
    let Ok(version) = PypiVersionInner::from_str(v) else {
        return format!("{}.0", v);
    };
    let mut release = version.release().to_vec();
    if release.len() < 2 {
        let Some(last) = release.pop() else {
            return format!("{}.0", v);
        };
        let next = (u128::from(last) + 1).to_string();
        return if version.epoch() == 0 {
            next
        } else {
            format!("{}!{}", version.epoch(), next)
        };
    }
    release.pop();
    let Some(last) = release.pop() else {
        return format!("{}.0", v);
    };
    let mut upper: Vec<String> = release.iter().map(|part| part.to_string()).collect();
    upper.push((u128::from(last) + 1).to_string());
    let upper = upper.join(".");
    if version.epoch() == 0 {
        upper
    } else {
        format!("{}!{}", version.epoch(), upper)
    }
}
