use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    Attribute, FnArg, ImplItem, ImplItemFn, ItemImpl, LitBool, LitInt, LitStr, PatType, Result,
    ReturnType, Type, parse2,
};

use crate::attrs::{error, spanned_error};
use crate::signature::{
    docs_from_attrs, param_name, reject_extern_signature, reject_generic_signature,
    reject_script_reference_param, reject_unsafe_signature, reject_unsupported_integer_type,
    type_ident, wrapper_inner_type,
};

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
    reject_generic_signature(&item.generics, "#[script_methods]")?;

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
    let native_registration_tokens = native_method_registration_tokens(&methods);
    let host_method_registration_tokens = script_host_method_registration_tokens(&methods);
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
                #native_registration_tokens
            }

            #[must_use]
            pub fn vela_register_host_methods(
                builder: ::vela_engine::EngineBuilder,
            ) -> ::vela_engine::EngineBuilder {
                let owner_key = Self::vela_host_type_desc().key;
                #host_method_registration_tokens
            }
        }

        impl ::vela_engine::ScriptHostMethodMetadata for #self_ty {
            fn script_host_method_descs() -> ::std::vec::Vec<::vela_engine::NativeMethodDesc> {
                Self::vela_native_method_descs()
            }

            fn register_script_host_methods(
                builder: ::vela_engine::EngineBuilder,
            ) -> ::vela_engine::EngineBuilder {
                Self::vela_register_host_methods(builder)
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
        if is_context_param(param) || is_host_execution_param(param) {
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

fn reject_return_type(output: &ReturnType) -> Result<()> {
    match output {
        ReturnType::Default => Ok(()),
        ReturnType::Type(_, ty) => reject_unsupported_integer_type(ty),
    }
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

fn script_host_method_registration_tokens(methods: &[MethodMeta]) -> TokenStream {
    let mut builder = quote! { builder };
    for method in methods {
        let desc = method_desc_expr(method);
        if method.callable_native {
            let args_tuple = args_tuple_tokens(&method.params);
            let ident = &method.ident;
            builder = quote! {
                #builder.register_typed_native_method_fn::<#args_tuple, _>(
                    #desc,
                    Self::#ident,
                )
            };
        } else {
            builder = quote! {
                #builder.register_host_method_desc(#desc)
            };
        }
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

    #[test]
    fn rejects_impl_where_clauses() {
        let error = expand_result(quote! {
            impl Player
            where
                Player: Clone,
            {
                #[script_method(id = 1)]
                pub fn grant(player: HostRef, amount: i64) {}
            }
        })
        .expect_err("impl where clause should fail macro expansion");

        assert!(error.to_string().contains("where clauses"));
    }

    #[test]
    fn rejects_method_where_clauses() {
        let error = expand_result(quote! {
            impl Player {
                #[script_method(id = 1)]
                pub fn grant(player: HostRef, amount: i64)
                where
                    i64: Copy,
                {
                }
            }
        })
        .expect_err("method where clause should fail macro expansion");

        assert!(error.to_string().contains("where clauses"));
    }

    #[test]
    fn rejects_unsafe_methods() {
        let error = expand_result(quote! {
            impl Player {
                #[script_method(id = 1)]
                pub unsafe fn grant(player: HostRef, amount: i64) {
                }
            }
        })
        .expect_err("unsafe method should fail macro expansion");

        assert!(error.to_string().contains("unsafe functions"));
    }

    #[test]
    fn rejects_extern_methods() {
        let error = expand_result(quote! {
            impl Player {
                #[script_method(id = 1)]
                pub extern "C" fn grant(player: HostRef, amount: i64) {
                }
            }
        })
        .expect_err("extern method should fail macro expansion");

        assert!(error.to_string().contains("extern ABI functions"));
    }

    #[test]
    fn rejects_script_visible_rust_reference_parameters() {
        let error = expand_result(quote! {
            impl Player {
                #[script_method(id = 1)]
                pub fn grant(player: HostRef, amount: &mut i64) {}
            }
        })
        .expect_err("script-visible Rust references should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("script-visible parameters cannot use Rust references")
        );
    }

    #[test]
    fn rejects_unsupported_integer_parameters() {
        let error = expand_result(quote! {
            impl Player {
                #[script_method(id = 1)]
                pub fn grant(player: HostRef, amount: Option<u128>) {}
            }
        })
        .expect_err("unsupported integer parameter should fail macro expansion");

        assert!(error.to_string().contains("u128"));
        assert!(
            error
                .to_string()
                .contains("script-visible native signatures do not support")
        );
    }

    #[test]
    fn rejects_unsupported_integer_parameters_inside_arrays() {
        let error = expand_result(quote! {
            impl Player {
                #[script_method(id = 1)]
                pub fn grant(player: HostRef, amounts: [usize; 2]) {}
            }
        })
        .expect_err("unsupported array integer parameter should fail macro expansion");

        assert!(error.to_string().contains("usize"));
    }

    #[test]
    fn rejects_unsupported_integer_returns() {
        let error = expand_result(quote! {
            impl Player {
                #[script_method(id = 1)]
                pub fn grant(player: HostRef) -> isize {
                    1
                }
            }
        })
        .expect_err("unsupported integer return should fail macro expansion");

        assert!(error.to_string().contains("isize"));
    }

    #[test]
    fn infers_fixed_array_signature_hints() {
        let tokens = expand_result(quote! {
            impl Player {
                #[script_method(id = 1)]
                pub fn weights(player: HostRef, values: [i64; 3]) -> [i64; 3] {
                    values
                }
            }
        })
        .expect("fixed array method should expand")
        .to_string();

        assert!(tokens.contains("TypeHint :: Array"));
    }
}
