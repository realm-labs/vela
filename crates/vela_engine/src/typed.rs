mod context_host;
mod host;
mod method;
mod native;
mod returning;
mod traits;

pub use returning::IntoNativeReturn;
pub use traits::{
    TypedContextHostNativeFunction, TypedHostNativeFunction, TypedNativeFunction,
    TypedNativeMethodFunction,
};

use vela_vm::{Value, VmError, VmErrorKind, VmResult};

pub(crate) fn expect_arity(args: &[Value], expected: usize) -> VmResult<()> {
    if args.len() == expected {
        return Ok(());
    }
    Err(VmError {
        kind: VmErrorKind::ArityMismatch {
            name: "typed native function".to_owned(),
            expected,
            actual: args.len(),
        },
        source_span: None,
        call_stack: Default::default(),
    })
}
