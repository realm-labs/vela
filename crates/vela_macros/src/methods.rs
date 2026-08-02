use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{
    FnArg, GenericArgument, ImplItem, ItemImpl, LitBool, LitStr, PathArguments, Result, ReturnType,
    Type, Visibility, ext::IdentExt, parse::Parser, parse2,
};

use crate::attrs::{parse_key_value_attr, parse_qualified_name, reject_duplicate_attr_keys};
use crate::export::emission;
use crate::export::signature::{
    EffectName, classify_method, classify_method_with_host_collection_returns,
};
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
    reject_generic_signature(&item.generics, "#[vela_macros::methods]")?;
    let MethodsOptions {
        owner_path,
        public_only,
    } = parse_options(attr, &item)?;
    let trait_path = item.trait_.as_ref().map(|(_, path, _)| path);
    let mut generated = Vec::new();
    let mut contract_functions = Vec::new();
    let mut registration_functions = Vec::new();
    for impl_item in &mut item.items {
        let ImplItem::Fn(method) = impl_item else {
            continue;
        };
        let method_attrs = take_method_attrs(method)?;
        if method_attrs.skip {
            continue;
        }
        if public_only && !matches!(method.vis, Visibility::Public(_)) {
            continue;
        }
        if !method_attrs.explicit
            && method
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr"))
        {
            continue;
        }
        let Some(FnArg::Receiver(receiver)) = method.sig.inputs.first() else {
            if method_attrs.explicit {
                return Err(syn::Error::new_spanned(
                    &method.sig,
                    "#[vela] associated functions require constructor registration; #[vela_macros::methods] exports instance methods",
                ));
            }
            continue;
        };
        if !method_attrs.explicit
            && (receiver.reference.is_none()
                || !method.sig.generics.params.is_empty()
                || method.sig.generics.where_clause.is_some()
                || has_opaque_return(&method.sig.output)
                || has_nested_borrowed_return(&method.sig.output))
        {
            continue;
        }
        reject_generic_signature(&method.sig.generics, "#[vela_macros::methods]")?;
        reject_unsafe_signature(&method.sig, "#[vela_macros::methods]")?;
        reject_extern_signature(&method.sig, "#[vela_macros::methods]")?;
        let signature = if method_attrs.host_collection {
            classify_method_with_host_collection_returns(&method.sig, &method_attrs.effects)?
        } else {
            classify_method(&method.sig, &method_attrs.effects)?
        };
        let public_name = method_attrs
            .name
            .clone()
            .unwrap_or_else(|| method.sig.ident.to_string());
        let Some(adapter) =
            emission::method_adapter(method, &item.self_ty, trait_path, &public_name, &signature)
        else {
            if method_attrs.explicit {
                return Err(syn::Error::new_spanned(
                    &method.sig,
                    "this exported method requires the async or borrowed-return adapter batch",
                ));
            }
            continue;
        };
        let docs = docs_from_attrs(&method.attrs);
        generated.push(emission::method_contract(
            method,
            &item.self_ty,
            &owner_path,
            &public_name,
            emission::MethodContractMetadata {
                docs: docs.as_deref(),
                reflect_callable: method_attrs.reflect_callable,
                attrs: &method_attrs.attrs,
            },
            &signature,
        ));
        generated.push(adapter);
        let helper_ident = method.sig.ident.unraw();
        let contract_ident = quote::format_ident!("vela_callable_contract_{helper_ident}");
        let registration_ident = quote::format_ident!("vela_register_export_{helper_ident}");
        let method_registration_ident = quote::format_ident!("vela_method_{helper_ident}");
        generated.push(quote! {
            #[must_use]
            pub fn #method_registration_ident(
            ) -> ::vela_engine::registration::MethodRegistration<Self> {
                ::vela_engine::registration::MethodRegistration::new(
                    Self::#contract_ident(),
                    Self::#registration_ident,
                )
            }
        });
        contract_functions.push(contract_ident);
        registration_functions.push(registration_ident);
    }
    if trait_path.is_some() && generated.is_empty() {
        return Err(syn::Error::new_spanned(
            &item,
            "#[vela_macros::methods] requires at least one supported trait method",
        ));
    }
    let bundle = if let Some(trait_path) = trait_path {
        let trait_ident = &trait_path
            .segments
            .last()
            .expect("trait paths contain at least one segment")
            .ident;
        let bundle_ident = quote::format_ident!("vela_protocol_{trait_ident}_methods");
        let protocol_contract_ident = quote::format_ident!("vela_protocol_contract_{trait_ident}");
        quote! {
            #[must_use]
            pub fn #bundle_ident() -> ::vela_engine::registration::MethodsRegistration<Self> {
                ::vela_engine::registration::MethodsRegistration::with_protocols(
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
            pub fn vela_methods() -> ::vela_engine::registration::MethodsRegistration<Self> {
                ::vela_engine::registration::MethodsRegistration::new(
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
    Ok(quote! {
        #item
        impl #self_ty {
            #(#generated)*
            #bundle
        }
    })
}

fn has_opaque_return(output: &ReturnType) -> bool {
    matches!(output, ReturnType::Type(_, ty) if matches!(ty.as_ref(), Type::ImplTrait(_)))
}

fn has_nested_borrowed_return(output: &ReturnType) -> bool {
    let ReturnType::Type(_, ty) = output else {
        return false;
    };
    nested_borrowed_type(ty, false)
}

fn nested_borrowed_type(ty: &Type, nested_in_owned_container: bool) -> bool {
    match ty {
        Type::Reference(_) => nested_in_owned_container,
        Type::Tuple(tuple) => tuple
            .elems
            .iter()
            .any(|element| nested_borrowed_type(element, nested_in_owned_container)),
        Type::Path(path) => {
            let Some(segment) = path.path.segments.last() else {
                return false;
            };
            let transparent = matches!(segment.ident.to_string().as_str(), "Option" | "Result");
            let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
                return false;
            };
            arguments.args.iter().any(|argument| {
                let GenericArgument::Type(argument) = argument else {
                    return false;
                };
                nested_borrowed_type(argument, nested_in_owned_container || !transparent)
            })
        }
        _ => false,
    }
}

#[derive(Default)]
pub(crate) struct MethodAttrs {
    pub(crate) explicit: bool,
    pub(crate) skip: bool,
    pub(crate) name: Option<String>,
    pub(crate) reflect_callable: bool,
    pub(crate) host_collection: bool,
    pub(crate) effects: BTreeSet<EffectName>,
    pub(crate) attrs: Vec<(String, String)>,
}

pub(crate) fn take_method_attrs(method: &mut syn::ImplItemFn) -> Result<MethodAttrs> {
    let mut parsed = MethodAttrs::default();
    let mut retained = Vec::with_capacity(method.attrs.len());
    for attr in std::mem::take(&mut method.attrs) {
        if !attr.path().is_ident("vela") {
            retained.push(attr);
            continue;
        }
        parsed.explicit = true;
        if matches!(attr.meta, syn::Meta::Path(_)) {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("skip") {
                parsed.skip = true;
                return Ok(());
            }
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
            if meta.path.is_ident("attr") {
                parsed.attrs.push(parse_key_value_attr(
                    meta.value()?.parse::<LitStr>()?,
                    "vela",
                )?);
                return Ok(());
            }
            if meta.path.is_ident("effects") {
                return meta.parse_nested_meta(|effect| {
                    let Some(ident) = effect.path.get_ident() else {
                        return Err(effect.error("effect must be an identifier"));
                    };
                    let effect_name = EffectName::parse(ident)?;
                    if effect_name == EffectName::Pure {
                        return Err(effect.error(
                            "effects(...) only adds effects; `pure` cannot remove an inferred host effect",
                        ));
                    }
                    if !parsed.effects.insert(effect_name) {
                        return Err(effect.error("duplicate additional effect"));
                    }
                    Ok(())
                });
            }
            Err(meta.error(
                "#[methods] supports only skip, name, reflect, host_collection, attr, and effects(...) on #[vela]",
            ))
        })?;
    }
    reject_duplicate_attr_keys(&parsed.attrs, "vela")?;
    method.attrs = retained;
    Ok(parsed)
}

struct MethodsOptions {
    owner_path: String,
    public_only: bool,
}

fn parse_options(attr: TokenStream, item: &ItemImpl) -> Result<MethodsOptions> {
    let mut configured = None;
    let mut public_only = false;
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("path") {
            configured = Some(parse_qualified_name(
                meta.value()?.parse::<LitStr>()?,
                "methods path",
            )?);
            return Ok(());
        }
        if meta.path.is_ident("public_only") {
            public_only = true;
            return Ok(());
        }
        Err(meta.error("unsupported methods attribute"))
    });
    parser.parse2(attr)?;
    if let Some(configured) = configured {
        return Ok(MethodsOptions {
            owner_path: configured,
            public_only,
        });
    }
    let syn::Type::Path(path) = item.self_ty.as_ref() else {
        return Err(syn::Error::new_spanned(
            &item.self_ty,
            "methods owner must have a stable named type path",
        ));
    };
    Ok(MethodsOptions {
        owner_path: path.path.to_token_stream().to_string().replace(' ', ""),
        public_only,
    })
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
        assert!(output.contains("vela_callable_contract_helper"));
        assert!(output.contains("host_read"));
        assert!(output.contains("host_write"));
    }

    #[test]
    fn methods_public_only_excludes_crate_visible_helpers() {
        let expanded = expand_result(
            quote! { path = "game::Player", public_only },
            quote! {
                impl Player {
                    pub fn level(&self) -> i64 { self.level }
                    pub(crate) fn load(&mut self) {}
                }
            },
        )
        .expect("public-only method group classifies");
        let output = expanded.to_string();

        assert!(output.contains("vela_callable_contract_level"));
        assert!(!output.contains("vela_callable_contract_load"));
    }

    #[test]
    fn methods_automatically_exports_supported_instance_methods() {
        let expanded = expand_result(
            quote! { path = "game::Player" },
            quote! {
                impl Player {
                    pub fn level(&self) -> i64 { self.level }
                    pub fn new(level: i64) -> Self { Self { level } }
                    pub fn unsupported_shape(&self) -> impl Iterator<Item = i64> {
                        std::iter::once(self.level)
                    }
                }
            },
        )
        .expect("explicit-only method group classifies");
        let output = expanded.to_string();

        assert!(output.contains("vela_callable_contract_level"));
        assert!(!output.contains("vela_callable_contract_new"));
        assert!(!output.contains("vela_callable_contract_unsupported_shape"));
    }

    #[test]
    fn methods_export_all_supported_instance_methods_regardless_of_visibility() {
        let expanded = expand_result(
            quote! { path = "game::Player" },
            quote! {
                impl Player {
                    pub(crate) fn crate_visible(&self) -> i64 { 1 }

                    fn explicitly_exposed(&self) -> i64 { 2 }

                    #[vela(skip)]
                    pub fn hidden(&self) -> i64 { 3 }

                    fn helper(&self) -> i64 { 4 }
                }
            },
        )
        .expect("method visibility policy should classify");
        let output = expanded.to_string();

        assert!(output.contains("vela_callable_contract_crate_visible"));
        assert!(output.contains("vela_callable_contract_explicitly_exposed"));
        assert!(!output.contains("vela_callable_contract_hidden"));
        assert!(output.contains("vela_callable_contract_helper"));
    }

    #[test]
    fn explicit_metadata_keeps_unsupported_methods_strict() {
        let error = expand_result(
            quote! { path = "game::Player" },
            quote! {
                impl Player {
                    #[vela]
                    pub fn values(&self) -> impl Iterator<Item = i64> {
                        std::iter::empty()
                    }
                }
            },
        )
        .expect_err("explicitly exported unsupported methods must report their signature");

        assert!(
            error
                .to_string()
                .contains("unsupported exported Rust boundary type")
        );
    }

    #[test]
    fn explicit_only_option_is_removed() {
        let error = expand_result(
            quote! { path = "game::Player", explicit_only },
            quote! {
                impl Player {
                    pub fn level(&self) -> i64 { self.level }
                }
            },
        )
        .expect_err("the explicit-only compatibility switch must stay removed");

        assert!(error.to_string().contains("unsupported methods attribute"));
    }

    #[test]
    fn methods_can_keep_a_rust_adapter_name_private_to_rust() {
        let expanded = expand_result(
            quote! { path = "config::EquipmentTable" },
            quote! {
                impl EquipmentTable {
                    #[vela(name = "get")]
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
                    #[vela(name = "values", host_collection)]
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
    fn methods_add_explicit_non_host_effects_to_receiver_effects() {
        let expanded = expand_result(
            quote! { path = "ops::Context" },
            quote! {
                impl Context {
                    #[vela(effects(event_emit, time))]
                    pub fn diagnostic(&self, message: String) -> bool {
                        !message.is_empty()
                    }
                }
            },
        )
        .expect("explicit method effects classify");
        let output = expanded.to_string();

        assert!(output.contains("host_read"));
        assert!(output.contains("event_emit"));
        assert!(output.contains("time"));
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

    #[test]
    fn empty_inherent_method_group_emits_only_an_export_bundle() {
        let expanded = expand_result(
            quote! { path = "config::Tables" },
            quote! {
                impl Tables {}
            },
        )
        .expect("an empty method group should remain a valid empty bundle");
        let output = expanded.to_string();

        assert!(output.contains("vela_methods"));
        assert!(!output.contains("ScriptHostObject"));
    }
}
