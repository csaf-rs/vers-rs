//! Version range specifier implementation for the vers-rs library.
//!
//! This module contains the `VersionRange` struct and its methods for parsing,
//! validating, normalizing, and checking version ranges.
//!
//! A version range specifier consists of a versioning scheme and a list of version
//! constraints. It defines a set of versions that satisfy the constraints.
//!
//! The format is: `vers:<versioning-scheme>/<version-constraint>|<version-constraint>|...`
//!
//! Examples:
//! - `vers:npm/1.2.3` (a single version)
//! - `vers:npm/>=1.0.0|<2.0.0` (a range of versions)
//! - `vers:pypi/*` (any version)
//!
//! The `VersionRange` struct provides methods for:
//! - Creating a new version range with `new`
//! - Normalizing and validating a version range with `normalize_and_validate`
//! - Checking if a version is within a range with `contains`
//!
//! It also implements `FromStr` for parsing a string into a `VersionRange` and
//! `Display` for converting a `VersionRange` back to a string.

use crate::Comparator::*;
use crate::DynamicVersionRange;
use crate::VersionConstraint;
use crate::constraint::VersionType;
use crate::range::VersionRange;
use crate::util::ascii_whitespace::is_ascii_vers_whitespace;
use crate::{Comparator, VersError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::VecDeque;
use std::fmt;
use std::fmt::Display;
use std::str::FromStr;

/// A version range specifier.
///
/// A version range specifier consists of a versioning scheme and a list of version constraints.
/// It defines a set of versions that satisfy the constraints.
///
/// The format is: `vers:<versioning-scheme>/<version-constraint>|<version-constraint>|...`
///
/// Examples:
/// - `vers:npm/1.2.3` (a single version)
/// - `vers:npm/>=1.0.0|<2.0.0` (a range of versions)
/// - `vers:pypi/*` (any version)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawVersVersionRange<V>")]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct VersVersionRange<V: VersionType> {
    /// The versioning scheme (e.g., "npm", "pypi", "maven", "deb")
    pub versioning_scheme: String,

    /// The list of version constraints
    pub constraints: Vec<VersionConstraint<V>>,
}
/// Raw JSON serde helper structure for VersVersionRange to normalize and validate on "Deserialize"
#[derive(Deserialize)]
struct RawVersVersionRange<V: VersionType> {
    versioning_scheme: String,
    constraints: Vec<VersionConstraint<V>>,
}

impl<V: VersionType> TryFrom<RawVersVersionRange<V>> for VersVersionRange<V> {
    type Error = VersError;

    fn try_from(raw: RawVersVersionRange<V>) -> Result<Self, Self::Error> {
        let mut range = VersVersionRange {
            versioning_scheme: raw.versioning_scheme,
            constraints: raw.constraints,
        };
        range.normalize_and_validate()?;
        Ok(range)
    }
}

impl<V: VersionType> VersionRange<V> for VersVersionRange<V> {
    /// Get the versioning scheme used by this range.
    ///
    /// # Returns
    ///
    /// The versioning scheme string (e.g., "npm", "semver")
    fn versioning_scheme(&self) -> &str {
        self.versioning_scheme.as_str()
    }

