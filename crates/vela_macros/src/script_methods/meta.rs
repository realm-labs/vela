use std::collections::BTreeSet;

use syn::{
    Attribute, FnArg, ImplItem, ImplItemFn, ItemImpl, LitBool, LitStr, PatType, Result, ReturnType,
    Type,
};

use crate::attrs::{
    error, parse_key_value_attr, parse_permission, reject_duplicate_attr_keys, spanned_error,
};
use crate::signature::{
    docs_from_attrs, is_mut_reference_to_type, is_shared_reference_to_type, param_name,
    reject_extern_signature, reject_generic_signature, reject_script_reference_param,
    reject_script_reference_return, reject_unsafe_signature, reject_unsupported_integer_type,
    type_ident, wrapper_inner_type,
};

#[derive(Clone)]
pub(super) struct MethodMeta {
    pub(super) ident: syn::Ident,
    pub(super) name: String,
    pub(super) stable_name: String,
    pub(super) effect: MethodEffect,
    pub(super) docs: Option<String>,
    pub(super) attrs: Vec<(String, String)>,
    pub(super) permissions: Vec<String>,
    pub(super) reflect_callable: bool,
    pub(super) params: Vec<ParamMeta>,
    pub(super) returns: HintKind,
    pub(super) callable_native: bool,
}

