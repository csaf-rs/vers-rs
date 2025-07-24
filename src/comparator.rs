//! Comparator type for the vers-rs library.
//!
//! The `Comparator` enum represents the different types of comparators that can be used
//! in version constraints, such as =, !=, <, <=, >, >=, and *.

use std::fmt;

/// Comparator for version constraints.
///
/// This enum represents the different types of comparators that can be used
/// in version constraints. Each comparator defines how a version is compared
/// to the constraint version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comparator {
    /// Equal (=) - The version must be exactly equal to the constraint version.
    Equal,
    /// Not equal (!=) - The version must not be equal to the constraint version.
    NotEqual,
    /// Less than (<) - The version must be less than the constraint version.
    LessThan,
    /// Less than or equal (<=) - The version must be less than or equal to the constraint version.
    LessThanOrEqual,
    /// Greater than (>) - The version must be greater than the constraint version.
    GreaterThan,
    /// Greater than or equal (>=) - The version must be greater than or equal to the constraint version.
    GreaterThanOrEqual,
    /// Any version (*) - Matches any version. Must be used alone.
    Any,
}

impl fmt::Display for Comparator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Comparator::Equal => write!(f, "="),
            Comparator::NotEqual => write!(f, "!="),
            Comparator::LessThan => write!(f, "<"),
            Comparator::LessThanOrEqual => write!(f, "<="),
            Comparator::GreaterThan => write!(f, ">"),
            Comparator::GreaterThanOrEqual => write!(f, ">="),
            Comparator::Any => write!(f, "*"),
        }
    }
}
