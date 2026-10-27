use crate::range::dynamic::DynamicVersionRange;
use crate::{Comparator, VersError, VersionConstraint};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

const WILDCARD_CONSTRAINT: &str = "*";

pub(crate) struct WildcardRange;

#[derive(Deserialize)]
pub(crate) struct RawWildcardRange {
    versioning_scheme: String,
    constraints: Vec<VersionConstraint<String>>,
}

#[derive(Serialize)]
struct SerializableWildcardRange {
    versioning_scheme: &'static str,
    constraints: &'static Vec<VersionConstraint<String>>,
}

impl WildcardRange {
    pub(crate) fn parse_constraint(raw: &str, error: &str) -> Result<(), VersError> {
        if raw != WILDCARD_CONSTRAINT {
            return Err(VersError::InvalidRange(error.to_string()));
        }
        Ok(())
    }

    pub(crate) fn parse_vers(
        s: &str,
        versioning_scheme: &str,
        error: &str,
    ) -> Result<(), VersError> {
        let (scheme, constraints) = DynamicVersionRange::extract_versioning_scheme(s)?;
        if scheme != versioning_scheme || constraints != WILDCARD_CONSTRAINT {
            return Err(VersError::InvalidRange(error.to_string()));
        }
        Ok(())
    }

    pub(crate) fn validate_raw(
        raw: RawWildcardRange,
        versioning_scheme: &str,
        error: &str,
    ) -> Result<(), VersError> {
        if raw.versioning_scheme != versioning_scheme
            || raw.constraints.len() != 1
            || raw.constraints[0].comparator != Comparator::Any
            || !raw.constraints[0].version.is_empty()
        {
            return Err(VersError::InvalidRange(error.to_string()));
        }
        Ok(())
    }

    pub(crate) fn serialize<S>(
        serializer: S,
        versioning_scheme: &'static str,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        SerializableWildcardRange {
            versioning_scheme,
            constraints: Self::constraints(),
        }
        .serialize(serializer)
    }

    pub(crate) fn constraints() -> &'static Vec<VersionConstraint<String>> {
        static CONSTRAINTS: OnceLock<Vec<VersionConstraint<String>>> = OnceLock::new();
        CONSTRAINTS.get_or_init(|| vec![VersionConstraint::new(Comparator::Any, String::new())])
    }
}
