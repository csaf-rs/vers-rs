//! Comparator type for the vers-rs library.
//!
//! The `Comparator` enum represents the different types of comparators that can be used
//! in version constraints, such as `""` (empty string), `!=`, `<`, `<=`, `>`, `>=`, and `*`.

/// Comparator for version constraints.
///
/// This enum represents the different types of comparators that can be used
/// in version constraints. Each comparator defines how a version is compared
/// to the constraint version.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, strum::Display,
)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub enum Comparator {
    /// Equal (=) - The version must be exactly equal to the constraint version.
    #[strum(to_string = "")]
    Equal,
    /// Not equal (!=) - The version must not be equal to the constraint version.
    #[strum(to_string = "!=")]
    NotEqual,
    /// Less than (<) - The version must be less than the constraint version.
    #[strum(to_string = "<")]
    LessThan,
    /// Less than or equal (<=) - The version must be less than or equal to the constraint version.
    #[strum(to_string = "<=")]
    LessThanOrEqual,
    /// Greater than (>) - The version must be greater than the constraint version.
    #[strum(to_string = ">")]
    GreaterThan,
    /// Greater than or equal (>=) - The version must be greater than or equal to the constraint version.
    #[strum(to_string = ">=")]
    GreaterThanOrEqual,
    /// Any version (*) - Matches any version. Must be used alone.
    #[strum(to_string = "*")]
    Any,
}
