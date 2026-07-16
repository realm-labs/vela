use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::ext::IdentExt;
use syn::parse::Parser;
use syn::{
    AngleBracketedGenericArguments, Attribute, FnArg, GenericArgument, ImplItemFn, ItemFn, LitInt,
    LitStr, Pat, PathArguments, Result, ReturnType, Type, Visibility, parse2,
};

use crate::attrs::parse_qualified_name;
use crate::signature::{
    docs_from_attrs, reject_extern_signature, reject_generic_signature, reject_unsafe_signature,
};

use super::attrs::ExportAttrs;
use super::emission;
use super::signature::{EffectName, ParameterMode, classify_function};

pub(crate) struct ReplaceableAttrs {
    path: String,
    authority: syn::Ident,
    index: usize,
    pub(crate) effects: BTreeSet<EffectName>,
}

pub(crate) struct RewrittenMethod {
    pub(crate) fallback: ImplItemFn,
    pub(crate) generated: TokenStream,
}

pub(crate) fn expand(attr: TokenStream, input: TokenStream) -> TokenStream {
    match expand_result(attr, input) {
        Ok(tokens) => tokens,
        Err(error) => error.to_compile_error(),
    }
}

fn expand_result(attr: TokenStream, input: TokenStream) -> Result<TokenStream> {
    let mut item = parse2::<ItemFn>(input)?;
    reject_generic_signature(&item.sig.generics, "#[vela::replaceable]")?;
    reject_unsafe_signature(&item.sig, "#[vela::replaceable]")?;
    reject_extern_signature(&item.sig, "#[vela::replaceable]")?;
    if !matches!(item.vis, Visibility::Public(_)) {
        return Err(syn::Error::new_spanned(
            &item.vis,
            "#[vela::replaceable] requires a public Rust function",
        ));
    }
    let attrs = parse_attrs(attr)?;
    let return_type = vm_result_ok_type(&item.sig.output)?;
    let authority = authority_parameter(&item, &attrs.authority)?;
    let classified = classify_function(&item.sig, &attrs.effects)?;
    if classified
        .parameters
        .iter()
        .any(|parameter| parameter.mode == ParameterMode::HiddenContext)
    {
        return Err(syn::Error::new_spanned(
            &item.sig,
            "replaceable entries derive dispatch from an ordinary authority parameter, not NativeCallContext",
        ));
    }

    let function_ident = item.sig.ident.clone();
    let fallback_ident = format_ident!("__vela_rust_{function_ident}");
    let slot_ident = format_ident!(
        "VELA_INTERCEPT_SLOT_{}",
        function_ident.to_string().to_uppercase()
    );
    let descriptor_ident = format_ident!("vela_replaceable_slot_{function_ident}");
    let contract_attrs = ExportAttrs {
        path: attrs.path,
        effects: attrs.effects,
        docs: None,
    };
    let docs = docs_from_attrs(&item.attrs);
    let contract =
        emission::function_contract(&item, &contract_attrs, docs.as_deref(), &classified);
    let contract_ident = format_ident!("vela_callable_contract_{function_ident}");
    let index = attrs.index;

    let call_arguments = item
        .sig
        .inputs
        .iter()
        .map(|argument| match argument {
            FnArg::Typed(parameter) => match parameter.pat.as_ref() {
                Pat::Ident(ident) => Ok(ident.ident.clone()),
                pattern => Err(syn::Error::new_spanned(
                    pattern,
                    "replaceable parameters require identifier patterns",
                )),
            },
            FnArg::Receiver(receiver) => Err(syn::Error::new_spanned(
                receiver,
                "use #[vela::replaceable] on a free function; host method macros may delegate to it",
            )),
        })
        .collect::<Result<Vec<_>>>()?;
    let arg_bindings = classified
        .parameters
        .iter()
        .zip(&call_arguments)
        .map(|(parameter, argument)| argument_binding(parameter.mode, &quote! { #argument }))
        .collect::<Vec<_>>();

    let target_lookup = quote! {
        <_ as ::vela_engine::dispatch::DispatchAuthority>::vela_dispatch_root(
            &*#authority,
        )
        .target(#slot_ident)
    };
    let override_call = if item.sig.asyncness.is_some() {
        quote! {
            let __vela_invocation =
                <_ as ::vela_engine::dispatch::DispatchAuthority>::vela_dispatch_root(
                    &*#authority,
                )
                .invocation();
            let mut __vela_args = ::vela_engine::runtime::CallArgs::new();
            #(#arg_bindings)*
            return __vela_invocation
                .call_async::<#return_type>(__vela_target, __vela_args)
                .await;
        }
    } else {
        quote! {
            let __vela_invocation =
                <_ as ::vela_engine::dispatch::DispatchAuthority>::vela_dispatch_root(
                    &*#authority,
                )
                .invocation();
            let mut __vela_args = ::vela_engine::runtime::CallArgs::new();
            #(#arg_bindings)*
            return __vela_invocation.call::<#return_type>(__vela_target, __vela_args);
        }
    };
    let fallback_call = if item.sig.asyncness.is_some() {
        quote! { #fallback_ident(#(#call_arguments),*).await }
    } else {
        quote! { #fallback_ident(#(#call_arguments),*) }
    };

    let mut fallback = item.clone();
    fallback.sig.ident = fallback_ident;
    fallback.vis = Visibility::Inherited;
    *item.block = syn::parse_quote!({
        if let Some(__vela_target) = #target_lookup {
            #override_call
        }
        #fallback_call
    });

    Ok(quote! {
        #fallback
        #item
        #contract

        #[doc(hidden)]
        pub const #slot_ident: ::vela_common::InterceptSlotIndex =
            ::vela_common::InterceptSlotIndex::new(#index);

        #[doc(hidden)]
        #[must_use]
        pub fn #descriptor_ident() -> ::vela_engine::dispatch::ReplaceableSlotDescriptor {
            ::vela_engine::dispatch::ReplaceableSlotDescriptor::new(
                #index,
                #contract_ident(),
            )
        }
    })
}

pub(crate) fn take_method_attrs(method: &mut ImplItemFn) -> Result<Option<ReplaceableAttrs>> {
    let mut found = None;
    let mut retained = Vec::with_capacity(method.attrs.len());
    for attribute in std::mem::take(&mut method.attrs) {
        if !is_replaceable_attr(&attribute) {
            retained.push(attribute);
            continue;
        }
        if found.is_some() {
            return Err(syn::Error::new_spanned(
                attribute,
                "method has duplicate `replaceable` attributes",
            ));
        }
        let syn::Meta::List(list) = &attribute.meta else {
            return Err(syn::Error::new_spanned(
                attribute,
                "replaceable method attribute requires arguments",
            ));
        };
        found = Some(parse_attrs(list.tokens.clone())?);
    }
    method.attrs = retained;
    Ok(found)
}

pub(crate) fn rewrite_method(
    method: &mut ImplItemFn,
    attrs: &ReplaceableAttrs,
    classified: &crate::export::signature::ClassifiedSignature,
) -> Result<RewrittenMethod> {
    if !matches!(method.vis, Visibility::Public(_)) {
        return Err(syn::Error::new_spanned(
            &method.vis,
            "replaceable methods must be public",
        ));
    }
    let return_type = vm_result_ok_type(&method.sig.output)?;
    let authority = method_authority(method, &attrs.authority)?;
    let method_ident = method.sig.ident.clone();
    let fallback_ident = format_ident!("__vela_rust_{method_ident}");
    let slot_ident = format_ident!(
        "VELA_INTERCEPT_SLOT_{}",
        method_ident.to_string().to_uppercase()
    );
    let descriptor_ident = format_ident!("vela_replaceable_slot_{method_ident}");
    let contract_ident = format_ident!("vela_callable_contract_{method_ident}");
    let index = attrs.index;

    let mut call_arguments = Vec::with_capacity(method.sig.inputs.len());
    for argument in &method.sig.inputs {
        match argument {
            FnArg::Receiver(_) => call_arguments.push(quote! { self }),
            FnArg::Typed(parameter) => match parameter.pat.as_ref() {
                Pat::Ident(ident) => {
                    let ident = &ident.ident;
                    call_arguments.push(quote! { #ident });
                }
                pattern => {
                    return Err(syn::Error::new_spanned(
                        pattern,
                        "replaceable parameters require identifier patterns",
                    ));
                }
            },
        }
    }
    let arg_bindings = classified
        .parameters
        .iter()
        .zip(&call_arguments)
        .map(|(parameter, argument)| argument_binding(parameter.mode, argument))
        .collect::<Vec<_>>();
    let target_lookup = quote! {
        <_ as ::vela_engine::dispatch::DispatchAuthority>::vela_dispatch_root(
            #authority,
        )
        .target(Self::#slot_ident)
    };
    let override_call = if method.sig.asyncness.is_some() {
        quote! {
            let __vela_invocation =
                <_ as ::vela_engine::dispatch::DispatchAuthority>::vela_dispatch_root(
                    #authority,
                )
                .invocation();
            let mut __vela_args = ::vela_engine::runtime::CallArgs::new();
            #(#arg_bindings)*
            return __vela_invocation
                .call_async::<#return_type>(__vela_target, __vela_args)
                .await;
        }
    } else {
        quote! {
            let __vela_invocation =
                <_ as ::vela_engine::dispatch::DispatchAuthority>::vela_dispatch_root(
                    #authority,
                )
                .invocation();
            let mut __vela_args = ::vela_engine::runtime::CallArgs::new();
            #(#arg_bindings)*
            return __vela_invocation.call::<#return_type>(__vela_target, __vela_args);
        }
    };
    let ordinary_arguments = method
        .sig
        .inputs
        .iter()
        .filter_map(|argument| match argument {
            FnArg::Receiver(_) => None,
            FnArg::Typed(parameter) => match parameter.pat.as_ref() {
                Pat::Ident(ident) => Some(ident.ident.clone()),
                _ => None,
            },
        });
    let fallback_call = if method.sig.asyncness.is_some() {
        quote! { self.#fallback_ident(#(#ordinary_arguments),*).await }
    } else {
        quote! { self.#fallback_ident(#(#ordinary_arguments),*) }
    };

    let mut fallback = method.clone();
    fallback.sig.ident = fallback_ident;
    fallback.vis = Visibility::Inherited;
    method.block = syn::parse_quote!({
        if let Some(__vela_target) = #target_lookup {
            #override_call
        }
        #fallback_call
    });
    let generated = quote! {
        #[doc(hidden)]
        pub const #slot_ident: ::vela_common::InterceptSlotIndex =
            ::vela_common::InterceptSlotIndex::new(#index);

        #[doc(hidden)]
        #[must_use]
        pub fn #descriptor_ident() -> ::vela_engine::dispatch::ReplaceableSlotDescriptor {
            ::vela_engine::dispatch::ReplaceableSlotDescriptor::new(
                #index,
                Self::#contract_ident(),
            )
        }
    };
    Ok(RewrittenMethod {
        fallback,
        generated,
    })
}

fn is_replaceable_attr(attribute: &Attribute) -> bool {
    attribute
        .path()
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "replaceable")
}

fn method_authority(method: &ImplItemFn, authority: &syn::Ident) -> Result<TokenStream> {
    if authority == "self" {
        if method
            .sig
            .inputs
            .iter()
            .any(|input| matches!(input, FnArg::Receiver(_)))
        {
            return Ok(quote! { &*self });
        }
        return Err(syn::Error::new_spanned(
            &method.sig,
            "dispatch authority `self` requires a method receiver",
        ));
    }
    authority_parameter_in_inputs(&method.sig.inputs, authority)?;
    Ok(quote! { &*#authority })
}

fn argument_binding(mode: ParameterMode, argument: &TokenStream) -> TokenStream {
    match mode {
        ParameterMode::Value | ParameterMode::ReadOnlyValueBorrow => quote! {
            __vela_args.push(
                ::vela_engine::args::IntoScriptArg::into_script_arg(#argument),
            );
        },
        ParameterMode::SharedHost => quote! {
            __vela_args.push_positional_host_ref(#argument);
        },
        ParameterMode::ExclusiveHost => quote! {
            __vela_args.push_positional_host_mut(#argument);
        },
        ParameterMode::HiddenContext => unreachable!("replaceable contexts were rejected"),
    }
}

fn parse_attrs(tokens: TokenStream) -> Result<ReplaceableAttrs> {
    let mut path = None;
    let mut authority = None;
    let mut index = None;
    let mut effects = BTreeSet::new();
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("path") {
            path = Some(parse_qualified_name(
                meta.value()?.parse::<LitStr>()?,
                "replaceable path",
            )?);
            return Ok(());
        }
        if meta.path.is_ident("authority") {
            let value = meta.value()?.parse::<LitStr>()?;
            authority = Some(value.parse_with(syn::Ident::parse_any)?);
            return Ok(());
        }
        if meta.path.is_ident("index") {
            index = Some(meta.value()?.parse::<LitInt>()?.base10_parse::<usize>()?);
            return Ok(());
        }
        if meta.path.is_ident("effects") {
            return meta.parse_nested_meta(|effect| {
                let Some(ident) = effect.path.get_ident() else {
                    return Err(effect.error("effect must be an identifier"));
                };
                let parsed = EffectName::parse(ident)?;
                if parsed == EffectName::Pure {
                    return Err(effect.error("effects(...) cannot remove inferred effects"));
                }
                effects.insert(parsed);
                Ok(())
            });
        }
        Err(meta.error("unsupported replaceable attribute"))
    });
    parser.parse2(tokens)?;
    Ok(ReplaceableAttrs {
        path: path
            .ok_or_else(|| syn::Error::new(proc_macro2::Span::call_site(), "missing path"))?,
        authority: authority
            .ok_or_else(|| syn::Error::new(proc_macro2::Span::call_site(), "missing authority"))?,
        index: index
            .ok_or_else(|| syn::Error::new(proc_macro2::Span::call_site(), "missing index"))?,
        effects,
    })
}

fn authority_parameter(item: &ItemFn, authority: &syn::Ident) -> Result<syn::Ident> {
    authority_parameter_in_inputs(&item.sig.inputs, authority)?;
    Ok(authority.clone())
}

fn authority_parameter_in_inputs(
    inputs: &syn::punctuated::Punctuated<FnArg, syn::Token![,]>,
    authority: &syn::Ident,
) -> Result<()> {
    for input in inputs {
        let FnArg::Typed(parameter) = input else {
            continue;
        };
        let Pat::Ident(pattern) = parameter.pat.as_ref() else {
            continue;
        };
        if pattern.ident == *authority {
            if !matches!(parameter.ty.as_ref(), Type::Reference(_)) {
                return Err(syn::Error::new_spanned(
                    &parameter.ty,
                    "dispatch authority must be passed by reference",
                ));
            }
            return Ok(());
        }
    }
    Err(syn::Error::new_spanned(
        inputs,
        format!("dispatch authority parameter `{authority}` does not exist"),
    ))
}

fn vm_result_ok_type(output: &ReturnType) -> Result<Type> {
    let ReturnType::Type(_, ty) = output else {
        return Err(syn::Error::new_spanned(
            output,
            "replaceable entries must return VmResult<T>",
        ));
    };
    let Type::Path(path) = ty.as_ref() else {
        return Err(syn::Error::new_spanned(ty, "expected VmResult<T>"));
    };
    let Some(segment) = path.path.segments.last() else {
        return Err(syn::Error::new_spanned(ty, "expected VmResult<T>"));
    };
    if segment.ident != "VmResult" {
        return Err(syn::Error::new_spanned(ty, "expected VmResult<T>"));
    }
    let PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }) =
        &segment.arguments
    else {
        return Err(syn::Error::new_spanned(ty, "expected VmResult<T>"));
    };
    let [GenericArgument::Type(ok)] = args.iter().collect::<Vec<_>>().as_slice() else {
        return Err(syn::Error::new_spanned(ty, "expected VmResult<T>"));
    };
    Ok((*ok).clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn expansion_keeps_public_shape_and_private_fallback() {
        let expanded = expand_result(
            quote! { path = "host::game::level", authority = "context", index = 0 },
            quote! {
                pub fn level(context: &mut ActorContext, player: &mut Player, amount: i64)
                    -> VmResult<i64>
                {
                    player.level += amount;
                    Ok(player.level)
                }
            },
        )
        .expect("replaceable function");
        let output = expanded.to_string();
        assert!(output.contains("fn __vela_rust_level"));
        assert!(output.contains("pub fn level"));
        assert!(output.contains("VELA_INTERCEPT_SLOT_LEVEL"));
        assert!(output.contains("push_host_mut"));
        assert!(output.contains("__vela_rust_level (context , player , amount)"));
    }
}
