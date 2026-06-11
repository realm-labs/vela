//! Runtime binding keys for standard-library implementations.
//!
//! This crate intentionally maps manifest-derived semantic IDs to implementation
//! keys without making `vela_stdlib` depend on VM function pointers.

use vela_def::{FunctionId, MethodId};
use vela_stdlib::{STD_FUNCTIONS, STD_METHODS};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StdFunctionImplementation {
    MathMax,
    MathMin,
    MathClamp,
    MathLerp,
    MathMoveTowards,
    MathDistance2d,
    MathDistance3d,
    MathPow,
    MathSqrt,
    MathSign,
    MathFloor,
    MathCeil,
    MathRound,
    MathAbs,
    OptionSome,
    OptionNone,
    OptionIsSome,
    OptionIsNone,
    OptionUnwrapOr,
    OptionOkOr,
    OptionFlatten,
    ResultOk,
    ResultErr,
    ResultIsOk,
    ResultIsErr,
    ResultUnwrapOr,
    ResultToOption,
    ResultToErrorOption,
    ResultFlatten,
    SetFromArray,
    BytesFromHex,
}

impl StdFunctionImplementation {
    #[must_use]
    pub const fn from_manifest_path(module: &str, name: &str) -> Option<Self> {
        match (module.as_bytes(), name.as_bytes()) {
            (b"math", b"max") => Some(Self::MathMax),
            (b"math", b"min") => Some(Self::MathMin),
            (b"math", b"clamp") => Some(Self::MathClamp),
            (b"math", b"lerp") => Some(Self::MathLerp),
            (b"math", b"move_towards") => Some(Self::MathMoveTowards),
            (b"math", b"distance2d") => Some(Self::MathDistance2d),
            (b"math", b"distance3d") => Some(Self::MathDistance3d),
            (b"math", b"pow") => Some(Self::MathPow),
            (b"math", b"sqrt") => Some(Self::MathSqrt),
            (b"math", b"sign") => Some(Self::MathSign),
            (b"math", b"floor") => Some(Self::MathFloor),
            (b"math", b"ceil") => Some(Self::MathCeil),
            (b"math", b"round") => Some(Self::MathRound),
            (b"math", b"abs") => Some(Self::MathAbs),
            (b"option", b"some") => Some(Self::OptionSome),
            (b"option", b"none") => Some(Self::OptionNone),
            (b"option", b"is_some") => Some(Self::OptionIsSome),
            (b"option", b"is_none") => Some(Self::OptionIsNone),
            (b"option", b"unwrap_or") => Some(Self::OptionUnwrapOr),
            (b"option", b"ok_or") => Some(Self::OptionOkOr),
            (b"option", b"flatten") => Some(Self::OptionFlatten),
            (b"result", b"ok") => Some(Self::ResultOk),
            (b"result", b"err") => Some(Self::ResultErr),
            (b"result", b"is_ok") => Some(Self::ResultIsOk),
            (b"result", b"is_err") => Some(Self::ResultIsErr),
            (b"result", b"unwrap_or") => Some(Self::ResultUnwrapOr),
            (b"result", b"to_option") => Some(Self::ResultToOption),
            (b"result", b"to_error_option") => Some(Self::ResultToErrorOption),
            (b"result", b"flatten") => Some(Self::ResultFlatten),
            (b"set", b"from_array") => Some(Self::SetFromArray),
            (b"bytes", b"from_hex") => Some(Self::BytesFromHex),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StdFunctionRuntimeBinding {
    pub id: FunctionId,
    pub debug_name: String,
    pub implementation: StdFunctionImplementation,
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
        .map(|spec| {
            let Some(implementation) =
                StdFunctionImplementation::from_manifest_path(spec.module, spec.name)
            else {
                panic!(
                    "missing standard runtime implementation for {}::{}",
                    spec.module, spec.name
                );
            };
            StdFunctionRuntimeBinding {
                id: spec.id(),
                debug_name: format!("{}::{}", spec.module, spec.name),
                implementation,
            }
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
        assert_eq!(bindings[0].debug_name, "math::max");
        assert_eq!(
            bindings[0].implementation,
            StdFunctionImplementation::MathMax
        );
    }

    #[test]
    fn every_manifest_function_has_a_typed_runtime_implementation() {
        let bindings = stdlib_function_runtime_bindings();

        for spec in STD_FUNCTIONS {
            let binding = bindings
                .iter()
                .find(|binding| binding.id == spec.id())
                .expect("manifest function should have a runtime binding");

            assert_eq!(
                binding.debug_name,
                format!("{}::{}", spec.module, spec.name)
            );
            assert_eq!(
                Some(binding.implementation),
                StdFunctionImplementation::from_manifest_path(spec.module, spec.name)
            );
        }
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