    /// Check if a version is contained within this range.
    ///
    /// This method implements the algorithm described in the specification to check
    /// if a version is contained within the range. A version is contained within a
    /// range if it satisfies any of the constraints.
    ///
    /// The algorithm:
    /// 1. If the constraint list contains only "*", then the version is in the range
    /// 2. Check for exact matches with equality comparators
    /// 3. Check for exact matches with inequality comparators
    /// 4. Check range constraints (>, >=, <, <=) to see if the version falls within any interval
    ///
    /// # Arguments
    ///
    /// * `version` - The version string to check
    ///
    /// # Returns
    ///
    /// A `Result` containing a boolean indicating whether the version is in the range
    ///
    /// # Examples
    ///
    /// ```
    /// use vers_rs::{parse, VersVersionRange};
    /// use vers_rs::range::VersionRange;
    /// use vers_rs::schemes::semver::SemVer;
    ///
    /// let range = "vers:npm/>=1.0.0|<2.0.0".parse::<VersVersionRange<SemVer>>().unwrap();
    /// assert!(range.contains("1.5.0".parse().unwrap()).unwrap());
    /// assert!(!range.contains("2.0.0".parse().unwrap()).unwrap());
    /// ```
    fn contains(&self, version: V) -> Result<bool, VersError> {
        // If there are no constraints, nothing is contained
        if self.constraints.is_empty() {
            return Ok(false);
        }

        // If the constraint list contains only "*", then the version is in the range
        if self.constraints.len() == 1 && self.constraints[0].comparator == Any {
            return Ok(true);
        }

        // Check for exact matches with equality and inequality comparators
        for constraint in &self.constraints {
            match constraint.comparator {
                Equal | GreaterThanOrEqual | LessThanOrEqual => {
                    if version == constraint.version {
                        return Ok(true);
                    }
                }
                NotEqual if version == constraint.version => {
                    return Ok(false);
                }
                _ => {}
            }
        }

        // If there are only NotEqual constraints (and at least one exists),
        // and we've checked them all without returning, then the version is in the range
        if self.constraints.iter().all(|c| c.comparator == NotEqual) {
            return Ok(true);
        }

        // Get range constraints
        let mut range_iterator = self
            .constraints
            .iter()
            .filter(|c| {
                matches!(
                    c.comparator,
                    LessThan | LessThanOrEqual | GreaterThan | GreaterThanOrEqual
                )
            })
            .peekable();

        // Iterate over pairs of range constraints
        let mut first = true;
        while let Some(current) = range_iterator.next() {
            // If this is the first iteration and the current comparator is "<" or "<="
            // and the tested version is less than the current version
            if first {
                if (current.comparator == LessThan || current.comparator == LessThanOrEqual)
                    && version < current.version
                {
                    return Ok(true);
                }
                first = false;
            }

            // If this is the last iteration and the current comparator is ">" or ">="
            // and the tested version is greater than the current version
            if range_iterator.peek().is_none()
                && (current.comparator == GreaterThan || current.comparator == GreaterThanOrEqual)
                && version > current.version
            {
                return Ok(true);
            }

            // If there's a next constraint
            if let Some(next) = range_iterator.peek() {
                // If the current comparator is ">" or ">=" and the next comparator is "<" or "<="
                // and the tested version is greater than the current version
                // and the tested version is less than the next version
                if matches!(current.comparator, GreaterThan | GreaterThanOrEqual)
                    && version > current.version
                    && matches!(next.comparator, LessThan | LessThanOrEqual)
                    && version < next.version
                {
                    return Ok(true);
                }
            }
        }

        // If we get here, the version is not in the range
        Ok(false)
    }

    fn constraints(&self) -> &Vec<VersionConstraint<V>> {
        &self.constraints
    }
}

impl<V: VersionType> VersVersionRange<V> {
    /// Create a new version range with the given versioning scheme and constraints.
    ///
    /// # Arguments
    ///
    /// * `versioning_scheme` - The versioning scheme to use (e.g., "npm", "pypi", "maven", "deb")
    /// * `constraints` - The list of version constraints
    ///
    /// # Returns
    ///
    /// A new `VersionRange` instance
    pub fn new(versioning_scheme: String, constraints: Vec<VersionConstraint<V>>) -> Self {
        Self {
            versioning_scheme,
            constraints,
        }
    }

