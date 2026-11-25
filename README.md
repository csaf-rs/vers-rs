# vers-rs

[![Build](https://github.com/csaf-rs/vers-rs/actions/workflows/build.yml/badge.svg?branch=main)](https://github.com/csaf-rs/vers-rs/actions/workflows/build.yml)
[![crates.io](https://img.shields.io/crates/v/vers-rs.svg)](https://crates.io/crates/vers-rs)
[![npm](https://img.shields.io/npm/v/%40csaf-rs%2Fvers-rs.svg)](https://www.npmjs.com/package/@csaf-rs/vers-rs)

A Rust library (with WASM support) for parsing, validating, and checking version
range specifiers.

This library implements the version range specifier (vers) format as described in
the [vers-spec](https://github.com/package-url/vers-spec).

## Usage

```rust
use vers_rs::schemes::semver::*;
use vers_rs::{parse, VersVersionRange};
use vers_rs::range::VersionRange;

// Parse a version range specifier with an explicit type
let range: VersVersionRange<SemVer> = "vers:npm/>=1.0.0|<2.0.0".parse().unwrap();

// Parse a version range specifier with dynamic dispatch
let dynamic_range = parse("vers:npm/>=1.0.0|<2.0.0").unwrap();

// Check if a version is within the range
assert!(range.contains("1.5.0".parse().unwrap()).unwrap());
assert!(!range.contains("2.0.0".parse().unwrap()).unwrap());

assert!(dynamic_range.contains("1.5.0".to_string()).unwrap());
assert!(!dynamic_range.contains("2.0.0".to_string()).unwrap());
```

### JavaScript / WASM Usage

Call `init()` once before using the library. In browsers it loads the wasm module; in Node.js it returns immediately, because the module is already loaded on import.

```js
import init, { parse, contains, parse_native } from "@csaf-rs/vers-rs";

await init();

const range = parse("vers:npm/>=1.0.0|<2.0.0");
console.log(contains(range, "1.5.0")); // true
console.log(contains(range, "2.0.0")); // false

// Parse a scheme's native range syntax directly, e.g. Debian's `<<1.0`
const deb = parse_native("deb", "<<1.0");
console.log(contains(deb, "0.9")); // true
```

## Supported types

- Semver
- NPM
- Debian
- Cargo
- Intdot
- all
- none

## Development

### Testing

Native tests run with the usual `cargo test`.

WASM-specific tests (using `#[wasm_bindgen_test]` in `src/wasm.rs`) only run
when targeting `wasm32-unknown-unknown`, and are best run with
[`wasm-pack`](https://crates.io/crates/wasm-pack), which takes care of
installing a matching `wasm-bindgen` CLI version automatically:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
wasm-pack test --node --features wasm
```

Node.js must be available on `PATH` for the `--node` runner. Use
`--headless --chrome` or `--headless --firefox` instead to run in a headless
browser.

## Features

- Parse version range specifiers in the format `vers:<type>/<constraint>|<constraint>|...`
- Validate version range specifiers according to the rules in the specification
- Check if a version is within a specified range
- Support for different types
- Dynamic dispatch wrapper that automatically detects version schemes

## TODO: Future Improvements

- **Version Comparison**: Implement proper version comparison for different types:
  - PEP440 for Python/PyPI
  - Maven versioning rules
  - RubyGems versioning rules

- **Normalization**: Improve the normalization algorithm:
  - Use proper version comparison for sorting
  - Handle more edge cases
  - Optimize for better performance

- **Validation**: Enhance validation:
  - Validate version formats for different types
  - Add more detailed error messages
  - Make sort order validation a hard requirement

- **Error Handling**: Improve error handling:
  - Add more specific error types
  - Provide more context in error messages
