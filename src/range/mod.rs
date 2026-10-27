use crate::constraint::VersionType;
use crate::{Comparator, VersError, VersionConstraint};
pub trait VersionRange<V: VersionType> {
    fn versioning_scheme(&self) -> &str;
    fn contains(&self, version: V) -> Result<bool, VersError>;
    fn constraints(&self) -> &Vec<VersionConstraint<V>>;

    /// Determine whether this range consists of exactly one version.
    ///
    /// A range is considered a single version when it has exactly one
    /// constraint using the implicit equality comparator (e.g. `vers:npm/1.2.3`).
    /// Ranges using `*`, `!=`, or any comparison operator (even a single one,
    /// e.g. `vers:npm/>=1.0.0`) are not single versions, since they match more
    /// than one version.
    fn is_single_version(&self) -> bool {
        matches!(self.constraints().as_slice(), [c] if c.comparator == Comparator::Equal)
    }

    /// Retrieve the single version of this range, if there is one.
    ///
    /// Returns `Some` with the version when [`Self::is_single_version`] holds,
    /// i.e. the range has exactly one constraint using the implicit equality
    /// comparator (e.g. `vers:npm/1.2.3`). Returns `None` otherwise.
    fn single_version(&self) -> Option<&V> {
        match self.constraints().as_slice() {
            [c] if c.comparator == Comparator::Equal => Some(&c.version),
            _ => None,
        }
    }

    /// Check whether a valid (normalized) version range is open-ended, i.e. unbounded above.
    ///
    /// A range is open-ended when its highest bound among the range-style
    /// constraints (`<`, `<=`, `>`, `>=`) uses `>` or `>=`, meaning every version
    /// greater than that bound is included and there is no upper limit.
    ///
    /// This assumes the range has already been normalized (e.g. via
    /// `normalize_and_validate` or successful parsing), so `constraints()` is
    /// sorted in ascending version order.
    ///
    /// # Examples
    ///
    /// ```
    /// use vers_rs::{VersVersionRange};
    /// use vers_rs::range::VersionRange;
    /// use vers_rs::schemes::semver::SemVer;
    ///
    /// let open: VersVersionRange<SemVer> = "vers:npm/>=1.0.0".parse().unwrap();
    /// assert!(open.is_open_ended());
    ///
    /// let closed: VersVersionRange<SemVer> = "vers:npm/>=1.0.0|<2.0.0".parse().unwrap();
    /// assert!(!closed.is_open_ended());
    /// ```
    fn is_open_ended(&self) -> bool {
        self.constraints()
            .iter()
            .rfind(|c| {
                matches!(
                    c.comparator,
                    Comparator::LessThan
                        | Comparator::LessThanOrEqual
                        | Comparator::GreaterThan
                        | Comparator::GreaterThanOrEqual
                )
            })
            .is_some_and(|c| {
                matches!(
                    c.comparator,
                    Comparator::GreaterThan | Comparator::GreaterThanOrEqual
                )
            })
    }
}

pub mod dynamic;
pub mod vers;
pub(crate) mod wildcard;
