use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{ImplItem, ItemImpl, LitBool, LitStr, Result, Visibility, parse::Parser, parse2};

use crate::attrs::parse_qualified_name;
use crate::export::emission;
use crate::export::signature::{classify_method, classify_method_with_host_collection_returns};
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
    reject_generic_signature(&item.generics, "#[vela::methods]")?;
    let owner_path = parse_owner_path(attr, &item)?;
    let trait_path = item.trait_.as_ref().map(|(_, path, _)| path);
    let mut generated = Vec::new();
    let mut contract_functions = Vec::new();
    let mut registration_functions = Vec::new();
    for impl_item in &mut item.items {
        let ImplItem::Fn(method) = impl_item else {
            continue;
        };
        if trait_path.is_none() && !matches!(method.vis, Visibility::Public(_)) {
            continue;
        }
        reject_generic_signature(&method.sig.generics, "#[vela::methods]")?;
        reject_unsafe_signature(&method.sig, "#[vela::methods]")?;
        reject_extern_signature(&method.sig, "#[vela::methods]")?;
        let method_attrs = take_method_attrs(method)?;
        let additional_effects = BTreeSet::new();
        let signature = if method_attrs.host_collection {
            classify_method_with_host_collection_returns(&method.sig, &additional_effects)?
        } else {
            classify_method(&method.sig, &additional_effects)?
        };
        let docs = docs_from_attrs(&method.attrs);
        let public_name = method_attrs
            .name
            .unwrap_or_else(|| method.sig.ident.to_string());
        generated.push(emission::method_contract(
            method,
            &item.self_ty,
            &owner_path,
            &public_name,
            docs.as_deref(),
            method_attrs.reflect_callable,
            &signature,
        ));
        generated.push(
            emission::method_adapter(method, &item.self_ty, trait_path, &signature).ok_or_else(
                || {
                    syn::Error::new_spanned(
                        &method.sig,
                        "this exported method requires the async or borrowed-return adapter batch",
                    )
                },
            )?,
        );
        contract_functions.push(quote::format_ident!(
            "vela_callable_contract_{}",
            method.sig.ident
        ));
        registration_functions.push(quote::format_ident!(
            "vela_register_export_{}",
            method.sig.ident
        ));
    }
    if generated.is_empty() {
        return Err(syn::Error::new_spanned(
            &item,
            "#[vela::methods] requires at least one supported public method",
        ));
    }
    let bundle = if let Some(trait_path) = trait_path {
        let trait_ident = &trait_path
            .segments
            .last()
            .expect("trait paths contain at least one segment")
            .ident;
        let bundle_ident = quote::format_ident!("vela_protocol_{trait_ident}_exports");
        let protocol_contract_ident = quote::format_ident!("vela_protocol_contract_{trait_ident}");
        quote! {
            #[must_use]
            pub fn #bundle_ident() -> ::vela_engine::interop::ExportBundle {
                ::vela_engine::interop::ExportBundle::with_protocols(
                    vec![#(Self::#contract_functions()),*],
                    vec![#protocol_contract_ident()],
                    |builder| {
                        let builder = builder;
                        #(let builder = Self::#registration_functions(builder);)*
                        builder
                    },
                )
            }
        }
    } else {
        quote! {
            #[must_use]
            pub fn vela_inherent_exports() -> ::vela_engine::interop::ExportBundle {
                ::vela_engine::interop::ExportBundle::new(
                    vec![#(Self::#contract_functions()),*],
                    |builder| {
                        let builder = builder;
                        #(let builder = Self::#registration_functions(builder);)*
                        builder
                    },
                )
            }
        }
    };
    let self_ty = &item.self_ty;
    let host_object_impl = trait_path
        .is_none()
        .then(|| crate::script_methods::base_script_host_object_impl_tokens(self_ty));
    Ok(quote! {
        #item
        impl #self_ty {
            #(#generated)*
            #bundle
        }
        #host_object_impl
    })
}

#[derive(Default)]
struct MethodAttrs {
    name: Option<String>,
    reflect_callable: bool,
    host_collection: bool,
}

fn take_method_attrs(method: &mut syn::ImplItemFn) -> Result<MethodAttrs> {
    let mut parsed = MethodAttrs::default();
    let mut retained = Vec::with_capacity(method.attrs.len());
    for attr in std::mem::take(&mut method.attrs) {
        if !attr.path().is_ident("script_method") {
            retained.push(attr);
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("reflect") || meta.path.is_ident("reflect_callable") {
                parsed.reflect_callable = meta.value()?.parse::<LitBool>()?.value;
                return Ok(());
            }
            if meta.path.is_ident("name") {
                if parsed.name.is_some() {
                    return Err(meta.error("duplicate method name"));
                }
                let value = meta.value()?.parse::<LitStr>()?.value();
                if value.is_empty()
                    || !value.chars().enumerate().all(|(index, ch)| {
                        ch == '_' || ch.is_alphanumeric() && (index > 0 || ch.is_alphabetic())
                    })
                {
                    return Err(meta.error("method name must be a Vela identifier"));
                }
                parsed.name = Some(value);
                return Ok(());
            }
            if meta.path.is_ident("host_collection") {
                parsed.host_collection = true;
                return Ok(());
            }
            Err(meta.error(
                "#[methods] supports only name, reflect, and host_collection on #[script_method]",
            ))
        })?;
    }
    method.attrs = retained;
    Ok(parsed)
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

    #[test]
    fn methods_can_keep_a_rust_adapter_name_private_to_rust() {
        let expanded = expand_result(
            quote! { path = "config::EquipmentTable" },
            quote! {
                impl EquipmentTable {
                    #[script_method(name = "get")]
                    pub fn vela_get(&self, key: i32) -> Option<&Equipment> {
                        self.get(&key)
                    }
                }
            },
        )
        .expect("method alias classifies");
        let output = expanded.to_string();

        assert!(output.contains("\"config::EquipmentTable::get\""));
        assert!(!output.contains("\"config::EquipmentTable::vela_get\""));
        assert!(output.contains("vela_get"));
    }

    #[test]
    fn methods_can_mark_borrowed_host_object_slices() {
        let expanded = expand_result(
            quote! { path = "config::EquipmentTable" },
            quote! {
                impl EquipmentTable {
                    #[script_method(name = "values", host_collection)]
                    pub fn vela_values(&self) -> &[Equipment] {
                        self.values()
                    }
                }
            },
        )
        .expect("host collection return classifies");
        let output = expanded.to_string();

        assert!(output.contains("register_rust_host_slice"));
        assert!(output.contains("VelaHostBoundary"));
    }

    #[test]
    fn methods_keep_str_backing_alive_for_sync_and_scoped_calls() {
        let expanded = expand_result(
            quote! { path = "config::StringTable" },
            quote! {
                impl StringTable {
                    pub fn contains(&self, key: &str) -> bool {
                        self.get(key).is_some()
                    }

                    pub fn get(&self, key: &str) -> Option<&Row> {
                        self.rows.get(key)
                    }
                }
            },
        )
        .expect("borrowed string parameters classify");
        let output = expanded.to_string();

        assert_eq!(
            output
                .matches("String as :: vela_engine :: args :: FromScriptArg")
                .count(),
            2
        );
        assert_eq!(output.matches("as_str").count(), 2);
    }
}
