use std::collections::BTreeSet;

use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{Data, DeriveInput, Fields, Result, parse2};

use crate::attrs::{error, inferred_type_hint, parse_script_attrs, spanned_error};
use crate::hash::StableHasher;
use crate::script_host::type_identity;

struct ValueField {
    rust_ident: Ident,
    script_name: String,
    stable_name: String,
    id: u64,
    type_hint: Option<String>,
    docs: Option<String>,
    attrs: Vec<(String, String)>,
}

pub(crate) fn expand(input: TokenStream) -> TokenStream {
    match expand_result(input) {
        Ok(tokens) => tokens,
        Err(error) => error.to_compile_error(),
    }
}

fn expand_result(input: TokenStream) -> Result<TokenStream> {
    let input = parse2::<DeriveInput>(input)?;
    if !input.generics.params.is_empty() || input.generics.where_clause.is_some() {
        return Err(spanned_error(
            &input.generics,
            "Value does not support generic Rust types; register a concrete manual TypeBinding",
        ));
    }
    let Data::Struct(data) = &input.data else {
        return Err(spanned_error(
            &input,
            "Value currently supports named structs; enum generation is a later S2 slice",
        ));
    };
    let Fields::Named(named) = &data.fields else {
        return Err(spanned_error(
            &data.fields,
            "Value requires named struct fields",
        ));
    };
    let type_attrs = parse_script_attrs(&input.attrs)?;
    let identity = type_identity(
        &input.ident,
        type_attrs.path,
        type_attrs.module,
        type_attrs.name,
        type_attrs.alias,
    )?;
    let fields = collect_fields(named, &identity.stable_path)?;
    let schema_hash = schema_hash(
        &identity.name,
        &identity.module,
        &type_attrs.attrs,
        &type_attrs.traits,
        &fields,
    );
    let ident = input.ident;
    let type_name = identity.name;
    let module_name = identity.module;
    let qualified_type_name = format!("{module_name}::{type_name}");
    let type_id = identity.type_id;
    let decode_operation = format!("{module_name}::{type_name} Value decode");
    let docs = type_attrs.docs.map(|docs| quote! { .docs(#docs) });
    let type_attr_tokens = type_attrs.attrs.iter().map(|(name, value)| {
        quote! { desc = desc.attr(#name, #value); }
    });
    let trait_tokens = type_attrs.traits.iter().map(|trait_name| {
        quote! {
            desc = desc.trait_impl(::vela_reflect::registry::TraitDesc::new(#trait_name));
        }
    });
    let field_descs = fields.iter().map(field_desc_tokens);
    let encode_fields = fields.iter().map(|field| {
        let rust_ident = &field.rust_ident;
        let script_name = &field.script_name;
        quote! {
            (#script_name, ::vela_engine::args::IntoScriptArg::into_script_arg(self.#rust_ident))
        }
    });
    let decode_fields = fields.iter().map(|field| {
        let rust_ident = &field.rust_ident;
        let script_name = &field.script_name;
        quote! {
            #rust_ident: ::vela_engine::args::FromScriptArg::from_script_arg(
                fields.get(#script_name).ok_or_else(|| {
                    ::vela_vm::error::VmError::new(
                        ::vela_vm::error::VmErrorKind::TypeMismatch {
                            operation: #decode_operation,
                        },
                    )
                })?,
            )?
        }
    });
    let field_count = fields.len();

    Ok(quote! {
        impl #ident {
            #[must_use]
            pub const fn vela_value_type_id() -> ::vela_def::TypeId {
                ::vela_def::TypeId::new(#type_id)
            }

            #[must_use]
            pub fn vela_value_type_desc() -> ::vela_reflect::registry::TypeDesc {
                let mut desc = ::vela_reflect::registry::TypeDesc::new(
                    ::vela_reflect::registry::TypeKey::new(
                        Self::vela_value_type_id(),
                        #qualified_type_name,
                    ),
                )
                .kind(::vela_reflect::registry::TypeKind::ScriptStruct)
                .schema_hash(::vela_reflect::registry::SchemaHash::new(#schema_hash))
                .attr("module", #module_name)
                #docs;
                #(#type_attr_tokens)*
                #(#trait_tokens)*
                #(
                    desc = desc.field(#field_descs);
                )*
                desc
            }

            #[must_use]
            pub fn vela_type_binding() -> ::vela_engine::type_binding::TypeBinding<Self> {
                <Self as ::vela_engine::schema::ScriptValueSchema>::script_value_binding()
            }
        }

        impl ::vela_engine::schema::ScriptValueSchema for #ident {
            fn script_value_type_desc() -> ::vela_reflect::registry::TypeDesc {
                Self::vela_value_type_desc()
            }
        }

        impl ::vela_engine::interop::VelaValueBoundary for #ident {
            fn vela_type_hint() -> ::vela_engine::native::TypeHint {
                ::vela_engine::native::TypeHint::Record(
                    Self::vela_value_type_desc().key,
                )
            }
        }

        impl ::vela_engine::args::IntoScriptArg for #ident {
            fn into_script_arg(self) -> ::vela_vm::owned_value::OwnedValue {
                ::vela_vm::owned_value::OwnedValue::record(
                    #qualified_type_name,
                    [#(#encode_fields),*],
                )
            }
        }

        impl ::vela_engine::args::FromScriptArg for #ident {
            const TYPE_NAME: &'static str = #qualified_type_name;

            fn from_script_arg(
                value: &::vela_vm::owned_value::OwnedValue,
            ) -> ::vela_vm::error::VmResult<Self> {
                let ::vela_vm::owned_value::OwnedValue::Record { type_name, fields } = value else {
                    return Err(::vela_vm::error::VmError::new(
                        ::vela_vm::error::VmErrorKind::TypeMismatch {
                            operation: #decode_operation,
                        },
                    ));
                };
                if type_name != #qualified_type_name || fields.len() != #field_count {
                    return Err(::vela_vm::error::VmError::new(
                        ::vela_vm::error::VmErrorKind::TypeMismatch {
                            operation: #decode_operation,
                        },
                    ));
                }
                Ok(Self { #(#decode_fields),* })
            }
        }
    })
}

fn collect_fields(fields: &syn::FieldsNamed, stable_type_path: &str) -> Result<Vec<ValueField>> {
    let mut seen_names = BTreeSet::new();
    let mut seen_stable_names = BTreeSet::new();
    let mut seen_ids = BTreeSet::new();
    let mut result = Vec::with_capacity(fields.named.len());
    for field in &fields.named {
        let attrs = parse_script_attrs(&field.attrs)?;
        if attrs.skip {
            return Err(spanned_error(
                field,
                "Value fields cannot be skipped because structural decoding must reconstruct the exact Rust value",
            ));
        }
        let rust_ident = field
            .ident
            .clone()
            .ok_or_else(|| spanned_error(field, "Value requires named struct fields"))?;
        let rust_name = rust_ident.to_string();
        let script_name = attrs.field_name(&rust_name);
        if script_name.is_empty() || !seen_names.insert(script_name.clone()) {
            return Err(error(
                rust_ident.span(),
                "Value field names must be non-empty and unique",
            ));
        }
        let stable_name = attrs.alias.unwrap_or_else(|| script_name.clone());
        if !seen_stable_names.insert(stable_name.clone()) {
            return Err(error(rust_ident.span(), "duplicate Value field alias"));
        }
        let id = vela_common::stable_id("value_field", stable_type_path, &stable_name);
        if !seen_ids.insert(id) {
            return Err(error(
                rust_ident.span(),
                "duplicate generated Value field id",
            ));
        }
        result.push(ValueField {
            rust_ident,
            script_name,
            stable_name,
            id,
            type_hint: attrs.type_hint.or_else(|| inferred_type_hint(&field.ty)),
            docs: attrs.docs,
            attrs: attrs.attrs,
        });
    }
    Ok(result)
}

fn field_desc_tokens(field: &ValueField) -> TokenStream {
    let id = u128::from(field.id);
    let script_name = &field.script_name;
    let rust_name = field.rust_ident.to_string();
    let hint = field
        .type_hint
        .as_ref()
        .map(|hint| quote! { .type_hint(#hint) });
    let docs = field.docs.as_ref().map(|docs| quote! { .docs(#docs) });
    let attrs = field.attrs.iter().map(|(name, value)| {
        quote! { .attr(#name, #value) }
    });
    quote! {
        ::vela_reflect::registry::FieldDesc::new(
            ::vela_def::FieldId::new(#id),
            #script_name,
        )
        .writable(true)
        .attr("rust_name", #rust_name)
        #(#attrs)*
        #hint
        #docs
    }
}

fn schema_hash(
    type_name: &str,
    module: &str,
    attrs: &[(String, String)],
    traits: &[String],
    fields: &[ValueField],
) -> u64 {
    let mut hasher = StableHasher::new();
    hasher.write_str(type_name);
    hasher.write_str(module);
    for (name, value) in attrs {
        hasher.write_str(name);
        hasher.write_str(value);
    }
    for trait_name in traits {
        hasher.write_str(trait_name);
    }
    let mut fields = fields.iter().collect::<Vec<_>>();
    fields.sort_by_key(|field| (field.id, field.script_name.as_str()));
    for field in fields {
        hasher.write_u64(field.id);
        hasher.write_str(&field.script_name);
        hasher.write_str(&field.stable_name);
        hasher.write_str(field.type_hint.as_deref().unwrap_or(""));
        for (name, value) in &field.attrs {
            hasher.write_str(name);
            hasher.write_str(value);
        }
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use quote::quote;

    use super::expand_result;

    #[test]
    fn rejects_skipped_fields_that_cannot_be_reconstructed() {
        let error = expand_result(quote! {
            #[script(path = "host::PartialValue")]
            struct PartialValue {
                visible: i64,
                #[script(skip)]
                hidden: i64,
            }
        })
        .expect_err("skipped structural field should fail");

        assert!(error.to_string().contains("cannot be skipped"));
    }

    #[test]
    fn rejects_generic_structs_instead_of_generating_script_generics() {
        let error = expand_result(quote! {
            #[script(path = "host::Envelope")]
            struct Envelope<T> {
                value: T,
            }
        })
        .expect_err("generic Value should fail");

        assert!(error.to_string().contains("does not support generic"));
    }
}