#[derive(Clone)]
pub(super) struct ParamMeta {
    pub(super) name: String,
    pub(super) ty: Type,
    pub(super) hint: HintKind,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum MethodEffect {
    Pure,
    HostRead,
    HostWrite,
    EventEmit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HintKind {
    Any,
    Null,
    Bool,
    Int,
    Float,
    String,
    Array,
    Map,
    Set,
    HostOwner,
    Function,
}

#[derive(Clone, Debug, Default)]
struct ScriptMethodAttrs {
    has_attr: bool,
    name: Option<String>,
    alias: Option<String>,
    effect: Option<MethodEffect>,
    docs: Option<String>,
    attrs: Vec<(String, String)>,
    permissions: Vec<String>,
    reflect_callable: bool,
}

pub(super) fn collect_methods(item: &mut ItemImpl) -> Result<Vec<MethodMeta>> {
    let mut seen_stable_names = BTreeSet::new();
    let mut seen_names = BTreeSet::new();
    let mut methods = Vec::new();
    for impl_item in &mut item.items {
        let ImplItem::Fn(method) = impl_item else {
            continue;
        };
        let attrs = parse_script_method_attrs(&method.attrs)?;
        if !attrs.has_attr {
            continue;
        }
        let name = attrs
            .name
            .clone()
            .unwrap_or_else(|| method.sig.ident.to_string());
        if name.is_empty() {
            return Err(error(
                method.sig.ident.span(),
                "script method name cannot be empty",
            ));
        }
        if !seen_names.insert(name) {
            return Err(error(
                method.sig.ident.span(),
                "duplicate script method name",
            ));
        }
        let stable_name = attrs.alias.clone().unwrap_or_else(|| {
            attrs
                .name
                .clone()
                .unwrap_or_else(|| method.sig.ident.to_string())
        });
        if stable_name.is_empty() {
            return Err(error(
                method.sig.ident.span(),
                "script method alias cannot be empty",
            ));
        }
        if !seen_stable_names.insert(stable_name.clone()) {
            return Err(error(
                method.sig.ident.span(),
                "duplicate script method alias",
            ));
        }
        let docs = attrs
            .docs
            .clone()
            .or_else(|| docs_from_attrs(&method.attrs));
        methods.push(method_meta(method, attrs, stable_name, docs)?);
        method
            .attrs
            .retain(|attr| !attr.path().is_ident("script_method"));
    }

    Ok(methods)
}

fn parse_script_method_attrs(attrs: &[Attribute]) -> Result<ScriptMethodAttrs> {
    let mut parsed = ScriptMethodAttrs::default();
    for attr in attrs {
        if !attr.path().is_ident("script_method") {
            continue;
        }

        parsed.has_attr = true;
        attr.parse_nested_meta(|meta| {
            let Some(ident) = meta.path.get_ident() else {
                return Err(meta.error("unsupported script_method attribute"));
            };
            let name = ident.to_string();
            let value = meta.value()?;
            match name.as_str() {
                "name" => parsed.name = Some(value.parse::<LitStr>()?.value()),
                "alias" => parsed.alias = Some(value.parse::<LitStr>()?.value()),
                "effect" => {
                    parsed.effect = Some(parse_effect(&value.parse::<LitStr>()?.value())?);
                }
                "docs" => parsed.docs = Some(value.parse::<LitStr>()?.value()),
                "attr" => parsed.attrs.push(parse_key_value_attr(
                    value.parse::<LitStr>()?,
                    "script_method",
                )?),
                "permission" => parsed
                    .permissions
                    .push(parse_permission(value.parse::<LitStr>()?, "script_method")?),
                "reflect" | "reflect_callable" => {
                    parsed.reflect_callable = value.parse::<LitBool>()?.value;
                }
                _ => return Err(meta.error("unsupported script_method attribute")),
            }
            Ok(())
        })?;
    }
    parsed.permissions.sort();
    parsed.permissions.dedup();
    reject_duplicate_attr_keys(&parsed.attrs, "script_method")?;
    Ok(parsed)
}

fn parse_effect(effect: &str) -> Result<MethodEffect> {
    match effect {
        "pure" => Ok(MethodEffect::Pure),
        "read_host" | "host_read" => Ok(MethodEffect::HostRead),
        "write_host" | "host_write" => Ok(MethodEffect::HostWrite),
        "event_emit" | "emit_event" => Ok(MethodEffect::EventEmit),
        _ => Err(error(
            proc_macro2::Span::call_site(),
            "unsupported script_method effect",
        )),
    }
}

fn method_meta(
    method: &ImplItemFn,
    attrs: ScriptMethodAttrs,
    stable_name: String,
    docs: Option<String>,
) -> Result<MethodMeta> {
    reject_generic_signature(&method.sig.generics, "#[script_method]")?;
    if method.sig.asyncness.is_some() {
        return Err(spanned_error(
            &method.sig.asyncness,
            "#[script_method] does not support async methods",
        ));
    }
    reject_unsafe_signature(&method.sig, "#[script_method]")?;
    reject_extern_signature(&method.sig, "#[script_method]")?;

    let mut params = Vec::new();
    let mut skipped_receiver = false;
    for input in &method.sig.inputs {
        let param = match input {
            FnArg::Typed(param) => param,
            FnArg::Receiver(_) => {
                return Err(spanned_error(
                    input,
                    "script methods must use HostRef receiver parameters instead of self",
                ));
            }
        };
        if is_context_type(param) {
            if !is_context_param(param) {
                return Err(spanned_error(
                    input,
                    "NativeCallContext boundary parameters must be &mut NativeCallContext",
                ));
            }
            continue;
        }
        if is_host_execution_type(param) {
            if !is_host_execution_param(param) {
                return Err(spanned_error(
                    input,
                    "HostExecution boundary parameters must be &mut HostExecution",
                ));
            }
            continue;
        }
        if !skipped_receiver && (is_host_ref(&param.ty) || is_host_path(&param.ty)) {
            skipped_receiver = true;
            continue;
        }
        reject_script_reference_param(param)?;
        reject_unsupported_integer_type(&param.ty)?;
        params.push(ParamMeta {
            name: param_name(param),
            ty: param.ty.as_ref().clone(),
            hint: hint_for_type(&param.ty),
        });
    }
    reject_return_type(&method.sig.output)?;

    Ok(MethodMeta {
        ident: method.sig.ident.clone(),
        name: attrs.name.unwrap_or_else(|| method.sig.ident.to_string()),
        stable_name,
        effect: attrs.effect.unwrap_or(MethodEffect::Pure),
        docs,
        attrs: attrs.attrs,
        permissions: attrs.permissions,
        reflect_callable: attrs.reflect_callable,
        params,
        returns: return_hint(&method.sig.output),
        callable_native: has_callable_native_boundary(method),
    })
}

fn reject_return_type(output: &ReturnType) -> Result<()> {
    match output {
        ReturnType::Default => Ok(()),
        ReturnType::Type(_, ty) => {
            reject_script_reference_return(ty)?;
            reject_unsupported_integer_type(ty)
        }
    }
}

fn is_context_param(param: &PatType) -> bool {
    is_mut_reference_to_type(&param.ty, "NativeCallContext")
}

fn is_host_execution_param(param: &PatType) -> bool {
    is_mut_reference_to_type(&param.ty, "HostExecution")
}

fn is_host_ref(ty: &Type) -> bool {
    type_ident(ty).is_some_and(|ident| ident == "HostRef")
}

fn is_host_path(ty: &Type) -> bool {
    type_ident(ty).is_some_and(|ident| ident == "HostPath")
}

fn is_context_type(param: &PatType) -> bool {
    type_ident(&param.ty).is_some_and(|ident| ident == "NativeCallContext")
}

fn is_host_execution_type(param: &PatType) -> bool {
    type_ident(&param.ty).is_some_and(|ident| ident == "HostExecution")
}

fn has_callable_native_boundary(method: &ImplItemFn) -> bool {
    let mut inputs = method.sig.inputs.iter();
    let Some(FnArg::Typed(receiver)) = inputs.next() else {
        return false;
    };
    let Some(FnArg::Typed(host)) = inputs.next() else {
        return false;
    };
    is_shared_reference_to_type(&receiver.ty, "HostPath") && is_host_execution_param(host)
}

fn return_hint(output: &ReturnType) -> HintKind {
    match output {
        ReturnType::Default => HintKind::Null,
        ReturnType::Type(_, ty) => {
            return_wrapper_inner_hint(ty).unwrap_or_else(|| hint_for_type(ty))
        }
    }
}

fn return_wrapper_inner_hint(ty: &Type) -> Option<HintKind> {
    wrapper_inner_type(ty, &["Option", "VmResult", "HostResult"]).map(hint_for_type)
}

fn hint_for_type(ty: &Type) -> HintKind {
    if is_unit_tuple(ty) {
        return HintKind::Null;
    }
    if matches!(ty, Type::Array(_)) {
        return HintKind::Array;
    }
    if let Some(inner) = wrapper_inner_type(ty, &["Option"]) {
        return hint_for_type(inner);
    }
    match type_ident(ty).as_deref() {
        Some("bool") => HintKind::Bool,
        Some("i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32") => HintKind::Int,
        Some("f32" | "f64") => HintKind::Float,
        Some("String" | "str") => HintKind::String,
        Some("Vec") => HintKind::Array,
        Some("BTreeMap" | "HashMap") => HintKind::Map,
        Some("BTreeSet" | "HashSet") => HintKind::Set,
        Some("HostRef") => HintKind::HostOwner,
        Some("Value") => HintKind::Any,
        Some("NativeFunction" | "HostNativeFunction") => HintKind::Function,
        _ => HintKind::Any,
    }
}

fn is_unit_tuple(ty: &Type) -> bool {
    matches!(ty, Type::Tuple(tuple) if tuple.elems.is_empty())
}
