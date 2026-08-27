//! WASM ABI boundary for the `vers-rs` crate.
//!
//! This module exposes the crate's public API to JavaScript/TypeScript
//! consumers via `wasm-bindgen`. Values that implement [`tsify::Tsify`] are
//! wrapped in [`tsify::Ts`] rather than being passed across the ABI directly,
//! since `#[tsify(into_wasm_abi, from_wasm_abi)]` can leak memory when
//! (de)serialization fails, whereas `Ts<T>` handles that failure gracefully.

use crate::VersError;
use crate::range::VersionRange;
use crate::range::dynamic::DynamicVersionRange;
use tsify::Ts;
use tsify::Tsify;
use wasm_bindgen::prelude::*;

/// Parse a version range specifier string into a `DynamicVersionRange`.
///
/// This function automatically detects the versioning scheme and constructs
/// the appropriate typed version range.
///
/// # Arguments
///
/// * `s` - The version range specifier string to parse
///
/// # Returns
///
/// A `Result` containing either the parsed `DynamicVersionRange`, wrapped for
/// the wasm ABI, or an error.
#[wasm_bindgen]
pub fn parse(s: &str) -> Result<Ts<DynamicVersionRange>, VersError> {
    let range: DynamicVersionRange = s.parse()?;
    Ok(range.into_ts()?)
}

/// Check if a version string is contained within a dynamic version range.
///
/// This function checks if a version string satisfies the constraints defined
/// in a dynamic version range, automatically handling version parsing.
///
/// # Arguments
///
/// * `range` - The dynamic version range to check against
/// * `version_str` - The version string to check
///
/// # Returns
///
/// A `Result` containing a boolean indicating whether the version is in the range
#[wasm_bindgen]
pub fn contains(range: Ts<DynamicVersionRange>, version_str: String) -> Result<bool, VersError> {
    let range = range.to_rust()?;
    range.contains(version_str)
}

/// Parse a native range string for the given versioning scheme into a `DynamicVersionRange`.
///
/// This function accepts a scheme name and a native range string directly, without
/// requiring the `vers:scheme/` prefix. It delegates to the scheme's
/// [`crate::NativeVersionConverter`] implementation.
///
/// # Arguments
///
/// * `scheme` - The versioning scheme name (e.g. `"deb"`, `"semver"`, `"npm"`)
/// * `raw` - The native range string (e.g. `"<<1.0"`, `">=1.0.0|<2.0.0"`)
///
/// # Returns
///
/// A `Result` containing the parsed `DynamicVersionRange`, wrapped for the
/// wasm ABI, or an error.
#[wasm_bindgen]
pub fn parse_native(scheme: &str, raw: &str) -> Result<Ts<DynamicVersionRange>, VersError> {
    let range = DynamicVersionRange::parse_native(scheme, raw)?;
    Ok(range.into_ts()?)
}
