use proc_macro2::TokenStream;
use quote::format_ident;
use syn::{
    FnArg, ItemFn, LitBool, LitInt, LitStr, PatType, Result, ReturnType, Type, parse::Parser,
};

use crate::attrs::{error, parse_key_value_attr, spanned_error};
use crate::signature::{
    is_mut_reference_to_type, param_name, reject_script_reference_param,
    reject_script_reference_return, reject_unsupported_integer_type, type_ident,
    wrapper_inner_type,
};

#[derive(Clone)]
pub(super) struct FunctionMeta {
    pub(super) id: u64,
    pub(super) name: String,
    pub(super) effect: FunctionEffect,
    pub(super) docs: Option<String>,
    pub(super) attrs: Vec<(String, String)>,
    pub(super) permissions: Vec<String>,
    pub(super) reflect_callable: bool,
    pub(super) params: Vec<ParamMeta>,
    pub(super) returns: HintKind,
}

#[derive(Clone, Copy)]
pub(super) enum FunctionMode {
    Pure,
    Context,
    Host,
}

#[derive(Clone)]
pub(super) struct ParamMeta {
    pub(super) name: String,
    pub(super) ty: Type,
    pub(super) hint: HintKind,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum FunctionEffect {
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
    Function,
}

#[derive(Clone, Debug, Default)]
pub(super) struct ScriptFunctionAttrs {
    pub(super) id: Option<u64>,
    name: Option<String>,
    effect: Option<FunctionEffect>,
    pub(super) docs: Option<String>,
    attrs: Vec<(String, String)>,
    permissions: Vec<String>,
    reflect_callable: bool,
}

impl FunctionMode {
    pub(super) fn attr_name(self) -> &'static str {
        match self {
            Self::Pure => "#[script_function]",
            Self::Context => "#[script_context_function]",
            Self::Host => "#[script_host_function]",
        }
    }

    pub(super) fn register_helper_ident(self, fn_ident: &syn::Ident) -> syn::Ident {
        match self {
            Self::Pure => format_ident!("vela_register_native_function_{}", fn_ident),
            Self::Context => format_ident!("vela_register_context_native_function_{}", fn_ident),
            Self::Host => format_ident!("vela_register_host_native_function_{}", fn_ident),
        }
    }

    fn first_parameter_name(self) -> &'static str {
        match self {
            Self::Pure => "",
            Self::Context => "&mut NativeCallContext",
            Self::Host => "&mut HostExecution",
        }
    }

    fn is_boundary_param(self, param: &PatType) -> bool {
        match self {
            Self::Pure => false,
            Self::Context => is_context_param(param),
            Self::Host => is_host_execution_param(param),
        }
    }
}

pub(super) fn parse_script_function_attrs(attr: TokenStream) -> Result<ScriptFunctionAttrs> {
    let mut parsed = ScriptFunctionAttrs::default();
    let parser = syn::meta::parser(|meta| {
        let Some(ident) = meta.path.get_ident() else {
            return Err(meta.error("unsupported script_function attribute"));
        };
        let name = ident.to_string();
        let value = meta.value()?;
        match name.as_str() {
            "id" => parsed.id = Some(value.parse::<LitInt>()?.base10_parse()?),
            "name" => parsed.name = Some(value.parse::<LitStr>()?.value()),
            "effect" => {
                parsed.effect = Some(parse_effect(&value.parse::<LitStr>()?.value())?);
            }
            "docs" => parsed.docs = Some(value.parse::<LitStr>()?.value()),
            "attr" => parsed.attrs.push(parse_key_value_attr(
                value.parse::<LitStr>()?,
                "script_function",
            )?),
            "permission" => parsed.permissions.push(value.parse::<LitStr>()?.value()),
            "reflect" | "reflect_callable" => {
                parsed.reflect_callable = value.parse::<LitBool>()?.value;
            }
            _ => return Err(meta.error("unsupported script_function attribute")),
        }
        Ok(())
    });
    parser.parse2(attr)?;
    parsed.permissions.sort();
    parsed.permissions.dedup();
    Ok(parsed)
}

fn parse_effect(effect: &str) -> Result<FunctionEffect> {
    match effect {
        "pure" => Ok(FunctionEffect::Pure),
        "read_host" | "host_read" => Ok(FunctionEffect::HostRead),
        "write_host" | "host_write" => Ok(FunctionEffect::HostWrite),
        "event_emit" | "emit_event" => Ok(FunctionEffect::EventEmit),
        _ => Err(error(
            proc_macro2::Span::call_site(),
            "unsupported script_function effect",
        )),
    }
}

pub(super) fn function_meta(
    item: &ItemFn,
    attrs: ScriptFunctionAttrs,
    id: u64,
    docs: Option<String>,
    mode: FunctionMode,
) -> Result<FunctionMeta> {
    let mut params = Vec::new();
    let mut inputs = item.sig.inputs.iter();
    if matches!(mode, FunctionMode::Context | FunctionMode::Host) {
        let Some(input) = inputs.next() else {
            return Err(error(
                item.sig.ident.span(),
                &format!(
                    "{} requires a {} first parameter",
                    mode.attr_name(),
                    mode.first_parameter_name()
                ),
            ));
        };
        let FnArg::Typed(param) = input else {
            return Err(spanned_error(
                input,
                &format!("{} cannot use Rust self receivers", mode.attr_name()),
            ));
        };
        if !mode.is_boundary_param(param) {
            return Err(spanned_error(
                input,
                &format!(
                    "{} first parameter must be {}",
                    mode.attr_name(),
                    mode.first_parameter_name()
                ),
            ));
        }
    }

    for input in inputs {
        let FnArg::Typed(param) = input else {
            return Err(spanned_error(
                input,
                "script functions cannot use Rust self receivers",
            ));
        };
        if is_context_type(param) {
            return Err(spanned_error(
                input,
                "use #[script_context_function] for NativeCallContext callbacks",
            ));
        }
        if is_host_execution_type(param) {
            return Err(spanned_error(
                input,
                "use #[script_host_function] for HostExecution callbacks",
            ));
        }
        reject_script_reference_param(param)?;
        reject_unsupported_integer_type(&param.ty)?;
        params.push(ParamMeta {
            name: param_name(param),
            ty: param.ty.as_ref().clone(),
            hint: hint_for_type(&param.ty),
        });
    }
    reject_return_type(&item.sig.output)?;

    let name = attrs.name.unwrap_or_else(|| item.sig.ident.to_string());
    if name.is_empty() {
        return Err(error(
            item.sig.ident.span(),
            "script function name cannot be empty",
        ));
    }

    Ok(FunctionMeta {
        id,
        name,
        effect: attrs.effect.unwrap_or(FunctionEffect::Pure),
        docs,
        attrs: attrs.attrs,
        permissions: attrs.permissions,
        reflect_callable: attrs.reflect_callable,
        params,
        returns: return_hint(&item.sig.output),
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

fn is_context_type(param: &PatType) -> bool {
    type_ident(&param.ty).is_some_and(|ident| ident == "NativeCallContext")
}

fn is_host_execution_type(param: &PatType) -> bool {
    type_ident(&param.ty).is_some_and(|ident| ident == "HostExecution")
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
    wrapper_inner_type(ty, &["Option", "VmResult"]).map(hint_for_type)
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
        Some("Value") => HintKind::Any,
        Some("NativeFunction" | "HostNativeFunction") => HintKind::Function,
        _ => HintKind::Any,
    }
}

fn is_unit_tuple(ty: &Type) -> bool {
    matches!(ty, Type::Tuple(tuple) if tuple.elems.is_empty())
}
