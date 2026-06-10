//! Runtime binding keys for standard-library implementations.
//!
//! This crate intentionally maps manifest-derived semantic IDs to implementation
//! keys without making `vela_stdlib` depend on VM function pointers.

use vela_def::{FunctionId, MethodId};
use vela_stdlib::{STD_FUNCTIONS, STD_METHODS};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StdFunctionRuntimeBinding {
    pub id: FunctionId,
    pub implementation: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StdMethodRuntimeBinding {
    pub id: MethodId,
    pub implementation: String,
}

#[must_use]
pub fn stdlib_function_runtime_bindings() -> Vec<StdFunctionRuntimeBinding> {
    STD_FUNCTIONS
        .iter()
        .map(|spec| StdFunctionRuntimeBinding {
            id: spec.id(),
            implementation: format!("{}::{}", spec.module, spec.name),
        })
        .collect()
}

#[must_use]
pub fn stdlib_method_runtime_bindings() -> Vec<StdMethodRuntimeBinding> {
    STD_METHODS
        .iter()
        .map(|spec| StdMethodRuntimeBinding {
            id: spec.id(),
            implementation: format!("{}::{}", spec.owner, spec.name),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use vela_stdlib::{STD_FUNCTIONS, STD_METHODS};

    #[test]
    fn function_bindings_are_keyed_by_manifest_ids() {
        let bindings = stdlib_function_runtime_bindings();

        assert_eq!(bindings.len(), STD_FUNCTIONS.len());
        assert_eq!(bindings[0].id, STD_FUNCTIONS[0].id());
        assert_eq!(bindings[0].implementation, "math::max");
    }

    #[test]
    fn method_bindings_are_keyed_by_manifest_ids() {
        let bindings = stdlib_method_runtime_bindings();
        let expected = STD_METHODS
            .iter()
            .find(|spec| spec.owner == "Array" && spec.name == "sort_by")
            .expect("Array::sort_by should be declared in the manifest");
        let binding = bindings
            .iter()
            .find(|binding| binding.implementation == "Array::sort_by")
            .expect("Array::sort_by runtime binding should exist");

        assert_eq!(bindings.len(), STD_METHODS.len());
        assert_eq!(binding.id, expected.id());
    }

    #[test]
    fn runtime_binding_ids_are_unique() {
        let function_bindings = stdlib_function_runtime_bindings();
        let method_bindings = stdlib_method_runtime_bindings();
        let function_ids = function_bindings
            .iter()
            .map(|binding| binding.id.def_id())
            .collect::<BTreeSet<_>>();
        let method_ids = method_bindings
            .iter()
            .map(|binding| binding.id.def_id())
            .collect::<BTreeSet<_>>();

        assert_eq!(function_ids.len(), function_bindings.len());
        assert_eq!(method_ids.len(), method_bindings.len());
    }
}
