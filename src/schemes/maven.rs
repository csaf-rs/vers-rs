//! Version constraint type for the Maven versioning scheme.

use crate::VersionConstraint;
use crate::constraint::NativeVersionConverter;
use crate::enums::comparator::Comparator;
use crate::{VersError, VersioningScheme};
use mvn_version::ComparableVersion;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ordering;
use std::fmt::{self, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::str::FromStr;

#[derive(Clone, Debug, Eq)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct MavenVersion {
    raw: String,
}

impl Default for MavenVersion {
    fn default() -> Self {
        MavenVersion {
            raw: "0".to_string(),
        }
    }
}

impl Serialize for MavenVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.raw)
    }
}

impl<'de> Deserialize<'de> for MavenVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl NativeVersionConverter for MavenVersion {
    fn scheme_name() -> &'static str {
        VersioningScheme::Maven.as_ref()
    }

    fn versioning_schemes() -> Option<&'static [VersioningScheme]> {
        Some(&[VersioningScheme::Maven])
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

        // Official Maven bracket/parenthesis range syntax ([...], (...))
        if raw.starts_with('[') || raw.starts_with('(') {
            return parse_maven_unions(raw);
        }

        let clauses: Vec<&str> = if raw.contains(',') {
            raw.split(',').collect()
        } else {
            vec![raw]
        };

        let mut constraints = Vec::new();
        for clause in clauses {
            let constraint = Self::from_native_constraint(clause.trim())?;
            constraints.push(constraint);
        }

        if constraints.is_empty() {
            return Err(VersError::EmptyConstraints);
        }

        Ok(constraints)
    }

    fn from_native_constraint(raw: &str) -> Result<VersionConstraint<Self>, VersError> {
        let raw = raw.trim();

        if raw.is_empty() {
            return Err(VersError::InvalidConstraint("Empty constraint".to_string()));
        }
        if Self::native_wildcard() == Some(raw) {
            return Ok(VersionConstraint::new(Comparator::Any, Self::default()));
        }
        let (comparator, version_str) = if let Some(stripped) = raw.strip_prefix(">=") {
            (Comparator::GreaterThanOrEqual, stripped)
        } else if let Some(stripped) = raw.strip_prefix("<=") {
            (Comparator::LessThanOrEqual, stripped)
        } else if let Some(stripped) = raw.strip_prefix('>') {
            (Comparator::GreaterThan, stripped)
        } else if let Some(stripped) = raw.strip_prefix('<') {
            (Comparator::LessThan, stripped)
        } else if let Some(stripped) = raw.strip_prefix("==") {
            (Comparator::Equal, stripped)
        } else if let Some(stripped) = raw.strip_prefix('=') {
            (Comparator::Equal, stripped)
        } else {
            (Comparator::Equal, raw)
        };

        let version_str = version_str.trim();
        if version_str.is_empty() {
            return Err(VersError::InvalidConstraint("Missing version".to_string()));
        }

        let parsed_version = MavenVersion::from_str(version_str)?;

        Ok(VersionConstraint::new(comparator, parsed_version))
    }
}

impl FromStr for MavenVersion {
    type Err = VersError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.is_empty() {
            return Err(VersError::InvalidConstraint(
                "Empty version string".to_string(),
            ));
        }

        ComparableVersion::from_str(s).map_err(|e| {
            VersError::InvalidConstraint(format!("Invalid Maven version format '{}': {}", s, e))
        })?;

        Ok(MavenVersion { raw: s.to_string() })
    }
}

impl Display for MavenVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

impl Hash for MavenVersion {
    fn hash<H: Hasher>(&self, state: &mut H) {
        ComparableVersion::from_str(&self.raw)
            .expect("MavenVersion stores only parsed versions")
            .hash(state);
    }
}

impl PartialEq for MavenVersion {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl PartialOrd for MavenVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MavenVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.raw == other.raw {
            return Ordering::Equal;
        }

        match (
            ComparableVersion::from_str(&self.raw),
            ComparableVersion::from_str(&other.raw),
        ) {
            (Ok(a), Ok(b)) => a.cmp(&b),
            _ => self.raw.cmp(&other.raw),
        }
    }
}

