//! Analysis-only facts for diagnostics, completion, and stdlib metadata.

mod completion;
mod expression;
mod facts;
mod hints;
mod registry;
mod stdlib;
mod type_fact;

pub use completion::{
    CompletionItem, CompletionKind, global_completions, local_completions, member_completions,
    type_completions,
};
pub use expression::{ExprFactScope, type_fact_from_expr};
pub use facts::AnalysisFacts;
pub use hints::{type_fact_from_hint, type_fact_from_path};
pub use registry::{RegistryFacts, RegistryFunctionFact, RegistryMemberFact};
pub use stdlib::{
    LambdaFact, StdlibFunctionFact, StdlibMethodFact, stdlib_function_completion_facts,
    stdlib_function_fact, stdlib_method_fact, stdlib_method_facts,
};
pub use type_fact::TypeFact;