    /// Normalize and validate the version range in a single operation.
    ///
    /// This method first normalizes the version range by sorting and simplifying constraints,
    /// then validates the resulting normalized range according to the rules in the specification.
    ///
    /// # Returns
    ///
    /// A `Result` indicating whether the normalization and validation were successful or not
    pub fn normalize_and_validate(&mut self) -> Result<(), VersError> {
        // Check if constraints are empty
        if self.constraints.is_empty() {
            return Err(VersError::EmptyConstraints);
        }

        // Check for star constraint
        let has_star = self.constraints.iter().any(|c| c.comparator == Any);
        if has_star && self.constraints.len() > 1 {
            return Err(VersError::InvalidRange(
                "Star constraint must be used alone".to_string(),
            ));
        }

        // If there's only one constraint, no need for further validation
        if self.constraints.len() == 1 {
            return Ok(());
        }

        self.constraints.sort_by(|a, b| a.version.cmp(&b.version));

        // Check for duplicate versions, exploiting sorted order
        for i in 1..self.constraints.len() {
            if self.constraints[i].version == self.constraints[i - 1].version {
                return Err(VersError::DuplicateVersions(
                    self.constraints[i].version.to_string(),
                ));
            }
        }

        // First, let's perform normalization and simplification according to the README spec

        // Split constraints into unequal constraints and other constraints
        let mut unequal_constraints: Vec<VersionConstraint<V>> = Vec::new();
        let mut other_constraints: VecDeque<VersionConstraint<V>> = VecDeque::new();

        for constraint in self.constraints.drain(..) {
            if constraint.comparator == NotEqual {
                unequal_constraints.push(constraint);
            } else {
                other_constraints.push_back(constraint);
            }
        }

        // If there are no other constraints, just return the unequal constraints
        if other_constraints.is_empty() {
            self.constraints = unequal_constraints;
            return Ok(());
        }

        let mut filtered_constraints: Vec<VersionConstraint<V>> = Vec::new();

        // Take the current element by removing it from the list front
        while let Some(current) = other_constraints.pop_front() {
            // Check the next constraint if it exists
            if let Some(next) = other_constraints.front() {
                // If the current comparator is ">" or ">=" and next comparator is "=", ">" or ">=",
                // discard the next constraint
                if matches!(current.comparator, GreaterThan | GreaterThanOrEqual)
                    && matches!(next.comparator, GreaterThan | GreaterThanOrEqual | Equal)
                {
                    // Discard the next constraint
                    other_constraints.pop_front();
                    // Re-evaluate, keeping the current constraint (re-add)
                    other_constraints.push_front(current);
                    continue;
                }

                // If the current comparator is "=", "<" or "<=" and next comparator is <" or <=",
                // discard the current constraint
                if matches!(current.comparator, Equal | LessThan | LessThanOrEqual)
                    && matches!(next.comparator, LessThan | LessThanOrEqual)
                {
                    // Previous constraint becomes current if it exists
                    if let Some(previous) = filtered_constraints.pop() {
                        other_constraints.push_front(previous);
                    }
                    continue;
                }

                // Check the previous constraint if it exists
                if let Some(previous) = filtered_constraints.last() {
                    // If the previous comparator is ">" or ">=" and current comparator
                    // is "=", ">" or ">=", discard the current constraint
                    if matches!(previous.comparator, GreaterThan | GreaterThanOrEqual)
                        && matches!(current.comparator, GreaterThan | GreaterThanOrEqual | Equal)
                    {
                        // Discard the current constraint
                        continue;
                    }

                    // If the previous comparator is "=", "<" or "<=" and current comparator
                    // is "<" or "<=", discard the previous constraint
                    if matches!(previous.comparator, Equal | LessThan | LessThanOrEqual)
                        && matches!(current.comparator, LessThan | LessThanOrEqual)
                    {
                        // Discard the previous constraint
                        filtered_constraints.pop();
                    }
                }
            }

            filtered_constraints.push(current);
        }

        // Ignoring all constraints with "!=" comparators:
        // A "=" constraint must be followed only by a constraint with one of "=", ">", ">="
        // as comparator (or no constraint).
        let mut filter_iter = filtered_constraints.iter().map(|c| c.comparator).peekable();
        while let Some(current) = filter_iter.next() {
            if let Some(next) = filter_iter.peek()
                && current == Equal
                && !matches!(*next, Equal | GreaterThan | GreaterThanOrEqual)
            {
                return Err(VersError::InvalidRange(format!(
                    "\"{}\" must not be followed by \"{}\" in a normalized range \
                        (ignoring \"!=\")",
                    current, next,
                )));
            }
        }

        // And ignoring all constraints with "=" or "!=" comparators, the sequence of
        // constraint comparators must be an alternation of greater and lesser comparators:
        let mut filter_iter = filtered_constraints
            .iter()
            .map(|c| c.comparator)
            .filter(|c| *c != Equal)
            .peekable();
        while let Some(current) = filter_iter.next() {
            if let Some(next) = filter_iter.peek() {
                match current {
                    // "<" and "<=" must be followed by one of ">", ">=" (or no constraint).
                    LessThan | LessThanOrEqual => match next {
                        GreaterThan | GreaterThanOrEqual => {}
                        _ => {
                            return Err(VersError::InvalidRange(format!(
                                "\"{}\" must not be followed by \"{}\" in a normalized range \
                                (ignoring \"!=\" and \"=\")",
                                current, next,
                            )));
                        }
                    },
                    // ">" and ">=" must be followed by one of "<", "<=" (or no constraint).
                    GreaterThan | GreaterThanOrEqual => match next {
                        LessThan | LessThanOrEqual => {}
                        _ => {
                            return Err(VersError::InvalidRange(format!(
                                "\"{}\" must not be followed by \"{}\" in a normalized range \
                                (ignoring \"!=\" and \"=\")",
                                current, next,
                            )));
                        }
                    },
                    _ => {}
                }
            }
        }

        // Combine unequal constraints and filtered constraints
        filtered_constraints.extend(unequal_constraints);

        // Sort by version for the final normalized form
        filtered_constraints.sort_by(|a, b| a.version.cmp(&b.version));

        self.constraints = filtered_constraints;

        Ok(())
    }
}

