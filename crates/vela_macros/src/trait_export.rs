use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{ItemTrait, LitStr, Result, TraitItem, Visibility, parse::Parser, parse2};

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
    let item = parse2::<ItemTrait>(input)?;
    if !matches!(item.vis, Visibility::Public(_)) {
        return Err(syn::Error::new_spanned(
            &item.vis,
            "#[vela::trait_export] requires a public Rust trait",
        ));
    }
    reject_generic_signature(&item.generics, "#[vela::trait_export]")?;
    if !item.supertraits.is_empty() {
        return Err(syn::Error::new_spanned(
            &item.supertraits,
            "Vela protocol export does not infer Rust supertraits",
        ));
    }
    let public_path = parse_path(attr)?;
    let mut classified = Vec::new();
    for trait_item in &item.items {
        let TraitItem::Fn(method) = trait_item else {
            return Err(syn::Error::new_spanned(
                trait_item,
                "exported Vela protocols initially support methods only",
            ));
        };
        reject_generic_signature(&method.sig.generics, "#[vela::trait_export]")?;
        reject_unsafe_signature(&method.sig, "#[vela::trait_export]")?;
        reject_extern_signature(&method.sig, "#[vela::trait_export]")?;
        classified.push((method, classify_method(&method.sig, &BTreeSet::new())?));
    }
    if classified.is_empty() {
        return Err(syn::Error::new_spanned(
            &item,
            "exported Vela protocol must contain at least one method",
        ));
    }
    let docs = docs_from_attrs(&item.attrs);
    let contract =
        emission::protocol_contract(&item.ident, &public_path, docs.as_deref(), &classified);
    Ok(quote! {
        #item
        #contract
    })
}

fn parse_path(attr: TokenStream) -> Result<String> {
    let mut path = None;
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("path") {
            path = Some(parse_qualified_name(
                meta.value()?.parse::<LitStr>()?,
                "trait_export path",
            )?);
            return Ok(());
        }
        Err(meta.error("unsupported trait_export attribute"))
    });
    parser.parse2(attr)?;
    path.ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[vela::trait_export] requires path = \"module::Protocol\"",
        )
    })
}

#[cfg(test)]
mod tests {
    use quote::quote;

    use super::expand_result;

    #[test]
    fn trait_export_uses_stable_vela_protocol_path() {
        let expanded = expand_result(
            quote! { path = "game::Damageable" },
            quote! {
                pub trait Damageable {
                    fn take_damage(&mut self, amount: i64);
                    fn is_alive(&self) -> bool;
                }
            },
        )
        .expect("boundary-safe trait should export");
        let output = expanded.to_string();

        assert!(output.contains("vela_protocol_contract_Damageable"));
        assert!(output.contains("game::Damageable"));
        assert!(output.contains("RustTraitMethod"));
    }
}
