use crate::VersError;
use crate::VersionConstraint;
use crate::comparator::Comparator;
use crate::constraint::NativeVersionConverter;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::str::FromStr;

/// Scheme identifier string for Debian versions
pub static DEB_SCHEME: &str = "deb";

/// Macro to create InvalidVersionFormat errors for Debian versions
macro_rules! deb_format_error {
    ($s:expr, $msg:expr) => {
        VersError::InvalidVersionFormat(DEB_SCHEME, $s.to_string(), $msg.into())
    };
}

/// Debian version according to dpkg version format: [epoch:]upstream[-debian]
///
/// This implementation follows Debian Policy version comparison rules:
/// - Epoch numeric (default 0)
/// - Upstream version vs. Debian revision separated by last '-'
/// - Tilde '~' sorts before the end and before any other character
/// - Sequences of digits are compared numerically; non-digits lexicographically
#[derive(Clone, Debug, Eq, Serialize, Deserialize)]
pub struct DebVersion {
    epoch: u64,
    upstream: String,
    debian_revision: String,
}

impl Default for DebVersion {
    fn default() -> Self {
        DebVersion {
            epoch: 0,
            upstream: "0".to_string(),
            debian_revision: String::new(),
        }
    }
}

impl NativeVersionConverter for DebVersion {
    const SCHEME_NAME: &'static str = "deb";

    /// Parse a Debian native constraint string into standard vers constraints.
    ///
    /// Debian defines the following comparison operators:
    /// - `<<` (strictly less than) → vers `<`
    /// - `<=` (less than or equal) → vers `<=`
    /// - `=` (exactly equal) → vers `=`
    /// - `>=` (greater than or equal) → vers `>=`
    /// - `>>` (strictly greater than) → vers `>`
    ///
    /// Single `<`, `>`, and `!=` are **not** valid Debian comparators and will
    /// be rejected. Invalid combinations like `>>=` or `<<=` are also rejected
    /// because the remaining `=` cannot start a Debian version string.
    fn from_native_constraint(raw: &str) -> Result<VersionConstraint<Self>, VersError> {
        let raw = raw.trim();

        if raw.is_empty() {
            return Err(VersError::InvalidConstraint("Empty constraint".to_string()));
        }

        // Parse and validate the Debian comparator prefix.
        // Valid Debian comparators: <<, <=, =, >=, >>
        // Single <, >, and != are NOT valid Debian comparators.
        let (comparator, version_str) = if let Some(stripped) = raw.strip_prefix("<<") {
            (Comparator::LessThan, stripped)
        } else if let Some(stripped) = raw.strip_prefix("<=") {
            (Comparator::LessThanOrEqual, stripped)
        } else if let Some(stripped) = raw.strip_prefix(">>") {
            (Comparator::GreaterThan, stripped)
        } else if let Some(stripped) = raw.strip_prefix(">=") {
            (Comparator::GreaterThanOrEqual, stripped)
        } else if let Some(stripped) = raw.strip_prefix('=') {
            (Comparator::Equal, stripped)
        } else {
            return Err(VersError::InvalidConstraint(format!(
                "invalid Debian comparator in '{}': valid comparators are <<, <=, =, >=, >>",
                raw
            )));
        };

        let version_str = version_str.trim();
        if version_str.is_empty() {
            return Err(VersError::InvalidConstraint("Missing version".to_string()));
        }

        let parsed_version = version_str.parse::<Self>().map_err(|_| {
            VersError::InvalidConstraint(format!("Failed to parse version: {}", version_str))
        })?;

        Ok(VersionConstraint::new(comparator, parsed_version))
    }
}

impl std::fmt::Display for DebVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.epoch > 0 {
            write!(f, "{}:", self.epoch)?;
        }
        write!(f, "{}", self.upstream)?;
        if !self.debian_revision.is_empty() {
            write!(f, "-{}", self.debian_revision)?;
        }
        Ok(())
    }
}

