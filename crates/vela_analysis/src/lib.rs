//! Analysis-only facts for diagnostics, completion, and stdlib metadata.

mod stdlib;
mod type_fact;

pub use stdlib::{LambdaFact, StdlibMethodFact, stdlib_method_fact};
pub use type_fact::TypeFact;
