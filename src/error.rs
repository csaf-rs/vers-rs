//! Error types for the vers-rs library.
//!
//! This module contains the error types used throughout the library.
//! The main error type is `VersError`, which represents all possible
//! errors that can occur when working with version range specifiers.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(feature = "wasm")]
use {js_sys::Error as JsError, tsify::Tsify, wasm_bindgen::JsValue};

/// Errors that can occur when working with version range specifiers.
///
/// This enum represents all the possible errors that can occur when parsing,
/// validating, or using version range specifiers.
#[derive(Error, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(Tsify))]
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
    InvalidVersionFormat(String, String, String),
}

/// Convert VersError into a JS exception value when targeting wasm.
#[cfg(feature = "wasm")]
impl From<VersError> for JsValue {
    fn from(e: VersError) -> JsValue {
        JsValue::from(JsError::new(&e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vers_error_round_trips_through_json() {
        let original = VersError::InvalidVersionFormat(
            "deb".to_string(),
            "<<1.0.0!".to_string(),
            "unexpected character '!'".to_string(),
        );

        // Serialisation works fine, as Serialize has no lifetime constraints
        let json: String = serde_json::to_string(&original).unwrap();
        assert_eq!(
            json,
            r#"{"InvalidVersionFormat":["deb","<<1.0.0!","unexpected character '!'"]}"#
        );

        // Deserialisation from a runtime String successfully compiles and roundtrips
        let roundtripped: VersError = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtripped, original);
    }
}
