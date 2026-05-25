use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    FnArg, ItemFn, LitBool, LitInt, LitStr, PatType, Result, ReturnType, Type, TypePath,
    parse::Parser, parse2,
};

use crate::attrs::{error, spanned_error};
use crate::signature::{docs_from_attrs, param_name, reject_script_reference_param, type_ident};

#[derive(Clone)]
struct FunctionMeta {
    id: u64,
    name: String,
    effect: FunctionEffect,
    docs: Option<String>,
    permissions: Vec<String>,
    reflect_callable: bool,
    params: Vec<ParamMeta>,
    returns: HintKind,
}

#[derive(Clone, Copy)]
enum FunctionMode {
    Pure,
    Context,
    Host,
}

#[derive(Clone)]
struct ParamMeta {
    name: String,
    ty: Type,
    hint: HintKind,
}

#[derive(Clone, Copy, Debug)]
enum FunctionEffect {
    Pure,
    HostRead,
    HostWrite,
    EventEmit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HintKind {
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
struct ScriptFunctionAttrs {
    id: Option<u64>,
    name: Option<String>,
    effect: Option<FunctionEffect>,
    docs: Option<String>,
    permissions: Vec<String>,
    reflect_callable: bool,
}

pub(crate) fn expand(attr: TokenStream, input: TokenStream) -> TokenStream {
    match expand_result(attr, input, FunctionMode::Pure) {
        Ok(tokens) => tokens,
        Err(error) => error.to_compile_error(),
    }
}

pub(crate) fn expand_context(attr: TokenStream, input: TokenStream) -> TokenStream {
    match expand_result(attr, input, FunctionMode::Context) {
        Ok(tokens) => tokens,
        Err(error) => error.to_compile_error(),
    }
}

pub(crate) fn expand_host(attr: TokenStream, input: TokenStream) -> TokenStream {
    match expand_result(attr, input, FunctionMode::Host) {
        Ok(tokens) => tokens,
        Err(error) => error.to_compile_error(),
    }
}

fn expand_result(attr: TokenStream, input: TokenStream, mode: FunctionMode) -> Result<TokenStream> {
    let item = parse2::<ItemFn>(input)?;
    if !item.sig.generics.params.is_empty() {
        return Err(spanned_error(
            &item.sig.generics,
            &format!("{} does not support generic functions", mode.attr_name()),
        ));
    }
    if item.sig.asyncness.is_some() {
        return Err(spanned_error(
            &item.sig.asyncness,
            &format!("{} does not support async functions", mode.attr_name()),
        ));
    }

    let attrs = parse_script_function_attrs(attr)?;
    let id = attrs.id.ok_or_else(|| {
        error(
            item.sig.ident.span(),
            &format!("script functions require {}(id = N)", mode.attr_name()),
        )
    })?;
    let docs = attrs.docs.clone().or_else(|| docs_from_attrs(&item.attrs));
    let meta = function_meta(&item, attrs, id, docs, mode)?;
    let fn_ident = item.sig.ident.clone();
    let desc_name = format_ident!("vela_native_function_desc_{}", fn_ident);
    let register_name = mode.register_helper_ident(&fn_ident);
    let desc_tokens = desc_tokens(&meta);
    let args_tuple = args_tuple_tokens(&meta.params);
    let register_tokens = register_tokens(mode, &args_tuple, &desc_name, &fn_ident);

    Ok(quote! {
        #item

        #[must_use]
        pub fn #desc_name() -> ::vela_engine::NativeFunctionDesc {
            #desc_tokens
        }

        #[must_use]
        pub fn #register_name(
            builder: ::vela_engine::EngineBuilder,
        ) -> ::vela_engine::EngineBuilder {
            #register_tokens
        }
    })
}