/// Returns an error if any pair of comparators adjacent in `constraints` (already
/// filtered/ordered by version) matches `is_invalid`. Used to reject comparator
/// sequences that are redundant or nonsensical once irrelevant comparators
/// (e.g. "!=", "=") have been filtered out.
fn reject_adjacent_comparators<V: VersionType>(
    constraints: &[&VersionConstraint<V>],
    is_invalid: impl Fn(Comparator, Comparator) -> bool,
) -> Result<(), VersError> {
    for pair in constraints.windows(2) {
        let (current, next) = (pair[0], pair[1]);
        if is_invalid(current.comparator, next.comparator) {
            return Err(VersError::ConsecutiveOperators(format!(
                "{0} | {1}",
                current.comparator, next.comparator
            )));
        }
    }
    Ok(())
}

impl<V: VersionType> FromStr for VersVersionRange<V>
where
    V::Err: Into<VersError>,
{
    type Err = VersError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.is_ascii() {
            return Err(VersError::InvalidCharacters(
                "A VERS string shall only contain ASCII characters".to_string(),
            ));
        }

        if s.bytes().any(is_ascii_vers_whitespace) {
            return Err(VersError::InvalidCharacters(
                "Control characters (tabs, newlines, carriage returns) are not permitted"
                    .to_string(),
            ));
        }

        let (versioning_scheme, constraints_str) =
            DynamicVersionRange::extract_versioning_scheme(s)?;

        // check constraints
        if constraints_str.is_empty() {
            return Err(VersError::EmptyConstraints);
        }

        if constraints_str == "*" {
            return Ok(Self {
                versioning_scheme,
                constraints: vec![VersionConstraint::new(Comparator::Any, V::default())],
            });
        }

        // check multiple asterisks
        if constraints_str.chars().filter(|c| *c == '*').count() > 1 {
            return Err(VersError::MultipleAsterisks(constraints_str.to_string()));
        }

        // Split on '|' and delegate each segment to VersionConstraint::parse
        let segments: Vec<&str> = constraints_str.split('|').collect();
        let mut constraints = Vec::new();
        let mut seen_versions: HashMap<V, Vec<VersionConstraint<V>>> = HashMap::new();
        for segment in segments {
            if segment.is_empty() {
                return Err(VersError::EmptyConstraints);
            }
            let constraint = VersionConstraint::<V>::parse(segment)?;
            seen_versions
                .entry(constraint.version.clone())
                .or_default()
                .push(constraint.clone());

            constraints.push(constraint);
        }

        // check single asterisk
        if constraints.len() > 1 && constraints.iter().any(|c| c.comparator == Comparator::Any) {
            return Err(VersError::InvalidRange(
                "Star constraint must be used alone".to_string(),
            ));
        }

        // check duplicate versions
        let duplicate_versions: Vec<VersionConstraint<V>> = seen_versions
            .values()
            .filter(|v| v.len() > 1)
            .filter_map(|v| v.first().cloned())
            .collect();
        if !duplicate_versions.is_empty() {
            let duplicates: Vec<String> = duplicate_versions
                .iter()
                .map(|c| c.version.to_string())
                .collect();
            return Err(VersError::DuplicateVersions(
                duplicates.join(", ").to_string(),
            ));
        }

        // check ordering
        let check_order_result = constraints.windows(2).try_for_each(|pair| {
            let (a, b) = (&pair[0], &pair[1]);
            if a.version > b.version {
                Err(VersError::UnorderedVersions(
                    a.version.to_string(),
                    b.version.to_string(),
                ))
            } else {
                Ok(())
            }
        });
        check_order_result?;

        // non-equal is irrelevant for further comparator checks, as everything is allowed after non-equal
        // and should be ignored for the purpose of checking consecutive operators
        let constraints_without_non_equal: Vec<&VersionConstraint<V>> = constraints
            .iter()
            .filter(|c| c.comparator != Comparator::NotEqual)
            .collect();
        // check lessthan after equals and equals after greaterthan
        reject_adjacent_comparators(&constraints_without_non_equal, |current, next| {
            matches!(
                (current, next),
                // = | <= makes no sense, as the = constraint is already covered by the < constraint and the versions must be ordered at this point
                (Comparator::Equal, Comparator::LessThan)
                | (Comparator::Equal, Comparator::LessThanOrEqual)
                // >= | = makes no sense, as the = constraint is already covered by the > constraint and the versions must be ordered at this point
                | (Comparator::GreaterThan, Comparator::Equal)
                | (Comparator::GreaterThanOrEqual, Comparator::Equal)
            )
        })?;

        let constraints_without_non_equal_and_equal: Vec<&VersionConstraint<V>> =
            constraints_without_non_equal
                .into_iter()
                .filter(|c| c.comparator != Comparator::Equal)
                .collect();
        // check for redundant consecutive "<"/"<=" or ">"/">=" constraints
        reject_adjacent_comparators(&constraints_without_non_equal_and_equal, |current, next| {
            matches!(
                (current, next),
                // < | < or > | > makes no sense, as the versions must be ordered at this point
                (Comparator::LessThan, Comparator::LessThan)
                    | (Comparator::LessThan, Comparator::LessThanOrEqual)
                    | (Comparator::LessThanOrEqual, Comparator::LessThan)
                    | (Comparator::LessThanOrEqual, Comparator::LessThanOrEqual)
                    | (Comparator::GreaterThan, Comparator::GreaterThan)
                    | (Comparator::GreaterThan, Comparator::GreaterThanOrEqual)
                    | (Comparator::GreaterThanOrEqual, Comparator::GreaterThan)
                    | (
                        Comparator::GreaterThanOrEqual,
                        Comparator::GreaterThanOrEqual
                    )
            )
        })?;

        let range = Self {
            versioning_scheme,
            constraints,
        };

        Ok(range)
    }
}