impl FromStr for DebVersion {
    type Err = VersError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(deb_format_error!(s, "empty"));
        }

        // Parse epoch
        let (epoch, rest) = if let Some(colon) = s.find(':') {
            let ep_str = &s[..colon];
            if ep_str.is_empty() {
                return Err(deb_format_error!(s, "missing epoch"));
            }
            let epoch = ep_str
                .parse::<u64>()
                .map_err(|e| deb_format_error!(s, format!("invalid epoch: {e}")))?;
            (epoch, &s[colon + 1..])
        } else {
            (0, s)
        };

        if rest.ends_with('-') {
            return Err(deb_format_error!(
                s,
                "trailing '-' with empty debian_revision"
            ));
        }

        // Split upstream and debian revision at last '-'
        let (upstream, debian_revision) = rest
            .rfind('-')
            .map(|idx| {
                let (u, d) = rest.split_at(idx);
                (u, &d[1..])
            })
            .unwrap_or((rest, ""));

        // Validate upstream
        if upstream.is_empty() {
            return Err(deb_format_error!(s, "missing upstream_version"));
        }

        if !upstream.chars().next().unwrap().is_ascii_digit() {
            return Err(deb_format_error!(
                s,
                "upstream_version must start with a digit"
            ));
        }

        for ch in upstream.chars() {
            if !ch.is_ascii_alphanumeric() && !matches!(ch, '.' | '+' | '-' | '~') {
                return Err(deb_format_error!(
                    s,
                    format!("invalid character '{ch}' in upstream_version")
                ));
            }
        }

        // Validate debian_revision when present
        if !debian_revision.is_empty() {
            for ch in debian_revision.chars() {
                if !ch.is_ascii_alphanumeric() && !matches!(ch, '+' | '.' | '~') {
                    return Err(deb_format_error!(
                        s,
                        format!("invalid character '{ch}' in debian_revision")
                    ));
                }
            }
        }

        Ok(DebVersion {
            epoch,
            upstream: upstream.to_string(),
            debian_revision: debian_revision.to_string(),
        })
    }
}

impl Ord for DebVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        // Compare epochs first
        match self.epoch.cmp(&other.epoch) {
            Ordering::Equal => {}
            ord => return ord,
        }

        // Compare upstream versions
        match compare_part(&self.upstream, &other.upstream) {
            Ordering::Equal => {}
            ord => return ord,
        }

        // Compare debian revisions (empty means "0")
        let rev_a = if self.debian_revision.is_empty() {
            "0"
        } else {
            &self.debian_revision
        };
        let rev_b = if other.debian_revision.is_empty() {
            "0"
        } else {
            &other.debian_revision
        };
        compare_part(rev_a, rev_b)
    }
}

impl PartialOrd for DebVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// Implement PartialEq based on Ord, so that equality is consistent with
// ordering. This is necessary because `Ord::cmp` treats an empty debian_revision
// as "0", which would diverge from a field-by-field derived equality.
impl PartialEq for DebVersion {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

/// Compare two version part strings according to Debian's dpkg algorithm.
/// Alternates between comparing non-digit and digit sequences.
fn compare_part(a: &str, b: &str) -> Ordering {
    let mut a = a;
    let mut b = b;

    loop {
        // Compare non-digit sequence
        let ord = compare_non_digit_sequence(&mut a, &mut b);
        if ord != Ordering::Equal {
            return ord;
        }

        // Compare digit sequence
        let ord = compare_digit_sequence(&mut a, &mut b);
        if ord != Ordering::Equal {
            return ord;
        }

        // If both exhausted, they're equal
        if a.is_empty() && b.is_empty() {
            return Ordering::Equal;
        }
    }
}

/// Compare non-digit character sequences.
/// Returns when both reach a digit or the end of the string.
/// Implements Debian's special ordering: '~' < None < other chars
fn compare_non_digit_sequence(a: &mut &str, b: &mut &str) -> Ordering {
    loop {
        let ca = a.chars().next().filter(|c| !c.is_ascii_digit());
        let cb = b.chars().next().filter(|c| !c.is_ascii_digit());

        match (ca, cb) {
            (None, None) => return Ordering::Equal,
            (Some('~'), Some('~')) => {}
            (Some('~'), _) => return Ordering::Less,
            (_, Some('~')) => return Ordering::Greater,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(c1), Some(c2)) => match c1.cmp(&c2) {
                Ordering::Equal => {}
                ord => return ord,
            },
        }

        // Advance both slices by one character
        *a = &a[ca.unwrap().len_utf8()..];
        *b = &b[cb.unwrap().len_utf8()..];
    }
}

/// Compare digit sequences numerically by length first, then lexicographically.
fn compare_digit_sequence(a: &mut &str, b: &mut &str) -> Ordering {
    // Skip leading zeros
    *a = a.trim_start_matches('0');
    *b = b.trim_start_matches('0');

    // Collect digit runs
    let a_digits: String = a.chars().take_while(|c| c.is_ascii_digit()).collect();
    let b_digits: String = b.chars().take_while(|c| c.is_ascii_digit()).collect();

    // Advance slices past the digits we collected
    *a = &a[a_digits.len()..];
    *b = &b[b_digits.len()..];

    // Compare by length first, then lexicographically
    match a_digits.len().cmp(&b_digits.len()) {
        Ordering::Equal => a_digits.cmp(&b_digits),
        ord => ord,
    }
}

#[cfg(test)]
mod tests {
    use crate::Comparator;
    use crate::VersError;
    use crate::range::VersionRange;
    use crate::range::dynamic::DynamicVersionRange;

