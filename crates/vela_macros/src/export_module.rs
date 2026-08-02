use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Attribute, Item, ItemMod, LitStr, Meta, Result, Visibility, parse::Parser, parse2};

use crate::attrs::parse_qualified_name;
use crate::export::attrs::ExportAttrs;
use crate::export::emission;
use crate::export::signature::classify_function;
use crate::signature::{
    docs_from_attrs, reject_extern_signature, reject_generic_signature, reject_unsafe_signature,
};

pub(crate) fn expand(attr: TokenStream, input: TokenStream) -> TokenStream {
    match expand_result(attr, input) {
        Ok(tokens) => tokens,
        Err(error) => error.to_compile_error(),
    }
}

fn expand_result(attr: TokenStream, input: TokenStream) -> Result<TokenStream> {
    let mut module = parse2::<ItemMod>(input)?;
    let prefix = parse_prefix(attr)?;
    let Some((_, items)) = module.content.as_mut() else {
        return Err(syn::Error::new_spanned(
            &module,
            "#[vela_macros::export_module] requires an inline module",
        ));
    };
    let mut generated = Vec::new();
    let mut contract_functions = Vec::new();
    let mut registration_functions = Vec::new();
    for item in items.iter_mut() {
        let Item::Fn(function) = item else {
            continue;
        };
        if !matches!(function.vis, Visibility::Public(_)) {
            continue;
        }
        reject_generic_signature(&function.sig.generics, "#[vela_macros::export_module]")?;
        reject_unsafe_signature(&function.sig, "#[vela_macros::export_module]")?;
        reject_extern_signature(&function.sig, "#[vela_macros::export_module]")?;
        let default_path = format!("{prefix}::{}", function.sig.ident);
        let attrs = take_export_attrs(&mut function.attrs, default_path)?;
        let docs = attrs
            .docs
            .clone()
            .or_else(|| docs_from_attrs(&function.attrs));
        let classified = classify_function(&function.sig, &attrs.effects)?;
        generated.push(emission::function_contract(
            function,
            &attrs,
            docs.as_deref(),
            &classified,
        ));
        let adapter = emission::function_value_adapter(function, &classified).ok_or_else(|| {
            syn::Error::new_spanned(
                &function.sig,
                "this export-module signature requires the direct host/async adapter batch",
            )
        })?;
        generated.push(adapter);
        contract_functions.push(format_ident!(
            "vela_callable_contract_{}",
            function.sig.ident
        ));
        registration_functions.push(format_ident!("vela_register_export_{}", function.sig.ident));
    }
    if contract_functions.is_empty() {
        return Err(syn::Error::new_spanned(
            &module,
            "#[vela_macros::export_module] requires at least one supported public function",
        ));
    }
    for generated in generated {
        items.push(Item::Verbatim(generated));
    }
    items.push(Item::Verbatim(quote! {
        #[doc(hidden)]
        #[must_use]
        pub fn vela_module() -> ::vela_engine::registration::ModuleRegistration {
            ::vela_engine::registration::ModuleRegistration::new(
                vec![#(#contract_functions()),*],
                |builder| {
                    let builder = builder;
                    #(let builder = #registration_functions(builder);)*
                    builder
                },
            )
        }
    }));
    Ok(quote! { #module })
}

fn parse_prefix(attr: TokenStream) -> Result<String> {
    let mut path = None;
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("path") {
            path = Some(parse_qualified_name(
                meta.value()?.parse::<LitStr>()?,
                "export_module path",
            )?);
            return Ok(());
        }
        Err(meta.error("unsupported export_module attribute"))
    });
    parser.parse2(attr)?;
    path.ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[vela_macros::export_module] requires path = \"module\"",
        )
    })
}

fn take_export_attrs(attrs: &mut Vec<Attribute>, default_path: String) -> Result<ExportAttrs> {
    let positions = attrs
        .iter()
        .enumerate()
        .filter_map(|(index, attr)| is_export_attr(attr).then_some(index))
        .collect::<Vec<_>>();
    if positions.len() > 1 {
        return Err(syn::Error::new_spanned(
            &attrs[positions[1]],
            "duplicate export attribute in export module",
        ));
    }
    let Some(position) = positions.first().copied() else {
        return ExportAttrs::parse_with_default(TokenStream::new(), Some(default_path));
    };
    let attr = attrs.remove(position);
    let tokens = match attr.meta {
        Meta::Path(_) => TokenStream::new(),
        Meta::List(list) => list.tokens,
        Meta::NameValue(_) => {
            return Err(syn::Error::new_spanned(
                attr,
                "export overrides use #[vela(...)]",
            ));
        }
    };
    ExportAttrs::parse_with_default(tokens, Some(default_path))
}

fn is_export_attr(attr: &Attribute) -> bool {
    attr.path().is_ident("vela")
}

#[cfg(test)]
mod tests {
    use quote::quote;

    use super::expand_result;

    #[test]
    fn module_exports_public_functions_and_keeps_private_helpers_private() {
        let expanded = expand_result(
            quote! { path = "game" },
            quote! {
                mod exports {
                    pub fn normalize(amount: i64) -> i64 { amount.max(0) }

                    #[vela(effects(random))]
                    pub fn roll() -> i64 { 4 }

                    fn helper() -> i64 { 1 }
                }
            },
        )
        .expect("export module should expand");
        let output = expanded.to_string();

        assert!(output.contains("game::normalize"));
        assert!(output.contains("game::roll"));
        assert!(output.contains("vela_module"));
        assert!(!output.contains("vela_callable_contract_helper"));
    }
}
