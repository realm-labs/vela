//! Analysis-only facts for diagnostics, completion, and stdlib metadata.

mod facts;
mod hints;
mod stdlib;
mod type_fact;

pub use facts::AnalysisFacts;
pub use hints::{type_fact_from_hint, type_fact_from_path};
pub use stdlib::{
    LambdaFact, StdlibFunctionFact, StdlibMethodFact, stdlib_function_fact, stdlib_method_fact,
};
pub use type_fact::TypeFact;
