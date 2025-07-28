use crate::{VersError, VersionConstraint};
use crate::constraint::VT;

pub trait VersionRange<V> {
    fn versioning_scheme(&self) -> &str;
    fn contains(&self, version: V) -> Result<bool, VersError>;
    fn constraints(&self) -> &Vec<VersionConstraint<impl VT>>;
}

pub mod generic;
pub mod dynamic;