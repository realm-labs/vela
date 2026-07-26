use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::{
    ImplItem, ItemImpl, LitStr, Result, Type, Visibility, ext::IdentExt, parse::Parser, parse2,
};

use crate::attrs::parse_qualified_name;
use crate::export::emission;
use crate::export::signature::{classify_method, classify_method_with_host_collection_returns};
use crate::methods::take_method_attrs;
use crate::script_host::type_identity;
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
            "#[external_host] accepts a declaration-only inherent impl",
        ));
    }
    reject_generic_signature(&item.generics, "#[external_host]")?;
    let attrs = parse_attrs(attr)?;
    let self_ty = item.self_ty.clone();
    let rust_ident = type_ident(&self_ty)?;
    let identity = type_identity(&rust_ident, Some(attrs.path.clone()), None, None, None)?;
    let type_name = identity.name;
    let module_name = identity.module;
    let stable_path = identity.stable_path;
    let type_id = identity.type_id;
    let host_id = identity.host_id;
    let schema_hash = empty_host_schema_hash(&type_name, &module_name);
    let adapter_ident = format_ident!("__VelaExternalHostAdapter_{}_{}", rust_ident, host_id);
    let trait_ident = format_ident!("__VelaExternalHostMethods_{}_{}", rust_ident, host_id);
    let register_ident = attrs.register;

    let mut trait_signatures = Vec::new();
    let mut trait_methods = Vec::new();
    let mut generated = Vec::new();
    let mut contract_functions = Vec::new();
    let mut registration_functions = Vec::new();

    for impl_item in &mut item.items {
        let ImplItem::Fn(method) = impl_item else {
            return Err(syn::Error::new_spanned(
                impl_item,
                "#[external_host] supports methods only",
            ));
        };
        reject_generic_signature(&method.sig.generics, "#[external_host]")?;
        reject_unsafe_signature(&method.sig, "#[external_host]")?;
        reject_extern_signature(&method.sig, "#[external_host]")?;
        let method_attrs = take_method_attrs(method)?;
        let signature = if method_attrs.host_collection {
            classify_method_with_host_collection_returns(&method.sig, &method_attrs.effects)?
        } else {
            classify_method(&method.sig, &method_attrs.effects)?
        };
        let docs = docs_from_attrs(&method.attrs);
        let public_name = method_attrs
            .name
            .unwrap_or_else(|| method.sig.ident.to_string());
        generated.push(emission::method_contract(
            method,
            &self_ty,
            &attrs.path,
            &public_name,
            emission::MethodContractMetadata {
                docs: docs.as_deref(),
                reflect_callable: method_attrs.reflect_callable,
                attrs: &method_attrs.attrs,
            },
            &signature,
        ));
        generated.push(
            emission::method_adapter(
                method,
                &self_ty,
                Some(&syn::parse_quote!(#trait_ident)),
                &public_name,
                &signature,
            )
            .ok_or_else(|| {
                syn::Error::new_spanned(
                    &method.sig,
                    "this external Host method requires an unsupported adapter shape",
                )
            })?,
        );
        let helper_ident = method.sig.ident.unraw();
        contract_functions.push(format_ident!("vela_callable_contract_{helper_ident}"));
        registration_functions.push(format_ident!("vela_register_export_{helper_ident}"));

        let signature = &method.sig;
        trait_signatures.push(quote! { #signature; });
        method.vis = Visibility::Inherited;
        trait_methods.push(method.clone());
    }

    let host_object_impl = crate::script_methods::base_script_host_object_impl_tokens(&self_ty);

    Ok(quote! {
        #[allow(non_camel_case_types)]
        trait #trait_ident {
            #(#trait_signatures)*
        }

        impl #trait_ident for #self_ty {
            #(#trait_methods)*
        }

        #[allow(non_camel_case_types)]
        struct #adapter_ident;

        impl #adapter_ident {
            #(#generated)*
        }

        impl #self_ty {
            #[doc(hidden)]
            pub(crate) const fn vela_stable_type_path() -> &'static str {
                #stable_path
            }
        }

        impl ::vela_engine::schema::ScriptHostSchema for #self_ty {
            fn script_host_type_desc() -> ::vela_reflect::registry::TypeDesc {
                ::vela_reflect::registry::TypeDesc::new(
                    ::vela_reflect::registry::TypeKey::new(
                        ::vela_def::TypeId::new(#type_id),
                        #type_name,
                    ),
                )
                .kind(::vela_reflect::registry::TypeKind::Host)
                .schema_hash(::vela_reflect::registry::SchemaHash::new(#schema_hash))
                .host_type(::vela_common::HostTypeId::new(#host_id))
                .attr("module", #module_name)
            }
        }

        impl ::vela_host::object::ScriptHostFieldAccess for #self_ty {
            fn script_host_type_id(&self) -> ::vela_common::HostTypeId {
                ::vela_common::HostTypeId::new(#host_id)
            }

            fn script_host_type_shape() -> Option<::std::string::String> {
                Some(#type_name.to_owned())
            }

            fn read_host_target_from(
                &self,
                target: ::vela_host::target::HostTargetInstance<'_>,
                _offset: usize,
            ) -> ::vela_host::error::HostResult<::vela_host::value::HostValue> {
                Err(::vela_host::error::HostError {
                    kind: ::vela_host::error::HostErrorKind::MissingPath {
                        path: target.to_diagnostic_path().to_host_path(),
                    },
                    source_span: None,
                })
            }

            fn write_host_target_from(
                &mut self,
                target: ::vela_host::target::HostTargetInstance<'_>,
                _offset: usize,
                _value: ::vela_host::value::HostValue,
            ) -> ::vela_host::error::HostResult<()> {
                Err(::vela_host::error::HostError {
                    kind: ::vela_host::error::HostErrorKind::PermissionDenied {
                        path: target.to_diagnostic_path().to_host_path(),
                        action: "write",
                    },
                    source_span: None,
                })
            }
        }

        #host_object_impl

        #[must_use]
        pub fn #register_ident(
            builder: ::vela_engine::builder::EngineBuilder,
        ) -> ::vela_engine::builder::EngineBuilder {
            let builder = builder.register_rust_type::<#self_ty>(
                <#self_ty as ::vela_engine::schema::ScriptHostSchema>::
                    script_host_binding(),
            );
            #(
                let builder = #adapter_ident::#registration_functions(builder);
            )*
            builder
        }
    })
}

struct ExternalHostAttrs {
    path: String,
    register: Ident,
}

fn parse_attrs(attr: TokenStream) -> Result<ExternalHostAttrs> {
    let mut path = None;
    let mut register = None;
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("path") {
            path = Some(parse_qualified_name(
                meta.value()?.parse::<LitStr>()?,
                "external Host path",
            )?);
            return Ok(());
        }
        if meta.path.is_ident("register") {
            let value = meta.value()?.parse::<LitStr>()?;
            register = Some(syn::parse_str::<Ident>(&value.value()).map_err(|_| {
                syn::Error::new(value.span(), "register must be a Rust identifier")
            })?);
            return Ok(());
        }
        Err(meta.error("#[external_host] supports only path and register"))
    });
    parser.parse2(attr)?;
    Ok(ExternalHostAttrs {
        path: path
            .ok_or_else(|| syn::Error::new(proc_macro2::Span::call_site(), "missing path"))?,
        register: register
            .ok_or_else(|| syn::Error::new(proc_macro2::Span::call_site(), "missing register"))?,
    })
}

