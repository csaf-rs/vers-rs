/// Versioning schemes defined by the VERS specification.
///
/// The variants that are not currently supported by this crate are included so
/// callers can distinguish an unknown scheme from a known but unsupported one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::AsRefStr, strum::Display, strum::EnumString)]
pub enum VersioningScheme {
    #[strum(serialize = "alpm")]
    Alpm,
    #[strum(serialize = "apk")]
    Apk,
    #[strum(serialize = "cargo")]
    Cargo,
    #[strum(serialize = "composer")]
    Composer,
    #[strum(serialize = "cpan")]
    Cpan,
    #[strum(serialize = "dart")]
    Dart,
    #[strum(serialize = "deb")]
    Deb,
    #[strum(serialize = "gem")]
    Gem,
    #[strum(serialize = "golang")]
    Golang,
    #[strum(serialize = "hackage")]
    Hackage,
    #[strum(serialize = "maven")]
    Maven,
    #[strum(serialize = "nuget")]
    Nuget,
    #[strum(serialize = "npm")]
    Npm,
    #[strum(serialize = "pypi")]
    Pypi,
    #[strum(serialize = "rpm")]
    Rpm,
    #[strum(serialize = "all")]
    All,
    #[strum(serialize = "datetime")]
    Datetime,
    #[strum(serialize = "generic")]
    Generic,
    #[strum(serialize = "intdot")]
    Intdot,
    #[strum(serialize = "lexicographic")]
    Lexicographic,
    #[strum(serialize = "none")]
    None,
    #[strum(serialize = "semver")]
    Semver,
}

impl VersioningScheme {
    /// Schemes listed in the official `vers-types-index.json`.
    pub const REGISTERED: &'static [VersioningScheme] =
        &[VersioningScheme::Npm, VersioningScheme::Pypi];

    /// Schemes with a working implementation in this crate not
    /// listed in REGISTERED.
    pub const SUPPORTED: &'static [VersioningScheme] = &[
        VersioningScheme::Semver,
        VersioningScheme::Deb,
        VersioningScheme::Npm,
        VersioningScheme::Cargo,
        VersioningScheme::Intdot,
        VersioningScheme::All,
        VersioningScheme::None,
    ];

    pub fn is_registered(self) -> bool {
        Self::REGISTERED.contains(&self)
    }

    pub fn is_supported(self) -> bool {
        Self::SUPPORTED.contains(&self)
    }
}

#[cfg(test)]
mod tests {
    use super::VersioningScheme;

    #[test]
    fn supported_schemes_are_parsed_not_unsupported() {
        // Drift guard: every SUPPORTED scheme must have a match arm in
        // DynamicVersionRange::from_str, so none of them reports
        // UnsupportedVersioningScheme.
        use crate::{DynamicVersionRange, VersError};
        for scheme in VersioningScheme::SUPPORTED {
            let input = format!("vers:{}/*", scheme.as_ref());
            let result: Result<DynamicVersionRange, _> = input.parse();
            assert!(
                !matches!(result, Err(VersError::UnsupportedVersioningScheme(_))),
                "{scheme} is in SUPPORTED but has no implementation"
            );
        }
    }

    #[test]
    fn registered_but_unsupported_scheme_is_pypi() {
        assert!(VersioningScheme::Pypi.is_registered());
        assert!(!VersioningScheme::Pypi.is_supported());
    }
}