impl<V: VersionType> Display for VersVersionRange<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "vers:{}/", self.versioning_scheme)?;

        if self.constraints.is_empty() {
            return Ok(());
        }

        // Convert all constraints to their string representations, handling 'Any' as '*'
        let constraint_strs: Vec<String> = self
            .constraints
            .iter()
            .map(|c| {
                if c.comparator == Comparator::Any {
                    "*".to_string()
                } else {
                    c.to_string()
                }
            })
            .collect();

        // Join them together with '|'
        write!(f, "{}", constraint_strs.join("|"))
    }
}

#[cfg(test)]
mod tests {
    use super::VersVersionRange;
    use crate::Comparator;
    use crate::VersError;
    use crate::VersionConstraint;
    use crate::range::VersionRange;
    use crate::schemes::semver::SemVer;

    #[test]
    fn test_invalid_scheme() {
        let result: Result<VersVersionRange<SemVer>, _> = "foo:npm/1.2.3".parse();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), VersError::InvalidScheme);
    }

    #[test]
    fn test_normalize() {
        let mut range = VersVersionRange::<SemVer>::new(
            "npm".to_string(),
            vec![
                VersionConstraint::new(Comparator::GreaterThanOrEqual, "1.0.0".parse().unwrap()),
                VersionConstraint::new(Comparator::GreaterThan, "1.5.0".parse().unwrap()),
                VersionConstraint::new(Comparator::LessThan, "3.0.0".parse().unwrap()),
                VersionConstraint::new(Comparator::LessThanOrEqual, "2.0.0".parse().unwrap()),
            ],
        );

        match range.normalize_and_validate() {
            Ok(_) => {}
            Err(e) => panic!("{}", e),
        }

        assert_eq!(range.constraints().len(), 2);
        assert_eq!(
            range.constraints()[0].comparator,
            Comparator::GreaterThanOrEqual
        );
        assert_eq!(range.constraints()[0].version.to_string(), "1.0.0");
        assert_eq!(range.constraints()[1].comparator, Comparator::LessThan);
        assert_eq!(range.constraints()[1].version.to_string(), "3.0.0");
    }

    #[test]
    fn test_empty_constraints_display() {
        let range = VersVersionRange::<SemVer>::new("npm".to_string(), vec![]);
        let display_str = range.to_string();
        assert_eq!(display_str, "vers:npm/");
    }

    #[test]
    fn test_empty_constraints_contains() {
        let range = VersVersionRange::<SemVer>::new("npm".to_string(), vec![]);
        let result = range.contains("1.0.0".parse().unwrap());
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_deserialize_empty_constraints_fails() {
        // Construct a JSON representation of a range with empty constraints
        let json_data = r#"{
            "versioning_scheme": "npm",
            "constraints": []
        }"#;

        let result: Result<VersVersionRange<SemVer>, _> = serde_json::from_str(json_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_is_open_ended_bare_greater_than_or_equal() {
        let range: VersVersionRange<SemVer> = "vers:npm/>=1.0.0".parse().unwrap();
        assert!(range.is_open_ended());
    }

    #[test]
    fn test_is_open_ended_bare_greater_than() {
        let range: VersVersionRange<SemVer> = "vers:npm/>1.0.0".parse().unwrap();
        assert!(range.is_open_ended());
    }

    #[test]
    fn test_is_open_ended_bare_less_than_is_not_open_ended() {
        let range: VersVersionRange<SemVer> = "vers:npm/<1.0.0".parse().unwrap();
        assert!(!range.is_open_ended());
    }

    #[test]
    fn test_is_open_ended_bare_less_than_or_equal_is_not_open_ended() {
        let range: VersVersionRange<SemVer> = "vers:npm/<=1.0.0".parse().unwrap();
        assert!(!range.is_open_ended());
    }

    #[test]
    fn test_is_open_ended_bounded_range_is_not_open_ended() {
        let range: VersVersionRange<SemVer> = "vers:npm/>=1.0.0|<2.0.0".parse().unwrap();
        assert!(!range.is_open_ended());
    }

    #[test]
    fn test_is_open_ended_star_is_not_open_ended() {
        // "*" has no explicit ">" or ">=" bound, so it is not considered open-ended
        // even though it matches every version.
        let range: VersVersionRange<SemVer> = "vers:npm/*".parse().unwrap();
        assert!(!range.is_open_ended());
    }

    #[test]
    fn test_is_open_ended_empty_constraints_is_not_open_ended() {
        let range = VersVersionRange::<SemVer>::new("npm".to_string(), vec![]);
        assert!(!range.is_open_ended());
    }

    #[test]
    fn test_is_open_ended_bare_equal_is_not_open_ended() {
        let range: VersVersionRange<SemVer> = "vers:npm/1.2.3".parse().unwrap();
        assert!(!range.is_open_ended());
    }

    #[test]
    fn test_is_open_ended_only_not_equal_constraints_is_not_open_ended() {
        // No ">" or ">=" range constraint is present at all, so even though every
        // version except the excluded ones is contained, this is not "open-ended"
        // under our definition (it requires an explicit trailing ">"/">=" bound).
        let range: VersVersionRange<SemVer> = "vers:npm/!=1.0.0|!=2.0.0".parse().unwrap();
        assert!(!range.is_open_ended());
    }

    #[test]
    fn test_is_open_ended_trailing_not_equal_after_unbounded_greater_than_or_equal() {
        // Regression test for the edge case ">=0.0.2|!=0.0.8": a "!=" constraint can
        // sort *after* the final ">"/">=" bound (it has a higher version) without
        // affecting whether the range is unbounded above. Naively checking only the
        // last constraint in the (version-sorted) list would miss this.
        let range: VersVersionRange<SemVer> = "vers:npm/0.0.1|>=0.0.2|!=0.0.8".parse().unwrap();
        assert!(range.is_open_ended());
        assert!(range.contains("999.0.0".parse().unwrap()).unwrap());
        assert!(!range.contains("0.0.8".parse().unwrap()).unwrap());
    }

    #[test]
    fn test_is_open_ended_disjoint_range_ending_unbounded() {
        // Two disjoint intervals: below 1.0.0, or at/above 2.0.0 and beyond.
        let range: VersVersionRange<SemVer> = "vers:npm/<=1.0.0|>=2.0.0".parse().unwrap();
        assert!(range.is_open_ended());
    }

    #[test]
    fn test_is_open_ended_disjoint_range_ending_bounded() {
        // Two disjoint intervals, but the highest one is still capped from above.
        let range: VersVersionRange<SemVer> = "vers:npm/<1.0.0|>=2.0.0|<3.0.0".parse().unwrap();
        assert!(!range.is_open_ended());
    }
}
