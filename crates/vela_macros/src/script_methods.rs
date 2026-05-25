use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    Attribute, FnArg, ImplItem, ImplItemFn, ItemImpl, LitBool, LitInt, LitStr, Pat, PatType,
    Result, ReturnType, Type, TypePath, parse2,
};

use crate::attrs::{error, spanned_error};

#[derive(Clone)]
struct MethodMeta {
    ident: syn::Ident,
    id: u32,
    name: String,
    effect: MethodEffect,
    docs: Option<String>,
    permissions: Vec<String>,
    reflect_callable: bool,
    params: Vec<ParamMeta>,
    returns: HintKind,
    callable_native: bool,
}

#[derive(Clone)]
struct ParamMeta {
    name: String,
    ty: Type,
    hint: HintKind,
}

#[derive(Clone, Copy, Debug)]
enum MethodEffect {
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
    HostOwner,
    Function,
}

#[derive(Clone, Debug, Default)]
struct ScriptMethodAttrs {
    has_attr: bool,
    id: Option<u32>,
    name: Option<String>,
    effect: Option<MethodEffect>,
    docs: Option<String>,
    permissions: Vec<String>,
    reflect_callable: bool,
}

pub(crate) fn expand(input: TokenStream) -> TokenStream {
    match expand_result(input) {
        Ok(tokens) => tokens,
        Err(error) => error.to_compile_error(),
    }
}

pub(crate) fn expand_standalone_method(input: TokenStream) -> TokenStream {
    let error = error(
        proc_macro2::Span::call_site(),
        "#[script_method] must be used inside #[script_methods]",
    )
    .to_compile_error();
    quote! {
        #error
        #input
    }
}

fn expand_result(input: TokenStream) -> Result<TokenStream> {
    let mut item = parse2::<ItemImpl>(input)?;
    if item.trait_.is_some() {
        return Err(spanned_error(
            &item,
            "#[script_methods] only supports inherent impl blocks",
        ));
    }
    if !item.generics.params.is_empty() {
        return Err(spanned_error(
            &item.generics,
            "#[script_methods] does not support generic impl blocks",
        ));
    }

    let mut seen_ids = BTreeSet::new();
    let mut methods = Vec::new();
    for impl_item in &mut item.items {
        let ImplItem::Fn(method) = impl_item else {
            continue;
        };
        let attrs = parse_script_method_attrs(&method.attrs)?;
        if !attrs.has_attr {
            continue;
        }
        let id = attrs.id.ok_or_else(|| {
            error(
                method.sig.ident.span(),
                "script methods require #[script_method(id = N)]",
            )
        })?;
        if !seen_ids.insert(id) {
            return Err(error(method.sig.ident.span(), "duplicate script method id"));
        }
        let docs = attrs
            .docs
            .clone()
            .or_else(|| docs_from_attrs(&method.attrs));
        methods.push(method_meta(method, attrs, id, docs)?);
        method
            .attrs
            .retain(|attr| !attr.path().is_ident("script_method"));
    }

    let self_ty = item.self_ty.clone();
    let method_tokens = methods.iter().map(method_tokens);
    let registration_tokens = native_method_registration_tokens(&methods);
    Ok(quote! {
        #item

        impl #self_ty {
            #[must_use]
            pub fn vela_native_method_descs() -> ::std::vec::Vec<::vela_engine::NativeMethodDesc> {
                let owner_key = Self::vela_host_type_desc().key;
                let mut methods = ::std::vec::Vec::new();
                #(#method_tokens)*
                methods
            }

            #[must_use]
            pub fn vela_register_native_method_fns(
                builder: ::vela_engine::EngineBuilder,
            ) -> ::vela_engine::EngineBuilder {
                let owner_key = Self::vela_host_type_desc().key;
                #registration_tokens
            }
        }

        impl ::vela_engine::ScriptHostMethodMetadata for #self_ty {
            fn script_host_method_descs() -> ::std::vec::Vec<::vela_engine::NativeMethodDesc> {
                Self::vela_native_method_descs()
            }
        }
    })
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
                _ => return Err(meta.error("unsupported script_method attribute")),
            }
            Ok(())
        })?;
    }
    parsed.permissions.sort();
    parsed.permissions.dedup();
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
    id: u32,
    docs: Option<String>,
) -> Result<MethodMeta> {
    if !method.sig.generics.params.is_empty() {
        return Err(spanned_error(
            &method.sig.generics,
            "#[script_method] does not support generic methods",
        ));
    }
    if method.sig.asyncness.is_some() {
        return Err(spanned_error(
            &method.sig.asyncness,
            "#[script_method] does not support async methods",
        ));
    }

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
        if is_context_param(param) || is_host_execution_param(param) {
            continue;
        }
        if !skipped_receiver && (is_host_ref(&param.ty) || is_host_path(&param.ty)) {
            skipped_receiver = true;
            continue;
        }
        params.push(ParamMeta {
            name: param_name(param),
            ty: param.ty.as_ref().clone(),
            hint: hint_for_type(&param.ty),
        });
    }

    Ok(MethodMeta {
        ident: method.sig.ident.clone(),
        id,
        name: attrs.name.unwrap_or_else(|| method.sig.ident.to_string()),
        effect: attrs.effect.unwrap_or(MethodEffect::Pure),
        docs,
        permissions: attrs.permissions,
        reflect_callable: attrs.reflect_callable,
        params,
        returns: return_hint(&method.sig.output),
        callable_native: has_callable_native_boundary(method),
    })
}

fn is_context_param(param: &PatType) -> bool {
    type_ident(&param.ty).is_some_and(|ident| ident == "NativeCallContext")
}

