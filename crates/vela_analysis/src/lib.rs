//! Analysis-only facts for diagnostics, completion, and stdlib metadata.

mod stdlib;
mod type_fact;

pub use stdlib::{
    LambdaFact, StdlibFunctionFact, StdlibMethodFact, stdlib_function_fact, stdlib_method_fact,
};
pub use type_fact::TypeFact;
