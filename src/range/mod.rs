use crate::constraint::VersionType;
use crate::{VersError, VersionConstraint};

pub trait VersionRange<V> {
    fn versioning_scheme(&self) -> &str;
    fn contains(&self, version: V) -> Result<bool, VersError>;
    fn constraints(&self) -> &Vec<VersionConstraint<impl VersionType>>;
}

pub mod dynamic;
pub mod generic;
