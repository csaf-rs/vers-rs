//! Error types for the vers-rs library.
//!
//! This module contains the error types used throughout the library.
//! The main error type is `VersError`, which represents all possible
//! errors that can occur when working with version range specifiers.

use thiserror::Error;

/// Errors that can occur when working with version range specifiers.
///
/// This enum represents all the possible errors that can occur when parsing,
/// validating, or using version range specifiers.
#[derive(Error, Debug, PartialEq, Eq)]
pub enum VersError {
    #[error("Invalid URI scheme, expected 'vers'")]
    InvalidScheme,
    
    #[error("Missing versioning scheme")]
    MissingVersioningScheme,
    
    #[error("Empty version constraints")]
    EmptyConstraints,
    
    #[error("Invalid version constraint: {0}")]
    InvalidConstraint(String),
    
    #[error("Duplicate version: {0}")]
    DuplicateVersion(String),
    
    #[error("Invalid version range: {0}")]
    InvalidRange(String),
    
    #[error("Incompatible versioning schemes: {0} and {1}")]
    IncompatibleVersioningSchemes(String, String),
    
    #[error("Unsupported versioning scheme: {0}")]
    UnsupportedVersioningScheme(String),
    
    #[error("Invalid version format for scheme {0}: {1}, error was: {2}")]
    InvalidVersionFormat(&'static str, String, String),
}