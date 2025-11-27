use crate::VersError;
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
