pub(in crate::compiler) mod metadata;

use vela_syntax::ast::SyntaxArgument;

use super::record_shapes::ValueShape;
use super::value_types::RuntimeTypeFact;
use super::{CompileError, CompileErrorKind, CompileResult, Compiler};
use vela_common::{Diagnostic, PrimitiveTag, Span};
use vela_def::{DefPath, FunctionId, MethodId, TypeId};
use vela_registry::ParamDef;

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn resolve_native_function_id(
        &self,
        name: &str,
        call_span: Span,
    ) -> CompileResult<FunctionId> {
        let Some(registry) = self.facts.registry else {
            return Ok(function_id_for_native_name(name));
        };
        if let Some(id) = registry.resolve_native_function_name(name) {
            return Ok(id);
        }

        Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
            vec![
                Diagnostic::error(format!("unresolved native function `{name}`"))
                    .with_code("compiler::unresolved_native_function")
                    .with_span(call_span)
                    .with_label(call_span, "native function is not registered"),
            ],
        )))
    }

    pub(in crate::compiler) fn value_method_id_for_type(
        &self,
        receiver_type: &RuntimeTypeFact,
        method: &str,
    ) -> Option<MethodId> {
        if let Some(registry) = self.facts.registry {
            let owner = self.registry_value_type_id(receiver_type)?;
            return registry.resolve_value_method(owner, method);
        }
        None
    }

    pub(in crate::compiler) fn registry_value_method_params(
        &self,
        receiver_type: Option<&RuntimeTypeFact>,
        method: &str,
    ) -> Option<&[ParamDef]> {
        let registry = self.facts.registry?;
        let owner = self.registry_value_type_id(receiver_type?)?;
        let method = registry.resolve_value_method(owner, method)?;
        registry.method_params(method)
    }

    pub(in crate::compiler) fn value_methods_known_for_type(
        &self,
        receiver_type: &RuntimeTypeFact,
    ) -> bool {
        self.registry_value_type_id(receiver_type).is_some()
    }

    fn registry_value_type_id(&self, receiver_type: &RuntimeTypeFact) -> Option<TypeId> {
        let registry = self.facts.registry?;
        if let RuntimeTypeFact::Primitive(primitive) = receiver_type
            && let Some(id) = registry.primitive_type_id(*primitive)
        {
            return Some(id);
        }
        let type_name = receiver_type.std_type_name();
        registry.resolve_type(&DefPath::ty("std", std::iter::empty::<&str>(), type_name))
    }
}

impl Compiler<'_, '_> {
    fn reject_static_ord_shape(
        &self,
        method: &str,
        shape: &ValueShape,
        span: Span,
    ) -> CompileResult<()> {
        if let Some(value_type) = shape.value_type() {
            if !runtime_type_satisfies_ord(&value_type) {
                return Err(missing_array_ord_error(
                    method,
                    "key",
                    &value_type.source_type_display(),
                    span,
                ));
            }
            return Ok(());
        }
        let Some(type_name) = shape.as_record().and_then(|record| record.type_name()) else {
            return Ok(());
        };
        if !self.is_declared_script_type(type_name)
            || self.type_implements_builtin_trait_method(type_name, "Ord", "cmp")
        {
            return Ok(());
        }
        Err(missing_array_ord_error(method, "key", type_name, span))
    }

    pub(in crate::compiler) fn reject_static_syntax_array_ordering_method_without_ord(
        &self,
        source: vela_common::SourceId,
        method: &str,
        args: &[SyntaxArgument],
        receiver_type: Option<&RuntimeTypeFact>,
        receiver_shape: Option<&ValueShape>,
        span: Span,
    ) -> CompileResult<()> {
        if !matches!(method, "sort" | "sort_by" | "min" | "max") {
            return Ok(());
        }
        if method == "sort_by" {
            let Some(receiver_shape) = receiver_shape else {
                return Ok(());
            };
            let Some(key_shape) =
                self.syntax_callback_return_shape(receiver_shape, method, args, Some(source))
            else {
                return Ok(());
            };
            return self.reject_static_ord_shape(method, &key_shape, span);
        }
        if let Some(RuntimeTypeFact::Array(element)) = receiver_type
            && !runtime_type_satisfies_ord(element)
        {
            return Err(missing_array_ord_error(
                method,
                "element",
                &element.source_type_display(),
                span,
            ));
        }
        let Some(ValueShape::Array(element)) = receiver_shape else {
            return Ok(());
        };
        let Some(type_name) = element.as_record().and_then(|record| record.type_name()) else {
            return Ok(());
        };
        if !self.is_declared_script_type(type_name)
            || self.type_implements_builtin_trait_method(type_name, "Ord", "cmp")
        {
            return Ok(());
        }
        Err(missing_array_ord_error(method, "element", type_name, span))
    }
}