impl FunctionMode {
    fn attr_name(self) -> &'static str {
        match self {
            Self::Pure => "#[script_function]",
            Self::Context => "#[script_context_function]",
            Self::Host => "#[script_host_function]",
        }
    }

    fn register_helper_ident(self, fn_ident: &syn::Ident) -> syn::Ident {
        match self {
            Self::Pure => format_ident!("vela_register_native_function_{}", fn_ident),
            Self::Context => format_ident!("vela_register_context_native_function_{}", fn_ident),
            Self::Host => format_ident!("vela_register_host_native_function_{}", fn_ident),
        }
    }

    fn first_parameter_name(self) -> &'static str {
        match self {
            Self::Pure => "",
            Self::Context => "NativeCallContext",
            Self::Host => "HostExecution",
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

fn parse_script_function_attrs(attr: TokenStream) -> Result<ScriptFunctionAttrs> {
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

fn function_meta(
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
        if matches!(mode, FunctionMode::Pure) && is_context_param(param) {
            return Err(spanned_error(
                input,
                "use #[script_context_function] for NativeCallContext callbacks",
            ));
        }
        if matches!(mode, FunctionMode::Pure) && is_host_execution_param(param) {
            return Err(spanned_error(
                input,
                "use #[script_host_function] for HostExecution callbacks",
            ));
        }
        reject_script_reference_param(param)?;
        params.push(ParamMeta {
            name: param_name(param),
            ty: param.ty.as_ref().clone(),
            hint: hint_for_type(&param.ty),
        });
    }

    Ok(FunctionMeta {
        id,
        name: attrs.name.unwrap_or_else(|| item.sig.ident.to_string()),
        effect: attrs.effect.unwrap_or(FunctionEffect::Pure),
        docs,
        permissions: attrs.permissions,
        reflect_callable: attrs.reflect_callable,
        params,
        returns: return_hint(&item.sig.output),
    })
}

fn is_context_param(param: &PatType) -> bool {
    type_ident(&param.ty).is_some_and(|ident| ident == "NativeCallContext")
}

fn is_host_execution_param(param: &PatType) -> bool {
    type_ident(&param.ty).is_some_and(|ident| ident == "HostExecution")
}

fn return_hint(output: &ReturnType) -> HintKind {
    match output {
        ReturnType::Default => HintKind::Null,
        ReturnType::Type(_, ty) => result_inner_hint(ty).unwrap_or_else(|| hint_for_type(ty)),
    }
}

fn result_inner_hint(ty: &Type) -> Option<HintKind> {
    let Type::Path(TypePath { path, .. }) = ty else {
        return None;
    };
    let segment = path.segments.last()?;
    let ident = segment.ident.to_string();
    if !matches!(ident.as_str(), "Result" | "VmResult") {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    args.args.iter().find_map(|arg| match arg {
        syn::GenericArgument::Type(ty) => Some(hint_for_type(ty)),
        _ => None,
    })
}

fn hint_for_type(ty: &Type) -> HintKind {
    if is_unit_tuple(ty) {
        return HintKind::Null;
    }
    match type_ident(ty).as_deref() {
        Some("bool") => HintKind::Bool,
        Some(
            "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64" | "u128"
            | "usize",
        ) => HintKind::Int,
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

fn desc_tokens(function: &FunctionMeta) -> TokenStream {
    let id = function.id;
    let name = &function.name;
    let effect = effect_tokens(function.effect);
    let returns = hint_tokens(function.returns);
    let params = function.params.iter().map(param_tokens);
    let access = access_tokens(function);
    let docs = function
        .docs
        .as_ref()
        .map(|docs| quote! { desc = desc.docs(#docs); });

    quote! {
        {
            let mut desc = ::vela_engine::NativeFunctionDesc::new(
                #name,
                ::vela_engine::NativeFunctionId::new(#id),
            )
            .effects(#effect)
            .returns(#returns)
            .access(#access);
            #(
                desc = desc.param(#params);
            )*
            #docs
            desc
        }
    }
}

fn param_tokens(param: &ParamMeta) -> TokenStream {
    let name = &param.name;
    let hint = hint_tokens(param.hint);
    quote! { #name, #hint }
}

fn args_tuple_tokens(params: &[ParamMeta]) -> TokenStream {
    match params {
        [] => quote! { () },
        [param] => {
            let ty = &param.ty;
            quote! { (#ty,) }
        }
        params => {
            let types = params.iter().map(|param| &param.ty);
            quote! { (#(#types),*) }
        }
    }
}

fn register_tokens(
    mode: FunctionMode,
    args_tuple: &TokenStream,
    desc_name: &syn::Ident,
    fn_ident: &syn::Ident,
) -> TokenStream {
    match mode {
        FunctionMode::Pure => {
            quote! { builder.register_typed_native_fn::<#args_tuple, _>(#desc_name(), #fn_ident) }
        }
        FunctionMode::Context => {
            quote! {
                builder.register_typed_context_host_native_fn::<#args_tuple, _>(
                    #desc_name(),
                    #fn_ident,
                )
            }
        }
        FunctionMode::Host => {
            quote! {
                builder.register_typed_host_native_fn::<#args_tuple, _>(
                    #desc_name(),
                    #fn_ident,
                )
            }
        }
    }
}

fn effect_tokens(effect: FunctionEffect) -> TokenStream {
    match effect {
        FunctionEffect::Pure => quote! { ::vela_engine::EffectSet::pure() },
        FunctionEffect::HostRead => quote! { ::vela_engine::EffectSet::host_read() },
        FunctionEffect::HostWrite => quote! { ::vela_engine::EffectSet::host_write() },
        FunctionEffect::EventEmit => quote! { ::vela_engine::EffectSet::event_emit() },
    }
}

fn hint_tokens(hint: HintKind) -> TokenStream {
    match hint {
        HintKind::Any => quote! { ::vela_engine::TypeHint::Any },
        HintKind::Null => quote! { ::vela_engine::TypeHint::Null },
        HintKind::Bool => quote! { ::vela_engine::TypeHint::Bool },
        HintKind::Int => quote! { ::vela_engine::TypeHint::Int },
        HintKind::Float => quote! { ::vela_engine::TypeHint::Float },
        HintKind::String => quote! { ::vela_engine::TypeHint::String },
        HintKind::Array => quote! { ::vela_engine::TypeHint::Array },
        HintKind::Map => quote! { ::vela_engine::TypeHint::Map },
        HintKind::Set => quote! { ::vela_engine::TypeHint::Set },
        HintKind::Function => quote! { ::vela_engine::TypeHint::Function },
    }
}

fn access_tokens(function: &FunctionMeta) -> TokenStream {
    let reflect_callable = function.reflect_callable;
    let permissions = function.permissions.iter().map(|permission| {
        quote! {
            access = access.require_permission(#permission);
        }
    });

    quote! {
        {
            let mut access =
                ::vela_engine::FunctionAccess::public().reflect_callable(#reflect_callable);
            #(#permissions)*
            access
        }
    }
}

#[cfg(test)]
mod tests {
    use quote::quote;

    use super::{FunctionMode, expand_result};

    #[test]
    fn rejects_missing_function_id() {
        let error = expand_result(
            quote! {},
            quote! {
                fn grant(amount: i64) -> i64 {
                    amount
                }
            },
            FunctionMode::Pure,
        )
        .expect_err("missing function id should fail macro expansion");

        assert!(error.to_string().contains("script functions require"));
    }

    #[test]
    fn rejects_generic_functions() {
        let error = expand_result(
            quote! { id = 1 },
            quote! {
                fn identity<T>(value: T) -> T {
                    value
                }
            },
            FunctionMode::Pure,
        )
        .expect_err("generic function should fail macro expansion");

        assert!(error.to_string().contains("generic functions"));
    }

    #[test]
    fn rejects_context_functions_without_context_param() {
        let error = expand_result(
            quote! { id = 1 },
            quote! {
                fn emit_event(amount: i64) -> bool {
                    amount > 0
                }
            },
            FunctionMode::Context,
        )
        .expect_err("missing context parameter should fail macro expansion");

        assert!(error.to_string().contains("NativeCallContext"));
    }

    #[test]
    fn rejects_host_functions_without_host_execution_param() {
        let error = expand_result(
            quote! { id = 1 },
            quote! {
                fn write_host(amount: i64) -> bool {
                    amount > 0
                }
            },
            FunctionMode::Host,
        )
        .expect_err("missing host execution parameter should fail macro expansion");

        assert!(error.to_string().contains("HostExecution"));
    }

    #[test]
    fn rejects_script_visible_rust_reference_parameters() {
        let error = expand_result(
            quote! { id = 1 },
            quote! {
                fn mutate_player(player: &mut Player) {}
            },
            FunctionMode::Pure,
        )
        .expect_err("script-visible Rust references should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("script-visible parameters cannot use Rust references")
        );
    }
}