/// Parses Maven union ranges separated by commas outside of bracket groups,
/// preserving the exact bounds and inclusivity of each sub-interval (e.g., `(,1.0.0],[1.2.0,]`).
pub fn parse_maven_unions(raw: &str) -> Result<Vec<VersionConstraint<MavenVersion>>, VersError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(VersError::EmptyConstraints);
    }

    let intervals = split_maven_intervals(raw);
    let mut all_constraints = Vec::new();

    for interval_str in intervals {
        let interval_str = interval_str.trim();
        if interval_str.is_empty() {
            return Err(VersError::InvalidConstraint(
                "Empty Maven interval".to_string(),
            ));
        }

        let constraints = parse_single_maven_interval(interval_str)?;
        all_constraints.extend(constraints);
    }

    if all_constraints.is_empty() {
        return Err(VersError::EmptyConstraints);
    }

    Ok(all_constraints)
}

/// Splits Maven intervals at commas while ignoring commas inside `[...]` or `(...)` brackets
/// to properly support multi-interval union ranges.
fn split_maven_intervals(s: &str) -> Vec<String> {
    let mut intervals = Vec::new();
    let mut current = String::new();
    let mut depth = 0;

    for c in s.chars() {
        match c {
            '[' | '(' => {
                depth += 1;
                current.push(c);
            }
            ']' | ')' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => {
                intervals.push(current);
                current = String::new();
            }
            _ => {
                current.push(c);
            }
        }
    }
    if !current.is_empty() || s.ends_with(',') {
        intervals.push(current);
    }
    intervals
}

/// Parses a single Maven interval (e.g., `[1.0,2.0)` or `(,1.0]`) into constraints,
/// correctly maintaining inclusivity rules for lower and upper bounds.
fn parse_single_maven_interval(
    raw: &str,
) -> Result<Vec<VersionConstraint<MavenVersion>>, VersError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(VersError::EmptyConstraints);
    }

    // Handle single version shorthand in Maven like `[1.0]`, mapping it to equality
    if raw.starts_with('[') && raw.ends_with(']') && !raw[1..raw.len() - 1].contains(',') {
        let ver_str = raw[1..raw.len() - 1].trim();
        let version = MavenVersion::from_str(ver_str)?;
        return Ok(vec![VersionConstraint::new(Comparator::Equal, version)]);
    }

    let first_char = raw.chars().next();
    let last_char = raw.chars().last();
    if raw.len() < 2
        || !matches!(first_char, Some('[' | '('))
        || !matches!(last_char, Some(']' | ')'))
    {
        return Err(VersError::InvalidConstraint(format!(
            "Invalid Maven interval format: '{}'",
            raw
        )));
    }

    let parts: Vec<&str> = raw.split(',').map(str::trim).collect();
    if parts.len() > 2 {
        return Err(VersError::InvalidConstraint(format!(
            "Invalid Maven interval format: '{}'",
            raw
        )));
    }

    let lower_str = parts
        .first()
        .map(|s| s.trim_start_matches(['[', '(']).trim())
        .unwrap_or("");
    let upper_str = parts
        .get(1)
        .map(|s| s.trim_end_matches([']', ')']).trim())
        .unwrap_or("");

    let lower_ver = if lower_str.is_empty() {
        None
    } else {
        Some(MavenVersion::from_str(lower_str)?)
    };
    let upper_ver = if upper_str.is_empty() {
        None
    } else {
        Some(MavenVersion::from_str(upper_str)?)
    };

    if let (Some(low), Some(high)) = (&lower_ver, &upper_ver)
        && low == high
    {
        if first_char == Some('[') && last_char == Some(']') {
            return Ok(vec![VersionConstraint::new(Comparator::Equal, low.clone())]);
        }
        return Err(VersError::InvalidConstraint(format!(
            "Empty Maven interval with equal bounds: '{}'",
            raw
        )));
    }

    let mut constraints = Vec::new();

    if let Some(low) = lower_ver {
        let comp = if first_char == Some('[') {
            Comparator::GreaterThanOrEqual
        } else {
            Comparator::GreaterThan
        };
        constraints.push(VersionConstraint::new(comp, low));
    }

    if let Some(high) = upper_ver {
        let comp = if last_char == Some(']') {
            Comparator::LessThanOrEqual
        } else {
            Comparator::LessThan
        };
        constraints.push(VersionConstraint::new(comp, high));
    }

    if constraints.is_empty() {
        return Err(VersError::EmptyConstraints);
    }

    Ok(constraints)
}

