mod context_host;
mod native;
mod returning;
mod traits;

pub use returning::IntoNativeReturn;
pub use traits::{TypedContextHostNativeFunction, TypedNativeFunction};

use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

pub(crate) fn expect_arity(args: &[OwnedValue], expected: usize) -> VmResult<()> {
    if args.len() == expected {
        return Ok(());
    }
    Err(VmError::new(VmErrorKind::ArityMismatch {
        name: "typed native function".to_owned(),
        expected,
        actual: args.len(),
    }))
}
