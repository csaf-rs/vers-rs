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
