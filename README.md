# vers-rs

A Rust library for parsing, validating, and checking version range specifiers.

This library implements the version range specifier (vers) format as described in
the [vers-spec](https://github.com/package-url/vers-spec).

## Usage

```rust
use vers_rs::schemes::semver::*;
use vers_rs::{parse, contains, GenericVersionRange};
use vers_rs::range::VersionRange;

// Parse a version range specifier with an explicit type
let range: GenericVersionRange<SemVer> = "vers:npm/>=1.0.0|<2.0.0".parse().unwrap();

// Parse a version range specifier with dynamic dispatch
let dynamic_range = parse("vers:npm/>=1.0.0|<2.0.0").unwrap();

// Check if a version is within the range
assert!(range.contains(&"1.5.0".parse().unwrap()).unwrap());
assert!(!range.contains(&"2.0.0".parse().unwrap()).unwrap());

assert!(dynamic_range.contains("1.5.0").unwrap());
assert!(!dynamic_range.contains("2.0.0").unwrap());
```

## Features

- Parse version range specifiers in the format `vers:<versioning-scheme>/<version-constraint>|<version-constraint>|...`
- Validate version range specifiers according to the rules in the specification
- Normalize and simplify version range specifiers
- Check if a version is within a specified range
- Support for different versioning schemes (npm/semver, pypi, maven, deb, etc.)
- Dynamic dispatch wrapper that automatically detects version schemes

## TODO: Future Improvements

- **Version Comparison**: Implement proper version comparison for different versioning schemes:
  - PEP440 for Python/PyPI
  - Maven versioning rules
  - Debian versioning rules
  - RubyGems versioning rules

- **Normalization**: Improve the normalization algorithm:
  - Use proper version comparison for sorting
  - Handle more edge cases
  - Optimize for better performance

- **Validation**: Enhance validation:
  - Validate version formats for different versioning schemes
  - Add more detailed error messages
  - Make sort order validation a hard requirement

- **Error Handling**: Improve error handling:
  - Add more specific error types
  - Provide more context in error messages
  - Consider returning errors for unknown versioning schemes
