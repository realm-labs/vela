pub(crate) mod attrs;
pub(crate) mod emission;
pub(crate) mod signature;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{ItemFn, Result, Visibility, parse2};

use crate::signature::{
    docs_from_attrs, reject_extern_signature, reject_generic_signature, reject_unsafe_signature,
};

use self::attrs::ExportAttrs;
use self::signature::classify_function;

pub(crate) fn expand(attr: TokenStream, input: TokenStream) -> TokenStream {
    match expand_result(attr, input) {
        Ok(tokens) => tokens,
        Err(error) => error.to_compile_error(),
    }
}

fn expand_result(attr: TokenStream, input: TokenStream) -> Result<TokenStream> {
    let item = parse2::<ItemFn>(input)?;
    reject_generic_signature(&item.sig.generics, "#[vela::export]")?;
    reject_unsafe_signature(&item.sig, "#[vela::export]")?;
    reject_extern_signature(&item.sig, "#[vela::export]")?;
    if !matches!(item.vis, Visibility::Public(_)) {
        return Err(syn::Error::new_spanned(
            &item.vis,
            "#[vela::export] requires a public Rust function",
        ));
    }

    let attrs = ExportAttrs::parse(attr)?;
    let docs = attrs.docs.clone().or_else(|| docs_from_attrs(&item.attrs));
    let classified = classify_function(&item.sig, &attrs.effects)?;
    let generated = emission::function_contract(&item, &attrs, docs.as_deref(), &classified);

    Ok(quote! {
        #item
        #generated
    })
}

#[cfg(test)]
mod tests {
    use quote::quote;

    use super::expand_result;

    #[test]
    fn export_accepts_ordinary_value_and_host_parameters() {
        let expanded = expand_result(
            quote! { path = "game::grant_exp" },
            quote! {
                pub fn grant_exp(player: &mut Player, amount: i64) -> VmResult<()> {
                    Ok(())
                }
            },
        )
        .expect("ordinary export should classify");
        let output = expanded.to_string();

        assert!(output.contains("vela_callable_contract_grant_exp"));
        assert!(output.contains("ExclusiveHost"));
        assert!(output.contains("host_write"));
    }

    #[test]
    fn export_rejects_boundary_wrapper_parameters() {
        let error = expand_result(
            quote! { path = "game::grant" },
            quote! {
                pub fn grant(player: HostRef) {}
            },
        )
        .expect_err("ordinary exports must hide HostRef");

        assert!(error.to_string().contains("boundary wrapper"));
    }

    #[test]
    fn export_rejects_effect_removal_spelling() {
        let error = expand_result(
            quote! { path = "game::grant", effects(pure) },
            quote! {
                pub fn grant(player: &mut Player) {}
            },
        )
        .expect_err("pure cannot be an additive effect");

        assert!(error.to_string().contains("effects(...) only adds"));
    }
}