fn is_host_execution_param(param: &PatType) -> bool {
    type_ident(&param.ty).is_some_and(|ident| ident == "HostExecution")
}

fn is_host_ref(ty: &Type) -> bool {
    type_ident(ty).is_some_and(|ident| ident == "HostRef")
}

fn is_host_path(ty: &Type) -> bool {
    type_ident(ty).is_some_and(|ident| ident == "HostPath")
}

fn has_callable_native_boundary(method: &ImplItemFn) -> bool {
    let mut inputs = method.sig.inputs.iter();
    let Some(FnArg::Typed(receiver)) = inputs.next() else {
        return false;
    };
    let Some(FnArg::Typed(host)) = inputs.next() else {
        return false;
    };
    is_host_path(&receiver.ty) && is_host_execution_param(host)
}

fn param_name(param: &PatType) -> String {
    match param.pat.as_ref() {
        Pat::Ident(ident) => ident.ident.to_string().trim_start_matches('_').to_owned(),
        _ => "arg".to_owned(),
    }
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
    if !matches!(ident.as_str(), "Result" | "VmResult" | "HostResult") {
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
        Some("HostRef") => HintKind::HostOwner,
        Some("Value") => HintKind::Any,
        Some("NativeFunction" | "HostNativeFunction") => HintKind::Function,
        _ => HintKind::Any,
    }
}

fn is_unit_tuple(ty: &Type) -> bool {
    matches!(ty, Type::Tuple(tuple) if tuple.elems.is_empty())
}

fn type_ident(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        Type::Reference(reference) => type_ident(&reference.elem),
        _ => None,
    }
}

fn docs_from_attrs(attrs: &[Attribute]) -> Option<String> {
    let docs = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .filter_map(doc_from_attr)
        .collect::<Vec<_>>();
    (!docs.is_empty()).then(|| docs.join("\n"))
}

fn doc_from_attr(attr: &Attribute) -> Option<String> {
    let syn::Meta::NameValue(name_value) = &attr.meta else {
        return None;
    };
    let syn::Expr::Lit(expr_lit) = &name_value.value else {
        return None;
    };
    let syn::Lit::Str(doc) = &expr_lit.lit else {
        return None;
    };
    Some(doc.value().trim().to_owned())
}

fn method_tokens(method: &MethodMeta) -> TokenStream {
    let desc = method_desc_expr(method);

    quote! {
        methods.push(#desc);
    }
}

fn method_desc_expr(method: &MethodMeta) -> TokenStream {
    let id = method.id;
    let name = &method.name;
    let effect = effect_tokens(method.effect);
    let returns = hint_tokens(method.returns);
    let params = method.params.iter().map(param_tokens);
    let access = access_tokens(method);
    let docs = method
        .docs
        .as_ref()
        .map(|docs| quote! { desc = desc.docs(#docs); });

    quote! {{
        let mut desc = ::vela_engine::NativeMethodDesc::new(
            owner_key.clone(),
            ::vela_common::HostMethodId::new(#id),
            #name,
        )
        .effects(#effect)
        .returns(#returns)
        .access(#access);
        #(
            desc = desc.param(#params);
        )*
        #docs
        desc
    }}
}

fn native_method_registration_tokens(methods: &[MethodMeta]) -> TokenStream {
    let mut builder = quote! { builder };
    for method in methods.iter().filter(|method| method.callable_native) {
        let desc = method_desc_expr(method);
        let args_tuple = args_tuple_tokens(&method.params);
        let ident = &method.ident;
        builder = quote! {
            #builder.register_typed_native_method_fn::<#args_tuple, _>(
                #desc,
                Self::#ident,
            )
        };
    }

    quote! {
        #builder
    }
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

fn param_tokens(param: &ParamMeta) -> TokenStream {
    let name = &param.name;
    let hint = hint_tokens(param.hint);
    quote! { #name, #hint }
}

fn effect_tokens(effect: MethodEffect) -> TokenStream {
    match effect {
        MethodEffect::Pure => quote! { ::vela_engine::EffectSet::pure() },
        MethodEffect::HostRead => quote! { ::vela_engine::EffectSet::host_read() },
        MethodEffect::HostWrite => quote! { ::vela_engine::EffectSet::host_write() },
        MethodEffect::EventEmit => quote! { ::vela_engine::EffectSet::event_emit() },
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
        HintKind::HostOwner => quote! { ::vela_engine::TypeHint::Host(owner_key.clone()) },
        HintKind::Function => quote! { ::vela_engine::TypeHint::Function },
    }
}

fn access_tokens(method: &MethodMeta) -> TokenStream {
    let reflect_callable = method.reflect_callable;
    let permissions = method.permissions.iter().map(|permission| {
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

    use super::expand_result;

    #[test]
    fn rejects_duplicate_method_ids() {
        let error = expand_result(quote! {
            impl Player {
                #[script_method(id = 1)]
                pub fn add_exp(player: HostRef, amount: i64) {}

                #[script_method(id = 1)]
                pub fn set_title(player: HostRef, title: String) {}
            }
        })
        .expect_err("duplicate method IDs should fail macro expansion");

        assert!(error.to_string().contains("duplicate script method id"));
    }

    #[test]
    fn rejects_self_receivers() {
        let error = expand_result(quote! {
            impl Player {
                #[script_method(id = 1)]
                pub fn add_exp(&mut self, amount: i64) {}
            }
        })
        .expect_err("self receiver should fail macro expansion");

        assert!(error.to_string().contains("HostRef receiver parameters"));
    }
}
