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
}

pub mod dynamic;
pub mod vers;
