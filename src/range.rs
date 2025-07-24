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

use crate::comparator::Comparator::*;
use crate::constraint::VT;
use crate::error::VersError;
use crate::VersionConstraint;
use std::collections::LinkedList;
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionRange<V : VT> {
    /// The versioning scheme (e.g., "npm", "pypi", "maven", "deb")
    pub versioning_scheme: String,
    
    /// The list of version constraints
    pub constraints: Vec<VersionConstraint<V>>,
}

impl<V : VT> VersionRange<V> {
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
        Self { versioning_scheme, constraints }
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
            return Err(VersError::InvalidRange("Star constraint must be used alone".to_string()));
        }

        // If there's only one constraint, no need for further validation
        if self.constraints.len() == 1 {
            return Ok(());
        }

        self.constraints.sort_by(|a, b| a.version.cmp(&b.version));

        // Check for duplicate versions, exploiting sorted order
        for i in 1..self.constraints.len() {
            if self.constraints[i].version == self.constraints[i - 1].version {
                return Err(VersError::DuplicateVersion(self.constraints[i].version.to_string()));
            }
        }

        // First, let's perform normalization and simplification according to the README spec

        // Split constraints into unequal constraints and other constraints
        let mut unequal_constraints: Vec<VersionConstraint<V>> = Vec::new();
        let mut other_constraints: LinkedList<VersionConstraint<V>> = LinkedList::new();

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
                if matches!(
                    current.comparator,
                    GreaterThan | GreaterThanOrEqual
                ) && matches!(
                    next.comparator,
                    GreaterThan | GreaterThanOrEqual | Equal
                ) {
                    // Discard the next constraint
                    other_constraints.pop_front();
                    // Re-evaluate, keeping the current constraint (re-add)
                    other_constraints.push_front(current);
                    continue;
                }

                // If the current comparator is "=", "<" or "<=" and next comparator is <" or <=",
                // discard the current constraint
                if matches!(current.comparator, Equal | LessThan | LessThanOrEqual)
                    && matches!(next.comparator, LessThan | LessThanOrEqual) {
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
                        && matches!(current.comparator, GreaterThan | GreaterThanOrEqual | Equal) {
                        // Discard the current constraint
                        continue;
                    }

                    // If the previous comparator is "=", "<" or "<=" and current comparator
                    // is "<" or "<=", discard the previous constraint
                    if matches!(previous.comparator, Equal | LessThan | LessThanOrEqual)
                        && matches!(current.comparator, LessThan | LessThanOrEqual) {
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
        let mut filter_iter = filtered_constraints
            .iter()
            .map(|c| c.comparator)
            .peekable();
        while let Some(current) = filter_iter.next() {
            if let Some(next) = filter_iter.peek() {
                if current == Equal && !matches!(*next, Equal | GreaterThan | GreaterThanOrEqual) {
                    return Err(VersError::InvalidRange(format!(
                        "\"{}\" must not be followed by \"{}\" in a normalized range \
                        (ignoring \"!=\")",
                        current,
                        next,
                    )))
                }
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
                    LessThan | LessThanOrEqual => {
                        match next {
                            GreaterThan | GreaterThanOrEqual => {},
                            _ => return Err(VersError::InvalidRange(format!(
                                "\"{}\" must not be followed by \"{}\" in a normalized range \
                                (ignoring \"!=\" and \"=\")",
                                current,
                                next,
                            )))
                        }
                    }
                    // ">" and ">=" must be followed by one of "<", "<=" (or no constraint).
                    GreaterThan | GreaterThanOrEqual => {
                        match next {
                            LessThan | LessThanOrEqual => {},
                            _ => return Err(VersError::InvalidRange(format!(
                                "\"{}\" must not be followed by \"{}\" in a normalized range \
                                (ignoring \"!=\" and \"=\")",
                                current,
                                next,
                            )))
                        }
                    }
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
    /// use vers_rs::{parse, VersionRange};
    /// use vers_rs::schemes::semver::SemVer;
    ///
    /// let range = "vers:npm/>=1.0.0|<2.0.0".parse::<VersionRange<SemVer>>().unwrap();
    /// assert!(range.contains(&"1.5.0".parse().unwrap()).unwrap());
    /// assert!(!range.contains(&"2.0.0".parse().unwrap()).unwrap());
    /// ```
    pub fn contains(&self, version: &V) -> Result<bool, VersError> {
        // If the constraint list contains only "*", then the version is in the range
        if self.constraints.len() == 1 && self.constraints[0].comparator == Any {
            return Ok(true);
        }
        
        // Check for exact matches with equality and inequality comparators
        for constraint in &self.constraints {
            match constraint.comparator {
                Equal | GreaterThanOrEqual | LessThanOrEqual => {
                    if version == &constraint.version {
                        return Ok(true);
                    }
                },
                NotEqual => {
                    if version == &constraint.version {
                        return Ok(false);
                    }
                },
                _ => {}
            }
        }

        // If there are only NotEqual constraints, and we've checked them all without returning,
        // then the version is in the range
        if self.constraints.iter().all(|c| c.comparator == NotEqual) {
            return Ok(true);
        }
        
        // Get range constraints
        let mut range_iterator = self.constraints.iter()
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
                if (current.comparator == LessThan || current.comparator == LessThanOrEqual) &&
                    version < &current.version
                {
                    return Ok(true);
                }
                first = false;
            }

            // If this is the last iteration and the current comparator is ">" or ">="
            // and the tested version is greater than the current version
            if range_iterator.peek().is_none() &&
                (current.comparator == GreaterThan || current.comparator == GreaterThanOrEqual) &&
                version > &current.version
            {
                return Ok(true);
            }
            
            // If there's a next constraint
            if let Some(next) = range_iterator.peek() {
                // If the current comparator is ">" or ">=" and the next comparator is "<" or "<="
                // and the tested version is greater than the current version
                // and the tested version is less than the next version
                if matches!(current.comparator, GreaterThan | GreaterThanOrEqual)
                    && version > &current.version
                    && matches!(next.comparator, LessThan | LessThanOrEqual)
                    && version < &next.version {
                    return Ok(true);
                }
            }
        }
        
        // If we get here, the version is not in the range
        Ok(false)
    }
}

impl<V : VT> FromStr for VersionRange<V> {
    type Err = VersError;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Remove all spaces and tabs
        let s = s.replace(|c: char| c.is_whitespace(), "");
        
        // Split on colon
        let parts: Vec<&str> = s.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(VersError::InvalidScheme);
        }
        
        // Validate URI scheme
        let scheme = parts[0];
        if scheme != "vers" {
            return Err(VersError::InvalidScheme);
        }
        
        // Split on slash
        let specifier_parts: Vec<&str> = parts[1].splitn(2, '/').collect();
        if specifier_parts.len() != 2 {
            return Err(VersError::MissingVersioningScheme);
        }
        
        // Get versioning scheme
        let versioning_scheme = specifier_parts[0].to_lowercase();
        if versioning_scheme.is_empty() {
            return Err(VersError::InvalidVersioningScheme(versioning_scheme));
        }
        
        // Get constraint string
        let constraints_str = specifier_parts[1].trim();
        if constraints_str.is_empty() {
            return Err(VersError::EmptyConstraints);
        }
        
        // Handle star constraint
        if constraints_str == "*" {
            return Ok(Self {
                versioning_scheme,
                constraints: vec![VersionConstraint::new(Any, V::default())],
            });
        }
        
        // Split constraints on each pipe
        let constraint_strs: Vec<&str> = constraints_str
            .trim_matches('|')
            .split('|')
            .filter(|s| !s.is_empty())
            .collect();
        
        if constraint_strs.is_empty() {
            return Err(VersError::EmptyConstraints);
        }

        // Parse each constraint
        let mut constraints = Vec::new();
        for constraint_str in constraint_strs {
            let constraint = VersionConstraint::<V>::parse(constraint_str)?;
            constraints.push(constraint);
        }
        
        let mut range = Self { versioning_scheme, constraints };
        range.normalize_and_validate()?;  // Use the combined function
        
        Ok(range)
    }
}

impl<V : VT> Display for VersionRange<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "vers:{}/", self.versioning_scheme)?;

        match self.constraints[0].comparator {
            Any => write!(f, "*")?,
            Equal => write!(f, "{}", self.constraints[0].version)?,
            _ => write!(f, "{}{}", self.constraints[0].comparator, self.constraints[0].version)?,
        }

        for constraint in &self.constraints[1..] {
            match constraint.comparator {
                Equal => write!(f, "|{}", constraint.version)?,
                _ => write!(f, "|{}{}", constraint.comparator, constraint.version)?,
            }
        }
        
        Ok(())
    }
}