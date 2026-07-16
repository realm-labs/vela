use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{ImplItem, ItemImpl, LitStr, Result, Visibility, parse::Parser, parse2};

use crate::attrs::parse_qualified_name;
use crate::export::emission;
use crate::export::signature::classify_method;
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
    let mut item = parse2::<ItemImpl>(input)?;
    if item.trait_.is_some() {
        return Err(syn::Error::new_spanned(
            &item,
            "trait implementation export will be enabled with stable Vela protocol mapping",
        ));
    }
    reject_generic_signature(&item.generics, "#[vela::methods]")?;
    let owner_path = parse_owner_path(attr, &item)?;
    let mut generated = Vec::new();
    for impl_item in &item.items {
        let ImplItem::Fn(method) = impl_item else {
            continue;
        };
        if !matches!(method.vis, Visibility::Public(_)) {
            continue;
        }
        reject_generic_signature(&method.sig.generics, "#[vela::methods]")?;
        reject_unsafe_signature(&method.sig, "#[vela::methods]")?;
        reject_extern_signature(&method.sig, "#[vela::methods]")?;
        let signature = classify_method(&method.sig, &BTreeSet::new())?;
        let docs = docs_from_attrs(&method.attrs);
        generated.push(emission::method_contract(
            method,
            &owner_path,
            docs.as_deref(),
            &signature,
        ));
    }
    if generated.is_empty() {
        return Err(syn::Error::new_spanned(
            &item,
            "#[vela::methods] requires at least one supported public method",
        ));
    }
    for tokens in generated {
        item.items.push(parse2(tokens)?);
    }
    Ok(quote! { #item })
}

fn parse_owner_path(attr: TokenStream, item: &ItemImpl) -> Result<String> {
    let mut configured = None;
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("path") {
            configured = Some(parse_qualified_name(
                meta.value()?.parse::<LitStr>()?,
                "methods path",
            )?);
            return Ok(());
        }
        Err(meta.error("unsupported methods attribute"))
    });
    parser.parse2(attr)?;
    if let Some(configured) = configured {
        return Ok(configured);
    }
    let syn::Type::Path(path) = item.self_ty.as_ref() else {
        return Err(syn::Error::new_spanned(
            &item.self_ty,
            "methods owner must have a stable named type path",
        ));
    };
    Ok(path.path.to_token_stream().to_string().replace(' ', ""))
}

#[cfg(test)]
mod tests {
    use quote::quote;

    use super::expand_result;

    #[test]
    fn methods_uses_shared_classifier_for_receivers() {
        let expanded = expand_result(
            quote! { path = "game::Player" },
            quote! {
                impl Player {
                    pub fn level(&self) -> i64 { self.level }
                    pub fn grant(&mut self, amount: i64) { self.level += amount; }
                    fn helper(&self) {}
                }
            },
        )
        .expect("method group classifies");
        let output = expanded.to_string();

        assert!(output.contains("vela_callable_contract_level"));
        assert!(output.contains("vela_callable_contract_grant"));
        assert!(!output.contains("vela_callable_contract_helper"));
        assert!(output.contains("host_read"));
        assert!(output.contains("host_write"));
    }
}
