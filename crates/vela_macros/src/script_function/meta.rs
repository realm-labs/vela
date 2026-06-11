use proc_macro2::TokenStream;
use quote::format_ident;
use syn::{FnArg, ItemFn, LitBool, LitStr, PatType, Result, ReturnType, Type, parse::Parser};
use vela_common::PrimitiveTag;

use crate::attrs::{
    error, parse_key_value_attr, parse_qualified_name, reject_duplicate_attr_keys, spanned_error,
};
use crate::signature::{
    is_mut_reference_to_type, param_name, reject_script_reference_param,
    reject_script_reference_return, reject_unsupported_integer_type, type_ident,
    wrapper_inner_type,
};

#[derive(Clone)]
pub(super) struct FunctionMeta {
    pub(super) id: u128,
    pub(super) name: String,
    pub(super) effect: FunctionEffect,
    pub(super) docs: Option<String>,
    pub(super) attrs: Vec<(String, String)>,
    pub(super) public: bool,
    pub(super) reflect_visible: bool,
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
    Primitive(PrimitiveTag),
    Array,
    Map,
    Set,
    PathProxy,
    Function,
}

#[derive(Clone, Debug, Default)]
pub(super) struct ScriptFunctionAttrs {
    name: Option<String>,
    alias: Option<String>,
    effect: Option<FunctionEffect>,
    pub(super) docs: Option<String>,
    attrs: Vec<(String, String)>,
    public: Option<bool>,
    reflect_visible: Option<bool>,
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
            "name" => {
                parsed.name = Some(parse_qualified_name(
                    value.parse::<LitStr>()?,
                    "script_function name",
                )?);
            }
            "alias" => {
                parsed.alias = Some(parse_qualified_name(
                    value.parse::<LitStr>()?,
                    "script_function alias",
                )?);
            }
            "effect" => {
                parsed.effect = Some(parse_effect(&value.parse::<LitStr>()?.value())?);
            }
            "docs" => parsed.docs = Some(value.parse::<LitStr>()?.value()),
            "attr" => parsed.attrs.push(parse_key_value_attr(
                value.parse::<LitStr>()?,
                "script_function",
            )?),
            "public" => {
                parsed.public = Some(value.parse::<LitBool>()?.value);
            }
            "reflect_visible" => {
                parsed.reflect_visible = Some(value.parse::<LitBool>()?.value);
            }
            "reflect" | "reflect_callable" => {
                parsed.reflect_callable = value.parse::<LitBool>()?.value;
            }
            _ => return Err(meta.error("unsupported script_function attribute")),
        }
        Ok(())
    });
    parser.parse2(attr)?;
    reject_duplicate_attr_keys(&parsed.attrs, "script_function")?;
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

    let name = attrs.name.ok_or_else(|| {
        error(
            item.sig.ident.span(),
            &format!(
                "{} requires name = \"module.function\" for stable ID generation",
                mode.attr_name()
            ),
        )
    })?;
    let stable_name = attrs.alias.unwrap_or_else(|| name.clone());
    let id = u128::from(vela_common::stable_id("native_function", "", &stable_name));
    let public = attrs.public.unwrap_or(true);
    let reflect_visible = attrs.reflect_visible.unwrap_or(public);

    Ok(FunctionMeta {
        id,
        name,
        effect: attrs.effect.unwrap_or(FunctionEffect::Pure),
        docs,
        attrs: attrs.attrs,
        public,
        reflect_visible,
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
        ReturnType::Default => HintKind::Primitive(PrimitiveTag::Null),
        ReturnType::Type(_, ty) => {
            if wrapper_inner_type(ty, &["Option"]).is_some() {
                HintKind::Any
            } else {
                return_wrapper_inner_hint(ty).unwrap_or_else(|| hint_for_type(ty))
            }
        }
    }
}

fn return_wrapper_inner_hint(ty: &Type) -> Option<HintKind> {
    wrapper_inner_type(ty, &["VmResult", "HostResult"]).map(hint_for_type)
}

fn hint_for_type(ty: &Type) -> HintKind {
    if is_unit_tuple(ty) {
        return HintKind::Primitive(PrimitiveTag::Null);
    }
    if matches!(ty, Type::Array(_)) {
        return HintKind::Array;
    }
    if wrapper_inner_type(ty, &["Option"]).is_some() {
        return HintKind::Any;
    }
    match type_ident(ty).as_deref() {
        Some("bool") => HintKind::Primitive(PrimitiveTag::Bool),
        Some("i8") => HintKind::Primitive(PrimitiveTag::I8),
        Some("i16") => HintKind::Primitive(PrimitiveTag::I16),
        Some("i32") => HintKind::Primitive(PrimitiveTag::I32),
        Some("i64") => HintKind::Primitive(PrimitiveTag::I64),
        Some("u8") => HintKind::Primitive(PrimitiveTag::U8),
        Some("u16") => HintKind::Primitive(PrimitiveTag::U16),
        Some("u32") => HintKind::Primitive(PrimitiveTag::U32),
        Some("f32") => HintKind::Primitive(PrimitiveTag::F32),
        Some("f64") => HintKind::Primitive(PrimitiveTag::F64),
        Some("String" | "str") => HintKind::Primitive(PrimitiveTag::String),
        Some("Vec") => HintKind::Array,
        Some("BTreeMap" | "HashMap") => HintKind::Map,
        Some("BTreeSet" | "HashSet") => HintKind::Set,
        Some("PathProxy") => HintKind::PathProxy,
        Some("Value") => HintKind::Any,
        Some("NativeFunction" | "HostNativeFunction") => HintKind::Function,
        _ => HintKind::Any,
    }
}

fn is_unit_tuple(ty: &Type) -> bool {
    matches!(ty, Type::Tuple(tuple) if tuple.elems.is_empty())
}
