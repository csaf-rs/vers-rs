//! Generates Rust test cases from the `vers-spec` test suite.
//!
//! Every JSON file in `vers-spec/tests` becomes a module in
//! `$OUT_DIR/vers_spec_tests.rs`, which `tests/vers_spec/main.rs` includes.
//! Each test case becomes one `#[test]` function that calls a helper from
//! `tests/vers_spec/support.rs`.
//!
//! Cases that need a versioning scheme vers-rs does not implement are still
//! generated, but marked `#[ignore]`, so they show up in the test report and
//! start running as soon as the scheme is added to the lists below.
//!
//! Cases that vers-rs is known to fail are listed in
//! `tests/vers_spec/known_failures.txt` and are also marked `#[ignore]`.
//!
//! When the submodule is not checked out (e.g. when building from a published
//! crate) nothing is generated.

use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;
use std::{env, fs};

const SUITE_DIR: &str = "vers-spec/tests";
const KNOWN_FAILURES: &str = "tests/vers_spec/known_failures.txt";

/// Known failing cases, keyed by (test file name, case index) with the reason.
type KnownFailures = HashMap<(String, usize), String>;

/// Schemes that can be parsed as a `vers:<scheme>/...` string.
const RANGE_SCHEMES: &[&str] = &["semver", "npm", "deb", "cargo", "intdot", "all", "none"];

/// Schemes whose bare versions can be compared.
const COMPARABLE_SCHEMES: &[&str] = &["semver", "npm", "deb", "cargo", "intdot"];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={SUITE_DIR}");

    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("vers_spec_tests.rs");
    let mut code = String::new();
    let known_failures = read_known_failures();

    if let Ok(entries) = fs::read_dir(SUITE_DIR) {
        let mut files: Vec<_> = entries
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect();
        files.sort();
        for file in files {
            println!("cargo:rerun-if-changed={}", file.display());
            generate_module(&file, &known_failures, &mut code);
        }
    }

    fs::write(out, code).unwrap();
}

fn read_known_failures() -> KnownFailures {
    println!("cargo:rerun-if-changed={KNOWN_FAILURES}");
    let Ok(text) = fs::read_to_string(KNOWN_FAILURES) else {
        return KnownFailures::new();
    };
    text.lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .map(|line| {
            let mut parts = line.splitn(3, ' ');
            let file = parts.next().unwrap().to_string();
            let index = parts
                .next()
                .and_then(|i| i.parse().ok())
                .unwrap_or_else(|| panic!("{KNOWN_FAILURES}: bad case index in `{line}`"));
            ((file, index), parts.next().unwrap_or("").to_string())
        })
        .collect()
}

fn generate_module(file: &Path, known_failures: &KnownFailures, code: &mut String) {
    let name = file.file_stem().unwrap().to_str().unwrap();
    let suite: Value = serde_json::from_str(&fs::read_to_string(file).unwrap())
        .unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    let tests = suite["tests"].as_array().unwrap();

    writeln!(code, "/// Generated from `{SUITE_DIR}/{name}.json`.").unwrap();
    writeln!(code, "mod {} {{", identifier(name)).unwrap();
    writeln!(code, "    use crate::support;").unwrap();
    for (index, case) in tests.iter().enumerate() {
        let known_failure = known_failures.get(&(name.to_string(), index));
        generate_case(index, case, known_failure, code);
    }
    writeln!(code, "}}\n").unwrap();
}

