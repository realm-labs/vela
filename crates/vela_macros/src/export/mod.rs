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
    reject_generic_signature(&item.sig.generics, "#[vela_macros::export]")?;
    reject_unsafe_signature(&item.sig, "#[vela_macros::export]")?;
    reject_extern_signature(&item.sig, "#[vela_macros::export]")?;
    if !matches!(item.vis, Visibility::Public(_)) {
        return Err(syn::Error::new_spanned(
            &item.vis,
            "#[vela_macros::export] requires a public Rust function",
        ));
    }

    let attrs = ExportAttrs::parse(attr)?;
    let docs = attrs.docs.clone().or_else(|| docs_from_attrs(&item.attrs));
    let classified = classify_function(&item.sig, &attrs.effects)?;
    let generated = emission::function_contract(&item, &attrs, docs.as_deref(), &classified);
    let adapter = emission::function_value_adapter(&item, &classified);

    Ok(quote! {
        #item
        #generated
        #adapter
    })
}

#[cfg(test)]
mod tests {
    use proc_macro2::TokenStream;
    use quote::quote;
    use rstest::rstest;

    use super::expand_result;

    #[test]
    fn export_accepts_owned_values_and_invocation_local_shared_values() {
        let expanded = expand_result(
            quote! { path = "game::grant_exp" },
            quote! {
                pub fn grant_exp(player: &Player, amount: i64) -> VmResult<()> {
                    Ok(())
                }
            },
        )
        .expect("ordinary export should classify");
        let output = expanded.to_string();

        assert!(output.contains("vela_callable_contract_grant_exp"));
        assert!(output.contains("ReadOnlyValueBorrow"));
        assert!(output.contains("FromScriptArg"));
        assert!(output.contains("catch_export_panic"));
        assert!(!output.contains("lease"));
        assert!(!output.contains("VelaHostBoundary"));
    }

    #[test]
    fn shared_value_adapter_decodes_owned_backing_without_a_host_fallback() {
        let expanded = expand_result(
            quote! { path = "data::inspect" },
            quote! {
                pub fn inspect(record: &Record, name: &str, label: &String) -> String {
                    todo!()
                }
            },
        )
        .expect("shared values use invocation-local decoding")
        .to_string();
        assert!(expanded.contains("as :: vela_engine :: args :: FromScriptArg"));
        assert!(expanded.contains("__vela_arg_1 . as_str ()"));
        assert!(expanded.contains("& __vela_arg_0"));
        assert!(expanded.contains("& __vela_arg_2"));
        assert!(expanded.contains("register_native_fn"));
        for removed in ["vela_host", "lease", "HostBoundary", "with_host"] {
            assert!(!expanded.contains(removed), "unexpected adapter: {removed}");
        }
    }

    #[rstest]
    #[case::boundary_wrapper_parameters(
        quote! { path = "game::grant" },
        quote! { pub fn grant(player: HostRef) {} },
        "ordinary exported signatures cannot mention a Vela boundary wrapper"
    )]
    #[case::effect_removal_spelling(
        quote! { path = "game::grant", effects(pure) },
        quote! { pub fn grant(player: &mut Player) {} },
        "effects(...) only adds effects; `pure` cannot remove declared effects"
    )]
    #[case::task_spawn_effect(
        quote! { path = "game::repair", effects(task_spawn) },
        quote! { pub fn repair() {} },
        "unsupported Vela callable effect"
    )]
    fn export_rejects_invalid_contracts(
        #[case] attr: TokenStream,
        #[case] input: TokenStream,
        #[case] expected: &str,
    ) {
        let error = expand_result(attr, input).expect_err("unsupported export contract");
        assert_eq!(error.to_string(), expected);
    }
}