    #[test]
    fn test_dynamic_parse_deb() {
        let range: DynamicVersionRange = "vers:deb/<<1.0".parse().unwrap();
        assert_eq!(range.versioning_scheme(), "deb");
        assert_eq!(range.constraints().len(), 1);
        assert_eq!(range.constraints()[0].comparator, Comparator::LessThan);
        assert_eq!(range.constraints()[0].version.to_string(), "1.0");
    }

    #[test]
    fn test_deb_version_ordering_basic() {
        let range: DynamicVersionRange = "vers:deb/<<1.0".parse().unwrap();
        assert!(range.contains("0.9".to_string()).unwrap());
        assert!(!range.contains("1.0".to_string()).unwrap());
    }

    #[test]
    fn test_deb_version_ordering_tilde_and_epoch() {
        // 1.0~beta < 1.0
        let range1: DynamicVersionRange = "vers:deb/<<1.0".parse().unwrap();
        assert!(range1.contains("1.0~beta".to_string()).unwrap());

        let range2: DynamicVersionRange = "vers:deb/>>2.0".parse().unwrap();
        // 1:1.0 > 2.0 because epoch 1 > 0
        assert!(range2.contains("1:1.0".to_string()).unwrap());
        assert!(!range2.contains("2.0".to_string()).unwrap());
    }

    #[test]
    fn test_deb_valid_comparators() {
        // << maps to LessThan
        let range: DynamicVersionRange = "vers:deb/<<1.0".parse().unwrap();
        assert_eq!(range.constraints()[0].comparator, Comparator::LessThan);

        // <= maps to LessThanOrEqual
        let range: DynamicVersionRange = "vers:deb/<=1.0".parse().unwrap();
        assert_eq!(
            range.constraints()[0].comparator,
            Comparator::LessThanOrEqual
        );

        // = maps to Equal
        let range: DynamicVersionRange = "vers:deb/=1.0".parse().unwrap();
        assert_eq!(range.constraints()[0].comparator, Comparator::Equal);

        // >= maps to GreaterThanOrEqual
        let range: DynamicVersionRange = "vers:deb/>=1.0".parse().unwrap();
        assert_eq!(
            range.constraints()[0].comparator,
            Comparator::GreaterThanOrEqual
        );

        // >> maps to GreaterThan
        let range: DynamicVersionRange = "vers:deb/>>1.0".parse().unwrap();
        assert_eq!(range.constraints()[0].comparator, Comparator::GreaterThan);
    }

    #[test]
    fn test_deb_invalid_comparators_rejected() {
        // Single < is not a valid Debian comparator
        let result: Result<DynamicVersionRange, VersError> = "vers:deb/<1.0".parse();
        assert!(result.is_err());

        // Single > is not a valid Debian comparator
        let result: Result<DynamicVersionRange, VersError> = "vers:deb/>1.0".parse();
        assert!(result.is_err());

        // != is not a valid Debian comparator
        let result: Result<DynamicVersionRange, VersError> = "vers:deb/!=1.0".parse();
        assert!(result.is_err());

        // >>= is not a valid Debian comparator (>> with version "=1.0" fails)
        let result: Result<DynamicVersionRange, VersError> = "vers:deb/>>=1.0".parse();
        assert!(result.is_err());

        // <<= is not a valid Debian comparator (<< with version "=1.0" fails)
        let result: Result<DynamicVersionRange, VersError> = "vers:deb/<<=1.0".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_deb_equality_consistent_with_ordering() {
        use super::DebVersion;

        // A version with empty debian_revision should equal one with "0"
        let a = DebVersion {
            epoch: 0,
            upstream: "1.0".to_string(),
            debian_revision: String::new(),
        };
        let b = DebVersion {
            epoch: 0,
            upstream: "1.0".to_string(),
            debian_revision: "0".to_string(),
        };

        // Equality must be consistent with Ord::cmp
        assert_eq!(a, b);
        assert!(!(a < b));
        assert!(!(a > b));
    }

    #[test]
    fn test_deb_parse_native_preserves_scheme() {
        let range = DynamicVersionRange::parse_native("deb", "<<1.0").unwrap();
        assert_eq!(range.versioning_scheme(), "deb");
    }

    #[test]
    fn test_deb_parse_native_normalizes() {
        // parse_native should normalize: >1.0|>2.0 simplifies to >1.0
        let range = DynamicVersionRange::parse_native("deb", ">>1.0|>>2.0").unwrap();
        assert_eq!(range.constraints().len(), 1);
        assert_eq!(range.constraints()[0].comparator, Comparator::GreaterThan);
        assert_eq!(range.constraints()[0].version.to_string(), "1.0");
    }
}
