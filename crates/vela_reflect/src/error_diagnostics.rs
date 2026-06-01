use crate::ReflectErrorKind;
use vela_common::Span;

impl ReflectErrorKind {
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnknownType { .. } => "reflect::unknown_type",
            Self::UnknownTypeName { .. } => "reflect::unknown_type_name",
            Self::UnknownField { .. } => "reflect::unknown_field",
            Self::UnknownMethod { .. } => "reflect::unknown_method",
            Self::UnknownVariant { .. } => "reflect::unknown_variant",
            Self::UnknownTrait { .. } => "reflect::unknown_trait",
            Self::UnknownModule { .. } => "reflect::unknown_module",
            Self::UnknownFunction { .. } => "reflect::unknown_function",
            Self::UnknownPermission { .. } => "reflect::unknown_permission",
            Self::PermissionDenied { .. } => "reflect::permission_denied",
            Self::MethodNotReflectCallable { .. } => "reflect::method_not_reflect_callable",
            Self::FunctionNotReflectVisible { .. } => "reflect::function_not_reflect_visible",
            Self::FunctionNotReflectCallable { .. } => "reflect::function_not_reflect_callable",
            Self::MethodPermissionDenied { .. } => "reflect::method_permission_denied",
            Self::MethodEffectPermissionDenied { .. } => "reflect::method_effect_permission_denied",
            Self::FunctionEffectPermissionDenied { .. } => {
                "reflect::function_effect_permission_denied"
            }
            Self::FunctionPermissionDenied { .. } => "reflect::function_permission_denied",
            Self::FieldPermissionDenied { .. } => "reflect::field_permission_denied",
            Self::LookupBudgetExceeded { .. } => "reflect::lookup_budget_exceeded",
            Self::FieldNotWritable { .. } => "reflect::field_not_writable",
            Self::FieldNotReflectReadable { .. } => "reflect::field_not_reflect_readable",
            Self::FieldNotReflectWritable { .. } => "reflect::field_not_reflect_writable",
            Self::InvalidTarget => "reflect::invalid_target",
            Self::InvalidValue => "reflect::invalid_value",
            Self::Host(_) => "reflect::host_error",
        }
    }

    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::UnknownType { host_type_id } => {
                format!("unknown reflected host type `{}`", host_type_id.get())
            }
            Self::UnknownTypeName {
                type_name,
                candidates,
                ..
            } => unknown_name_message("type", type_name, candidates),
            Self::UnknownField {
                type_name,
                field,
                candidates,
                ..
            } => unknown_member_message("field", field, type_name, candidates),
            Self::UnknownMethod {
                type_name,
                method,
                candidates,
                ..
            } => unknown_member_message("method", method, type_name, candidates),
            Self::UnknownVariant {
                type_name,
                variant,
                candidates,
                ..
            } => unknown_member_message("variant", variant, type_name, candidates),
            Self::UnknownTrait {
                trait_name,
                candidates,
                ..
            } => unknown_name_message("trait", trait_name, candidates),
            Self::UnknownModule {
                module, candidates, ..
            } => unknown_name_message("module", module, candidates),
            Self::UnknownFunction {
                function,
                candidates,
                ..
            } => unknown_name_message("function", function, candidates),
            Self::UnknownPermission {
                permission,
                candidates,
            } => unknown_name_message("permission", permission, candidates),
            Self::PermissionDenied { permission } => {
                format!("reflection requires permission `{}`", permission.as_str())
            }
            Self::MethodNotReflectCallable { type_name, method } => {
                format!("method `{type_name}.{method}` is not reflect-callable")
            }
            Self::FunctionNotReflectVisible { function } => {
                format!("function `{function}` is not reflect-visible")
            }
            Self::FunctionNotReflectCallable { function } => {
                format!("function `{function}` is not reflect-callable")
            }
            Self::MethodPermissionDenied { method, permission } => {
                format!("method `{method}` requires permission `{permission}`")
            }
            Self::MethodEffectPermissionDenied { method, permission } => {
                format!(
                    "method `{method}` requires reflection effect permission `{}`",
                    permission.as_str()
                )
            }
            Self::FunctionEffectPermissionDenied {
                function,
                permission,
            } => {
                format!(
                    "function `{function}` requires reflection effect permission `{}`",
                    permission.as_str()
                )
            }
            Self::FunctionPermissionDenied {
                function,
                permission,
            } => {
                format!("function `{function}` requires permission `{permission}`")
            }
            Self::FieldPermissionDenied {
                type_name,
                field,
                permission,
            } => {
                format!("field `{type_name}.{field}` requires permission `{permission}`")
            }
            Self::LookupBudgetExceeded { limit } => {
                format!("reflection lookup budget exceeded with limit {limit}")
            }
            Self::FieldNotWritable { type_name, field } => {
                format!("field `{type_name}.{field}` is not writable")
            }
            Self::FieldNotReflectReadable { type_name, field } => {
                format!("field `{type_name}.{field}` is not reflect-readable")
            }
            Self::FieldNotReflectWritable { type_name, field } => {
                format!("field `{type_name}.{field}` is not reflect-writable")
            }
            Self::InvalidTarget => "invalid reflection target".to_owned(),
            Self::InvalidValue => "invalid reflection value".to_owned(),
            Self::Host(message) => format!("host reflection error: {message}"),
        }
    }

    #[must_use]
    pub fn related_labels(&self) -> Vec<(Span, String)> {
        let related = match self {
            Self::UnknownTypeName { related, .. }
            | Self::UnknownField { related, .. }
            | Self::UnknownMethod { related, .. }
            | Self::UnknownVariant { related, .. }
            | Self::UnknownTrait { related, .. }
            | Self::UnknownModule { related, .. }
            | Self::UnknownFunction { related, .. } => related,
            _ => return Vec::new(),
        };
        related
            .iter()
            .filter_map(|candidate| {
                candidate.source_span.map(|span| {
                    (
                        span,
                        format!("candidate `{}` is declared here", candidate.name),
                    )
                })
            })
            .collect()
    }
}

fn unknown_name_message(kind: &str, name: &str, candidates: &[String]) -> String {
    let mut message = format!("unknown reflected {kind} `{name}`");
    add_candidate_hint(&mut message, candidates);
    message
}

fn unknown_member_message(kind: &str, member: &str, owner: &str, candidates: &[String]) -> String {
    let mut message = format!("unknown reflected {kind} `{member}` on `{owner}`");
    add_candidate_hint(&mut message, candidates);
    message
}

fn add_candidate_hint(message: &mut String, candidates: &[String]) {
    if !candidates.is_empty() {
        message.push_str("; candidates: ");
        message.push_str(&candidates.join(", "));
    }
}