fn generate_case(index: usize, case: &Value, known_failure: Option<&String>, code: &mut String) {
    let description = case["description"].as_str().unwrap();
    let test_type = case["test_type"].as_str().unwrap();
    let group = case["test_group"].as_str().unwrap();
    let fails = case["expected_failure"].as_bool().unwrap_or(false);
    let input = &case["input"];
    let expected = &case["expected_output"];

    let (scheme, body) = match test_type {
        "parse" => {
            let literal = lit(input);
            let body = if fails {
                format!("support::parse_err({literal});")
            } else {
                let scheme = lit(&expected["type"]);
                let constraints: Vec<String> = expected["constraints"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|c| format!("({}, {})", lit(&c[0]), lit(&c[1])))
                    .collect();
                format!(
                    "support::parse_ok({literal}, {scheme}, &[{}]);",
                    constraints.join(", ")
                )
            };
            (vers_scheme(input.as_str().unwrap()), body)
        }
        "validate" => (
            vers_scheme(input.as_str().unwrap()),
            format!("support::validate({}, {});", lit(input), lit(expected)),
        ),
        "from_native" => {
            let scheme = input["scheme"].as_str().unwrap();
            let native = lit(&input["native_range"]);
            let body = if fails {
                format!(
                    "support::from_native_err({}, {native});",
                    lit(&input["scheme"])
                )
            } else {
                format!(
                    "support::from_native_ok({}, {native}, {});",
                    lit(&input["scheme"]),
                    lit(expected)
                )
            };
            (Support::range(scheme), body)
        }
        "containment" => (
            vers_scheme(input["vers"].as_str().unwrap()),
            format!(
                "support::containment({}, {}, {});",
                lit(&input["vers"]),
                lit(&input["version"]),
                expected.as_bool().unwrap()
            ),
        ),
        "comparison" => {
            let scheme = comparison_scheme(input);
            (
                Support::comparable(scheme),
                format!(
                    "support::sorted({}, &{}, &{});",
                    lit(&Value::from(scheme)),
                    list(&input["versions"]),
                    list(expected)
                ),
            )
        }
        "equality" => {
            let scheme = comparison_scheme(input);
            let versions = input["versions"].as_array().unwrap();
            (
                Support::comparable(scheme),
                format!(
                    "support::equal({}, {}, {}, {});",
                    lit(&Value::from(scheme)),
                    lit(&versions[0]),
                    lit(&versions[1]),
                    expected.as_bool().unwrap()
                ),
            )
        }
        other => panic!("unsupported test type `{other}` in test case `{description}`"),
    };

    writeln!(code, "\n    /// {}", doc(description)).unwrap();
    writeln!(code, "    /// Test group: {group}.").unwrap();
    writeln!(code, "    #[test]").unwrap();
    if let Support::Missing(scheme) = &scheme {
        writeln!(
            code,
            "    #[ignore = {:?}]",
            format!("versioning scheme `{scheme}` is not implemented")
        )
        .unwrap();
    } else if let Some(reason) = known_failure {
        writeln!(
            code,
            "    #[ignore = {:?}]",
            format!("known failure: {reason}")
        )
        .unwrap();
    }
    writeln!(
        code,
        "    fn case_{index:04}_{}() {{\n        {body}\n    }}",
        identifier(description).chars().take(60).collect::<String>()
    )
    .unwrap();
}

enum Support {
    Yes,
    Missing(String),
}

impl Support {
    fn range(scheme: &str) -> Self {
        Self::check(scheme, RANGE_SCHEMES)
    }

    fn comparable(scheme: &str) -> Self {
        Self::check(scheme, COMPARABLE_SCHEMES)
    }

    fn check(scheme: &str, supported: &[&str]) -> Self {
        if supported.contains(&scheme) {
            Self::Yes
        } else {
            Self::Missing(scheme.to_string())
        }
    }
}

fn vers_scheme(vers: &str) -> Support {
    let scheme = vers
        .strip_prefix("vers:")
        .and_then(|rest| rest.split('/').next())
        .unwrap_or("");
    Support::range(scheme)
}

/// The scheme key is `input_scheme` in most files and `input_type` in some.
fn comparison_scheme(input: &Value) -> &str {
    input["input_scheme"]
        .as_str()
        .or_else(|| input["input_type"].as_str())
        .expect("comparison input has neither `input_scheme` nor `input_type`")
}

/// A Rust string literal for a JSON string.
fn lit(value: &Value) -> String {
    format!("{:?}", value.as_str().expect("expected a JSON string"))
}

/// A Rust array literal for a JSON array of strings.
fn list(value: &Value) -> String {
    let items: Vec<String> = value.as_array().unwrap().iter().map(lit).collect();
    format!("[{}]", items.join(", "))
}

/// A lowercase Rust identifier made of the alphanumeric parts of `text`.
fn identifier(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

/// Makes free text safe to put in a doc comment.
fn doc(text: &str) -> String {
    text.replace(['\n', '\r'], " ")
}
