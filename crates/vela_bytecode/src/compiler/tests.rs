use super::*;
use crate::CallArgument;
use vela_common::MethodId;
fn semantic_diagnostic_codes(error: CompileError) -> Vec<String> {
    let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
        panic!("expected semantic diagnostics");
    };
    diagnostics
        .into_iter()
        .filter_map(|diagnostic| diagnostic.code)
        .collect()
}

fn stable_test_trait_method_id(trait_name: &str, method_name: &str) -> MethodId {
    MethodId::new(vela_common::stable_id(
        "trait_method",
        trait_name,
        method_name,
    ))
}

mod closures_and_bindings;
mod diagnostics;
mod expressions;
mod host_paths;
mod literals_and_calls;
mod loops_and_errors;
mod module_resolution;
mod script_methods;
