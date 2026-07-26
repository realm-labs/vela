use proc_macro2::{Ident, TokenStream};
use quote::{ToTokens, format_ident, quote};
use std::collections::HashSet;
use syn::{
    Attribute, Expr, ImplItem, ImplItemFn, ItemImpl, LitStr, Result, Token, Type, Visibility,
    ext::IdentExt,
    parse::{Parse, ParseStream, Parser},
    parse2,
};

use crate::attrs::parse_qualified_name;
use crate::export::emission;
use crate::export::emission::hint_tokens;
use crate::export::signature::{classify_method, classify_method_with_host_collection_returns};
use crate::methods::{MethodAttrs, take_method_attrs};
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
    let adapter_ident = format_ident!("__VelaExternalHostAdapter_{}_{}", rust_ident, host_id);
    let trait_ident = format_ident!("__VelaExternalHostMethods_{}_{}", rust_ident, host_id);
    let register_ident = attrs.register;

    let mut trait_signatures = Vec::new();
    let mut trait_methods = Vec::new();
    let mut generated = Vec::new();
    let mut contract_functions = Vec::new();
    let mut registration_functions = Vec::new();
    let mut fields = Vec::new();
    let mut methods = Vec::new();
    let mut fields_block_seen = false;

    for impl_item in std::mem::take(&mut item.items) {
        match impl_item {
            ImplItem::Fn(mut method) => {
                let method_attrs = take_method_attrs(&mut method)?;
                let public_name = method_attrs
                    .name
                    .clone()
                    .unwrap_or_else(|| method.sig.ident.unraw().to_string());
                methods.push(ExternalMethod {
                    method,
                    attrs: method_attrs,
                    public_name,
                    field: None,
                });
            }
            ImplItem::Macro(item) if item.mac.path.is_ident("vela_fields") => {
                if fields_block_seen {
                    return Err(syn::Error::new_spanned(
                        item,
                        "#[external_host] accepts only one vela_fields! block",
                    ));
                }
                fields_block_seen = true;
                for field in parse2::<ExternalFields>(item.mac.tokens)?.fields {
                    let helper_ident = format_ident!("__vela_field_{}", field.rust_name.unraw());
                    let return_type = &field.return_type;
                    let expression = &field.expression;
                    let method = syn::parse2::<ImplItemFn>(quote! {
                        pub fn #helper_ident(&self) -> #return_type {
                            #expression
                        }
                    })?;
                    let method_attrs = MethodAttrs {
                        host_collection: field.host_collection,
                        ..MethodAttrs::default()
                    };
                    methods.push(ExternalMethod {
                        method,
                        attrs: method_attrs,
                        public_name: field.public_name.clone(),
                        field: Some(field),
                    });
                }
            }
            unsupported => {
                return Err(syn::Error::new_spanned(
                    unsupported,
                    "#[external_host] supports methods and one vela_fields! block",
                ));
            }
        }
    }

    let mut public_names = HashSet::new();
    for mut exported in methods {
        let method = &mut exported.method;
        reject_generic_signature(&method.sig.generics, "#[external_host]")?;
        reject_unsafe_signature(&method.sig, "#[external_host]")?;
        reject_extern_signature(&method.sig, "#[external_host]")?;
        let signature = if exported.attrs.host_collection {
            classify_method_with_host_collection_returns(&method.sig, &exported.attrs.effects)?
        } else {
            classify_method(&method.sig, &exported.attrs.effects)?
        };
        let docs = docs_from_attrs(&method.attrs);
        let public_name = exported.public_name;
        if !public_names.insert(public_name.clone()) {
            return Err(syn::Error::new_spanned(
                &method.sig.ident,
                format!("duplicate external Host member `{public_name}`"),
            ));
        }
        if let Some(field) = exported.field {
            let hint = hint_tokens(&signature.returns.ty);
            fields.push(ExternalFieldSchema {
                id: vela_common::stable_id("host_field", &stable_path, &field.public_name),
                rust_name: field.rust_name.to_token_stream().to_string(),
                public_name: field.public_name,
                return_type: field.return_type.to_token_stream().to_string(),
                hint,
            });
        }
        generated.push(emission::method_contract(
            method,
            &self_ty,
            &attrs.path,
            &public_name,
            emission::MethodContractMetadata {
                docs: docs.as_deref(),
                reflect_callable: exported.attrs.reflect_callable,
                attrs: &exported.attrs.attrs,
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

    let schema_hash = external_host_schema_hash(&type_name, &module_name, &fields);
    let field_tokens = fields.iter().map(ExternalFieldSchema::tokens);
    let host_object_impl = crate::host_object::base_script_host_object_impl_tokens(&self_ty);

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
                #(
                    .field(#field_tokens)
                )*
            }
        }

        impl ::vela_engine::type_registration::VelaType for #self_ty {
            fn register(
                builder: ::vela_engine::builder::EngineBuilder,
            ) -> ::vela_engine::builder::EngineBuilder {
                builder.register_type_binding::<Self>(
                    <Self as ::vela_engine::schema::ScriptHostSchema>::
                        script_host_binding(),
                )
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
            let builder = builder.register_type::<#self_ty>();
            #(
                let builder = #adapter_ident::#registration_functions(builder);
            )*
            builder
        }
    })
}