#[cfg(test)]
mod tests {
    use super::{MavenVersion, parse_maven_unions, parse_single_maven_interval};
    use crate::range::VersionRange;
    use crate::range::dynamic::DynamicVersionRange;
    use crate::{Comparator, NativeVersionConverter};
    use std::str::FromStr;

    #[test]
    fn test_maven_parse_simple() {
        let range = DynamicVersionRange::parse_native("maven", "1.0.0").unwrap();
        assert_eq!(range.versioning_scheme(), "maven");
        assert_eq!(range.constraints().len(), 1);
        assert_eq!(range.constraints()[0].comparator, Comparator::Equal);
        assert_eq!(range.constraints()[0].version.to_string(), "1.0.0");
    }

    #[test]
    fn test_maven_version_ordering() {
        let v1 = MavenVersion::from_str("1.0.0").unwrap();
        let v2 = MavenVersion::from_str("1.1.0").unwrap();
        assert!(v1 < v2);
    }

    #[test]
    fn test_maven_version_ordering_numeric() {
        let v_nine = MavenVersion::from_str("1.9.0").unwrap();
        let v_ten = MavenVersion::from_str("1.10.0").unwrap();

        assert!(v_nine < v_ten);
    }

    #[test]
    fn test_maven_version_ordering_qualifiers() {
        let snapshot = MavenVersion::from_str("1.0.0-SNAPSHOT").unwrap();
        let release = MavenVersion::from_str("1.0.0").unwrap();

        assert!(snapshot < release);
    }

    #[test]
    fn test_maven_union_ranges_parsing() {
        let constraints = parse_maven_unions("(,1.0.0],[1.2.0,)").unwrap();
        assert!(!constraints.is_empty());
    }

    #[test]
    fn test_maven_single_interval_bounds() {
        let constraints = parse_single_maven_interval("[1.0.0,2.0.0)").unwrap();
        assert_eq!(constraints.len(), 2);

        assert_eq!(constraints[0].comparator, Comparator::GreaterThanOrEqual);
        assert_eq!(constraints[0].version.to_string(), "1.0.0");

        assert_eq!(constraints[1].comparator, Comparator::LessThan);
        assert_eq!(constraints[1].version.to_string(), "2.0.0");
    }

    #[test]
    fn test_maven_single_version_shorthand() {
        let constraints = parse_single_maven_interval("[1.5.0]").unwrap();
        assert_eq!(constraints.len(), 1);
        assert_eq!(constraints[0].comparator, Comparator::Equal);
        assert_eq!(constraints[0].version.to_string(), "1.5.0");
    }

    #[test]
    fn test_maven_control_characters_error() {
        let res = MavenVersion::from_native("1.0.0\n");
        assert!(res.is_err());

        let res_tab = MavenVersion::from_native("1.0.0\t");
        assert!(res_tab.is_err());
    }

    #[test]
    fn test_maven_empty_constraints_error() {
        let res = MavenVersion::from_native("   ");
        assert!(res.is_err());
    }

    #[test]
    fn test_maven_empty_union_member_rejected() {
        assert!(parse_maven_unions("[1.0.0,2.0.0],").is_err());
        assert!(parse_maven_unions(",[1.0.0,2.0.0]").is_err());
        assert!(parse_maven_unions("[1.0.0],,[2.0.0]").is_err());
    }

    #[test]
    fn test_maven_equal_bounds_collapsed_or_rejected() {
        let constraints = parse_single_maven_interval("[1.0.0,1.0.0]").unwrap();
        assert_eq!(constraints.len(), 1);
        assert_eq!(constraints[0].comparator, Comparator::Equal);
        assert_eq!(constraints[0].version.to_string(), "1.0.0");

        assert!(parse_single_maven_interval("[1.0.0,1.0.0)").is_err());
        assert!(parse_single_maven_interval("(1.0.0,1.0.0]").is_err());
        assert!(parse_single_maven_interval("(1.0.0,1.0.0)").is_err());
    }
}
