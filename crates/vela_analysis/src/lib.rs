//! Analysis-only facts for diagnostics, completion, and stdlib metadata.

mod completion;
mod diagnostics;
mod expression;
mod facts;
mod hints;
mod hover;
mod registry;
mod stdlib;
mod type_fact;

pub use completion::{
    CompletionItem, CompletionKind, declaration_completions, global_completions, local_completions,
    member_completions, module_completions, type_completions,
};
pub use diagnostics::{
    match_exhaustiveness_diagnostics, match_pattern_diagnostics, member_access_diagnostics,
};
pub use expression::{ExprFactScope, type_fact_from_expr, type_fact_from_expr_with_registry};
pub use facts::AnalysisFacts;
pub use hints::{type_fact_from_hint, type_fact_from_path};
pub use hover::{
    HoverInfo, HoverKind, field_hover, function_hover, method_hover, module_hover, trait_hover,
    trait_method_hover, type_hover, variant_hover,
};
pub use registry::{RegistryFacts, RegistryFunctionFact, RegistryMemberFact};
pub use stdlib::{
    LambdaFact, StdlibFunctionFact, StdlibMethodFact, stdlib_function_completion_facts,
    stdlib_function_fact, stdlib_method_fact, stdlib_method_facts,
};
pub use type_fact::TypeFact;