struct ExternalMethod {
    method: ImplItemFn,
    attrs: MethodAttrs,
    public_name: String,
    field: Option<ExternalField>,
}

struct ExternalFields {
    fields: Vec<ExternalField>,
}

impl Parse for ExternalFields {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut fields = Vec::new();
        while !input.is_empty() {
            let attrs = Attribute::parse_outer(input)?;
            let host_collection = attrs
                .iter()
                .any(|attr| attr.path().is_ident("host_collection"));
            if let Some(attr) = attrs
                .iter()
                .find(|attr| !attr.path().is_ident("host_collection"))
            {
                return Err(syn::Error::new_spanned(
                    attr,
                    "vela_fields! supports only #[host_collection]",
                ));
            }
            let rust_name = input.call(Ident::parse_any)?;
            input.parse::<Token![:]>()?;
            let return_type = input.parse::<Type>()?;
            input.parse::<Token![=]>()?;
            let expression = input.parse::<Expr>()?;
            input.parse::<Token![;]>()?;
            fields.push(ExternalField {
                public_name: rust_name.unraw().to_string(),
                rust_name,
                return_type,
                expression,
                host_collection,
            });
        }
        Ok(Self { fields })
    }
}

struct ExternalField {
    rust_name: Ident,
    public_name: String,
    return_type: Type,
    expression: Expr,
    host_collection: bool,
}

struct ExternalFieldSchema {
    id: u64,
    rust_name: String,
    public_name: String,
    return_type: String,
    hint: TokenStream,
}

impl ExternalFieldSchema {
    fn tokens(&self) -> TokenStream {
        let id = u128::from(self.id);
        let name = &self.public_name;
        let rust_name = &self.rust_name;
        let hint = &self.hint;
        quote! {
            ::vela_reflect::registry::FieldDesc::new(
                ::vela_def::FieldId::new(#id),
                #name,
            )
            .access(
                ::vela_reflect::access::FieldAccess::new()
                    .readable(true)
                    .writable(false)
                    .reflect_readable(false)
                    .reflect_writable(false)
            )
            .type_hint((#hint).display_name())
            .attr("rust_name", #rust_name)
            .attr("vela_external_property", "true")
        }
    }
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

fn external_host_schema_hash(
    type_name: &str,
    module_name: &str,
    fields: &[ExternalFieldSchema],
) -> u64 {
    let mut hasher = crate::hash::StableHasher::new();
    hasher.write_str(type_name);
    hasher.write_str(module_name);
    let mut fields = fields.iter().collect::<Vec<_>>();
    fields.sort_by_key(|field| field.public_name.as_str());
    for field in fields {
        hasher.write_u64(field.id);
        hasher.write_str(&field.public_name);
        hasher.write_str(&field.return_type);
        hasher.write_bool(true);
        hasher.write_bool(false);
    }
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
                    vela_fields! {
                        count: i32 = self.count;
                    }

                    #[vela(name = "get")]
                    pub fn get(&self, key: i32) -> Option<&crate::Item> {
                        crate::ItemTable::get(self, &key)
                    }

                    #[vela(name = "type")]
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
        assert!(expanded.contains("vela_external_property"));
        assert!(expanded.contains("FieldDesc :: new"));
        assert!(expanded.contains("__vela_field_count"));
        assert!(expanded.contains("vela_callable_contract_type"));
        assert!(!expanded.contains("vela_callable_contract_r#type"));
    }
}
