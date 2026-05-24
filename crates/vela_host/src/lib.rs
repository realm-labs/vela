//! Host reference, path, and patch transaction model.

mod adapter;
mod error;
mod mock;
mod patch;
mod path;
mod tx;
mod value;

pub use adapter::ScriptStateAdapter;
pub use error::{HostError, HostErrorKind, HostResult};
pub use mock::MockStateAdapter;
pub use patch::{Patch, PatchOp};
pub use path::{HostPath, HostRef, PathSegment};
pub use tx::{HostObjectSnapshot, PatchTx};
pub use value::HostValue;

pub(crate) use value::add_values;

#[cfg(test)]
mod tests;