fn runtime_type_satisfies_ord(fact: &RuntimeTypeFact) -> bool {
    matches!(
        fact,
        RuntimeTypeFact::Primitive(
            PrimitiveTag::Bool
                | PrimitiveTag::Char
                | PrimitiveTag::I8
                | PrimitiveTag::I16
                | PrimitiveTag::I32
                | PrimitiveTag::I64
                | PrimitiveTag::U8
                | PrimitiveTag::U16
                | PrimitiveTag::U32
                | PrimitiveTag::U64
                | PrimitiveTag::String
                | PrimitiveTag::Bytes
        )
    )
}

fn missing_array_ord_error(
    method: &str,
    value_kind: &str,
    value_type: &str,
    span: Span,
) -> CompileError {
    CompileError::new(CompileErrorKind::SemanticDiagnostics(vec![
        Diagnostic::error(format!(
            "`Array.{method}` requires an `Ord` {value_kind}, but `{value_type}` does not implement `Ord`"
        ))
        .with_code("compiler::missing_ord_for_array_ordering")
        .with_span(span)
        .with_label(span, format!("static `Array.{method}` requires `Ord`"))
        .with_label(
            span,
            format!("add `impl Ord for {value_type}` or use a dynamic value"),
        ),
    ]))
}

pub(in crate::compiler) fn typed_container_mutation_arg_contract(
    receiver_type: Option<&RuntimeTypeFact>,
    method: &str,
    param_name: &str,
    position: usize,
) -> Option<RuntimeTypeFact> {
    match receiver_type? {
        RuntimeTypeFact::Array(element) => {
            match (method, mutation_arg_role(method, param_name, position)) {
                ("push" | "insert", MutationArgRole::Value) => Some((**element).clone()),
                ("extend", MutationArgRole::Values) => {
                    Some(RuntimeTypeFact::array((**element).clone()))
                }
                _ => None,
            }
        }
        RuntimeTypeFact::Map { key, value } => {
            match (method, mutation_arg_role(method, param_name, position)) {
                ("set", MutationArgRole::Key)
                    if !matches!(
                        key.as_ref(),
                        RuntimeTypeFact::Primitive(vela_common::PrimitiveTag::String)
                    ) =>
                {
                    Some((**key).clone())
                }
                ("set", MutationArgRole::Value) => Some((**value).clone()),
                ("extend", MutationArgRole::Values) => {
                    Some(RuntimeTypeFact::map((**key).clone(), (**value).clone()))
                }
                _ => None,
            }
        }
        RuntimeTypeFact::Set(element) => {
            match (method, mutation_arg_role(method, param_name, position)) {
                ("add", MutationArgRole::Value) => Some((**element).clone()),
                ("extend", MutationArgRole::Values) => {
                    Some(RuntimeTypeFact::set((**element).clone()))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MutationArgRole {
    Key,
    Value,
    Values,
    Other,
}

fn mutation_arg_role(method: &str, param_name: &str, position: usize) -> MutationArgRole {
    match param_name {
        "key" => MutationArgRole::Key,
        "value" => MutationArgRole::Value,
        "values" => MutationArgRole::Values,
        _ => match (method, position) {
            ("set", 0) => MutationArgRole::Key,
            ("set", 1) | ("insert", 1) | ("push", 0) | ("add", 0) => MutationArgRole::Value,
            ("extend", 0) => MutationArgRole::Values,
            _ => MutationArgRole::Other,
        },
    }
}

pub(in crate::compiler) fn mutation_arg_debug_name(
    method: &str,
    param_name: &str,
    position: usize,
) -> String {
    if param_name.is_empty() {
        match mutation_arg_role(method, param_name, position) {
            MutationArgRole::Key => "key",
            MutationArgRole::Value => "value",
            MutationArgRole::Values => "values",
            _ => "argument",
        }
        .to_owned()
    } else {
        param_name.to_owned()
    }
}

fn function_id_for_native_name(name: &str) -> FunctionId {
    if let Some((module, function)) = name.rsplit_once("::")
        && let Some(id) = vela_stdlib::std_function_id(module, function)
    {
        return id;
    }
    function_id_for_path("host", name)
}

fn function_id_for_path(package: &str, name: &str) -> FunctionId {
    let mut segments = name.split("::").collect::<Vec<_>>();
    let function = segments.pop().unwrap_or(name);
    FunctionId::from_def_id(DefPath::function(package, segments, function).id())
}