fn type_ident(ty: &Type) -> Result<Ident> {
    let Type::Path(path) = ty else {
        return Err(syn::Error::new_spanned(
            ty,
            "external Host owner must be a named type",
        ));
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.clone())
        .ok_or_else(|| syn::Error::new_spanned(ty, "missing external Host type name"))
}

fn empty_host_schema_hash(type_name: &str, module_name: &str) -> u64 {
    let mut hasher = crate::hash::StableHasher::new();
    hasher.write_str(type_name);
    hasher.write_str(module_name);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use quote::quote;

    use super::expand_result;

    #[test]
    fn external_host_keeps_adapter_methods_off_the_inherent_type() {
        let expanded = expand_result(
            quote! {
                path = "config::ItemTable",
                register = "register_item_table"
            },
            quote! {
                impl crate::ItemTable {
                    #[script_method(name = "get")]
                    pub fn get(&self, key: i32) -> Option<&crate::Item> {
                        crate::ItemTable::get(self, &key)
                    }

                    #[script_method(name = "type")]
                    pub fn r#type(&self) -> i32 {
                        1
                    }
                }
            },
        )
        .expect("external Host binding expands")
        .to_string();

        assert!(expanded.contains("trait __VelaExternalHostMethods_ItemTable"));
        assert!(expanded.contains("impl __VelaExternalHostMethods_ItemTable"));
        assert!(!expanded.contains("impl crate :: ItemTable { pub fn get"));
        assert!(expanded.contains("pub fn register_item_table"));
        assert!(expanded.contains("ScriptHostSchema for crate :: ItemTable"));
        assert!(expanded.contains("vela_callable_contract_type"));
        assert!(!expanded.contains("vela_callable_contract_r#type"));
    }
}
