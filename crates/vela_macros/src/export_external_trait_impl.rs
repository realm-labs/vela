use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{ImplItemFn, LitStr, Path, Result, Token, TraitItemFn, Type, TypePath, parse2};

use crate::export::emission;
use crate::export::signature::{ClassifiedSignature, classify_method};
use crate::signature::{
    reject_extern_signature, reject_generic_signature, reject_unsafe_signature,
};

struct ExternalTraitImpl {
    self_ty: TypePath,
    trait_path: Path,
    protocol_path: LitStr,
    methods: Vec<TraitItemFn>,
}

impl Parse for ExternalTraitImpl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        input.parse::<Token![type]>()?;
        let self_ty = input.parse()?;
        input.parse::<Token![;]>()?;
        input.parse::<Token![trait]>()?;
        let trait_path = input.parse()?;
        input.parse::<Token![as]>()?;
        let protocol_path: LitStr = input.parse()?;
        input.parse::<Token![;]>()?;
        let mut methods = Vec::new();
        while !input.is_empty() {
            methods.push(input.parse()?);
        }
        if methods.is_empty() {
            return Err(syn::Error::new(
                protocol_path.span(),
                "external trait export requires at least one selected method signature",
            ));
        }
        Ok(Self {
            self_ty,
            trait_path,
            protocol_path,
            methods,
        })
    }
}

pub(crate) fn expand(input: TokenStream) -> TokenStream {
    match expand_result(input) {
        Ok(tokens) => tokens,
        Err(error) => error.to_compile_error(),
    }
}

fn expand_result(input: TokenStream) -> Result<TokenStream> {
    let declaration = parse2::<ExternalTraitImpl>(input)?;
    let self_ty = Type::Path(declaration.self_ty.clone());
    let trait_ident = &declaration
        .trait_path
        .segments
        .last()
        .expect("trait paths contain a segment")
        .ident;
    let type_ident = &declaration
        .self_ty
        .path
        .segments
        .last()
        .expect("type paths contain a segment")
        .ident;
    let adapter_ident = format_ident!("VelaExternal{type_ident}{trait_ident}Exports");
    let protocol_path = declaration.protocol_path.value();
    let mut classified = Vec::<(TraitItemFn, ClassifiedSignature)>::new();
    let mut generated = Vec::new();
    let mut contract_functions = Vec::new();
    let mut registration_functions = Vec::new();
    for method in declaration.methods {
        reject_generic_signature(&method.sig.generics, "export_external_trait_impl!")?;
        reject_unsafe_signature(&method.sig, "export_external_trait_impl!")?;
        reject_extern_signature(&method.sig, "export_external_trait_impl!")?;
        let signature = classify_method(&method.sig, &Default::default())?;
        let rust_signature = method.sig.clone();
        let impl_method: ImplItemFn = syn::parse_quote! {
            pub #rust_signature { ::core::unreachable!() }
        };
        generated.push(emission::method_contract(
            &impl_method,
            &self_ty,
            &protocol_path,
            None,
            &signature,
        ));
        generated.push(
            emission::method_adapter(
                &impl_method,
                &self_ty,
                Some(&declaration.trait_path),
                &signature,
            )
            .ok_or_else(|| {
                syn::Error::new_spanned(
                    &method.sig,
                    "this external trait method requires the async or borrowed-return adapter batch",
                )
            })?,
        );
        contract_functions.push(format_ident!("vela_callable_contract_{}", method.sig.ident));
        registration_functions.push(format_ident!("vela_register_export_{}", method.sig.ident));
        classified.push((method, signature));
    }
    let protocol_methods = classified
        .iter()
        .map(|(method, signature)| (method, signature.clone()))
        .collect::<Vec<_>>();
    let protocol_contract =
        emission::protocol_contract(trait_ident, &protocol_path, None, &protocol_methods);
    let protocol_contract_ident = format_ident!("vela_protocol_contract_{trait_ident}");

    Ok(quote! {
        pub struct #adapter_ident;

        impl #adapter_ident {
            #(#generated)*
            #protocol_contract

            #[must_use]
            pub fn vela_exports() -> ::vela_engine::interop::ExportBundle {
                ::vela_engine::interop::ExportBundle::with_protocols(
                    vec![#(Self::#contract_functions()),*],
                    vec![Self::#protocol_contract_ident()],
                    |builder| {
                        let builder = builder;
                        #(let builder = Self::#registration_functions(builder);)*
                        builder
                    },
                )
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use quote::quote;

    use super::expand_result;

    #[test]
    fn declaration_only_adapter_emits_ufcs_thunks_and_explicit_bundle() {
        let expanded = expand_result(quote! {
            type external_game::Player;
            trait external_game::Damageable as "game::Damageable";
            fn take_damage(&mut self, amount: i64);
            fn is_alive(&self) -> bool;
        })
        .expect("external trait declaration should expand")
        .to_string();

        assert!(expanded.contains("VelaExternalPlayerDamageableExports"));
        assert!(expanded.contains("external_game :: Damageable"));
        assert!(expanded.contains("vela_exports"));
        assert!(!expanded.contains("impl external_game :: Damageable for"));
    }
}
