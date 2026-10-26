use std::collections::BTreeSet;

use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{Data, DataEnum, DataStruct, DeriveInput, Fields, Result, Type, parse2};

use crate::attrs::{ScriptAttrs, error, inferred_type_hint, parse_script_attrs, spanned_error};
use crate::hash::StableHasher;
use crate::type_identity::{TypeIdentity, type_identity};

struct ValueField {
    rust_ident: Ident,
    rust_type: Type,
    script_name: String,
    stable_name: String,
    id: u64,
    type_hint: Option<String>,
    explicit_type_hint: bool,
    docs: Option<String>,
    attrs: Vec<(String, String)>,
}

struct ValueVariant {
    rust_ident: Ident,
    script_name: String,
    stable_name: String,
    id: u64,
    fields: Vec<ValueField>,
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
    let type_attrs = parse_script_attrs(&input.attrs)?;
    let identity = type_identity(
        &input.ident,
        type_attrs.path.clone(),
        type_attrs.module.clone(),
        type_attrs.name.clone(),
        type_attrs.alias.clone(),
    )?;

    match &input.data {
        Data::Struct(data) => expand_struct(&input.ident, data, type_attrs, identity),
        Data::Enum(data) => expand_enum(&input.ident, data, type_attrs, identity),
        Data::Union(_) => Err(spanned_error(&input, "Value does not support Rust unions")),
    }
}

fn expand_struct(
    ident: &Ident,
    data: &DataStruct,
    type_attrs: ScriptAttrs,
    identity: TypeIdentity,
) -> Result<TokenStream> {
    let fields = match &data.fields {
        Fields::Named(named) => collect_fields(named, &identity.stable_path)?,
        Fields::Unit => Vec::new(),
        Fields::Unnamed(_) => {
            return Err(spanned_error(
                &data.fields,
                "Value requires named fields or a unit struct",
            ));
        }
    };
    let schema_hash = schema_hash(
        &identity.name,
        &identity.module,
        &type_attrs.attrs,
        &type_attrs.traits,
        &fields,
    );
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
    let encode_ref_fields = fields.iter().map(|field| {
        let rust_ident = &field.rust_ident;
        let script_name = &field.script_name;
        quote! {
            (
                #script_name,
                ::vela_engine::args::ToScriptValueRef::to_script_value_ref(
                    &self.#rust_ident
                ),
            )
        }
    });
    let encoded_fields = if fields.is_empty() {
        quote! {
            ::core::iter::empty::<(
                &'static str,
                ::vela_vm::owned_value::OwnedValue,
            )>()
        }
    } else {
        quote! { [#(#encode_fields),*] }
    };
    let encoded_ref_fields = if fields.is_empty() {
        quote! {
            ::core::iter::empty::<(
                &'static str,
                ::vela_vm::owned_value::OwnedValue,
            )>()
        }
    } else {
        quote! { [#(#encode_ref_fields),*] }
    };
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
    let decoded_value = if matches!(data.fields, Fields::Unit) {
        quote! { Self }
    } else {
        quote! { Self { #(#decode_fields),* } }
    };
    let dependency_registrations = fields.iter().map(|field| {
        let rust_type = &field.rust_type;
        quote! {
            let builder = <#rust_type as ::vela_engine::type_registration::RustValueType>::register_value_type_closure(builder);
        }
    });

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

            #[must_use]
            pub fn vela_type() -> ::vela_engine::registration::TypeRegistration<Self> {
                ::vela_engine::registration::TypeRegistration::of()
            }
        }

        impl ::vela_engine::schema::ScriptValueSchema for #ident {
            fn script_value_type_desc() -> ::vela_reflect::registry::TypeDesc {
                Self::vela_value_type_desc()
            }
        }

        impl ::vela_engine::type_registration::RustValueType for #ident {
            fn register_value_type_closure(
                builder: ::vela_engine::builder::EngineBuilder,
            ) -> ::vela_engine::builder::EngineBuilder {
                #(#dependency_registrations)*
                builder.register_generated_type_binding::<Self>(Self::vela_type_binding())
            }
        }

        impl ::vela_engine::type_registration::VelaType for #ident {
            fn register(
                builder: ::vela_engine::builder::EngineBuilder,
            ) -> ::vela_engine::builder::EngineBuilder {
                <Self as ::vela_engine::type_registration::RustValueType>::
                    register_value_type_closure(builder)
            }
        }

        impl ::vela_engine::interop::VelaValueBoundary for #ident {
            fn vela_type_hint() -> ::vela_engine::native::TypeHint {
                ::vela_engine::native::TypeHint::Record(
                    Self::vela_value_type_desc().key,
                )
            }
        }

        impl ::vela_engine::args::ToScriptValueRef for #ident {
            fn to_script_value_ref(&self) -> ::vela_vm::owned_value::OwnedValue {
                ::vela_vm::owned_value::OwnedValue::record(
                    #qualified_type_name,
                    #encoded_ref_fields,
                )
            }
        }

        impl ::vela_engine::args::IntoScriptArg for #ident {
            fn into_script_arg(self) -> ::vela_vm::owned_value::OwnedValue {
                ::vela_vm::owned_value::OwnedValue::record(
                    #qualified_type_name,
                    #encoded_fields,
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
                Ok(#decoded_value)
            }
        }

    })
}

fn expand_enum(
    ident: &Ident,
    data: &DataEnum,
    type_attrs: ScriptAttrs,
    identity: TypeIdentity,
) -> Result<TokenStream> {
    let variants = collect_variants(data, &identity.stable_path)?;
    let schema_hash = enum_schema_hash(
        &identity.name,
        &identity.module,
        &type_attrs.attrs,
        &type_attrs.traits,
        &variants,
    );
    let type_name = identity.name;
    let module_name = identity.module;
    let qualified_type_name = format!("{module_name}::{type_name}");
    let type_id = identity.type_id;
    let decode_operation = format!("{qualified_type_name} Value decode");
    let docs = type_attrs.docs.map(|docs| quote! { .docs(#docs) });
    let type_attr_tokens = type_attrs.attrs.iter().map(|(name, value)| {
        quote! { desc = desc.attr(#name, #value); }
    });
    let trait_tokens = type_attrs.traits.iter().map(|trait_name| {
        quote! {
            desc = desc.trait_impl(::vela_reflect::registry::TraitDesc::new(#trait_name));
        }
    });
    let variant_descs = variants.iter().map(variant_desc_tokens);
    let encode_arms = variants.iter().map(|variant| {
        let rust_variant = &variant.rust_ident;
        let script_variant = &variant.script_name;
        if variant.fields.is_empty() {
            quote! {
                Self::#rust_variant => ::vela_vm::owned_value::OwnedValue::enum_variant(
                    #qualified_type_name,
                    #script_variant,
                    ::std::vec::Vec::<(&str, ::vela_vm::owned_value::OwnedValue)>::new(),
                )
            }
        } else {
            let rust_fields = variant.fields.iter().map(|field| &field.rust_ident);
            let encoded_fields = variant.fields.iter().map(|field| {
                let rust_field = &field.rust_ident;
                let script_field = &field.script_name;
                quote! {
                    (#script_field, ::vela_engine::args::IntoScriptArg::into_script_arg(#rust_field))
                }
            });
            quote! {
                Self::#rust_variant { #(#rust_fields),* } => {
                    ::vela_vm::owned_value::OwnedValue::enum_variant(
                        #qualified_type_name,
                        #script_variant,
                        [#(#encoded_fields),*],
                    )
                }
            }
        }
    });
    let encode_ref_arms = variants.iter().map(|variant| {
        let rust_variant = &variant.rust_ident;
        let script_variant = &variant.script_name;
        if variant.fields.is_empty() {
            quote! {
                Self::#rust_variant => ::vela_vm::owned_value::OwnedValue::enum_variant(
                    #qualified_type_name,
                    #script_variant,
                    ::std::vec::Vec::<(&str, ::vela_vm::owned_value::OwnedValue)>::new(),
                )
            }
        } else {
            let rust_fields = variant.fields.iter().map(|field| &field.rust_ident);
            let encoded_fields = variant.fields.iter().map(|field| {
                let rust_field = &field.rust_ident;
                let script_field = &field.script_name;
                quote! {
                    (
                        #script_field,
                        ::vela_engine::args::ToScriptValueRef::to_script_value_ref(
                            #rust_field
                        ),
                    )
                }
            });
            quote! {
                Self::#rust_variant { #(#rust_fields),* } => {
                    ::vela_vm::owned_value::OwnedValue::enum_variant(
                        #qualified_type_name,
                        #script_variant,
                        [#(#encoded_fields),*],
                    )
                }
            }
        }
    });
    let decode_arms = variants.iter().map(|variant| {
        let rust_variant = &variant.rust_ident;
        let script_variant = &variant.script_name;
        let field_count = variant.fields.len();
        if variant.fields.is_empty() {
            quote! {
                #script_variant if fields.is_empty() => Ok(Self::#rust_variant)
            }
        } else {
            let decoded_fields = variant.fields.iter().map(|field| {
                let rust_field = &field.rust_ident;
                let script_field = &field.script_name;
                quote! {
                    #rust_field: ::vela_engine::args::FromScriptArg::from_script_arg(
                        fields.get(#script_field).ok_or_else(|| {
                            ::vela_vm::error::VmError::new(
                                ::vela_vm::error::VmErrorKind::TypeMismatch {
                                    operation: #decode_operation,
                                },
                            )
                        })?,
                    )?
                }
            });
            quote! {
                #script_variant if fields.len() == #field_count => {
                    Ok(Self::#rust_variant { #(#decoded_fields),* })
                }
            }
        }
    });
    let dependency_registrations = variants
        .iter()
        .flat_map(|variant| variant.fields.iter())
        .map(|field| {
            let rust_type = &field.rust_type;
            quote! {
                let builder = <#rust_type as ::vela_engine::type_registration::RustValueType>::register_value_type_closure(builder);
            }
        });

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
                .kind(::vela_reflect::registry::TypeKind::ScriptEnum)
                .schema_hash(::vela_reflect::registry::SchemaHash::new(#schema_hash))
                .attr("module", #module_name)
                #docs;
                #(#type_attr_tokens)*
                #(#trait_tokens)*
                #(
                    desc = desc.variant(#variant_descs);
                )*
                desc
            }

            #[must_use]
            pub fn vela_type_binding() -> ::vela_engine::type_binding::TypeBinding<Self> {
                <Self as ::vela_engine::schema::ScriptValueSchema>::script_value_binding()
            }

            #[must_use]
            pub fn vela_type() -> ::vela_engine::registration::TypeRegistration<Self> {
                ::vela_engine::registration::TypeRegistration::of()
            }
        }

        impl ::vela_engine::schema::ScriptValueSchema for #ident {
            fn script_value_type_desc() -> ::vela_reflect::registry::TypeDesc {
                Self::vela_value_type_desc()
            }
        }

        impl ::vela_engine::type_registration::RustValueType for #ident {
            fn register_value_type_closure(
                builder: ::vela_engine::builder::EngineBuilder,
            ) -> ::vela_engine::builder::EngineBuilder {
                #(#dependency_registrations)*
                builder.register_generated_type_binding::<Self>(Self::vela_type_binding())
            }
        }

        impl ::vela_engine::type_registration::VelaType for #ident {
            fn register(
                builder: ::vela_engine::builder::EngineBuilder,
            ) -> ::vela_engine::builder::EngineBuilder {
                <Self as ::vela_engine::type_registration::RustValueType>::
                    register_value_type_closure(builder)
            }
        }

        impl ::vela_engine::interop::VelaValueBoundary for #ident {
            fn vela_type_hint() -> ::vela_engine::native::TypeHint {
                ::vela_engine::native::TypeHint::Enum(
                    Self::vela_value_type_desc().key,
                )
            }
        }

        impl ::vela_engine::args::ToScriptValueRef for #ident {
            fn to_script_value_ref(&self) -> ::vela_vm::owned_value::OwnedValue {
                match self {
                    #(#encode_ref_arms),*
                }
            }
        }

        impl ::vela_engine::args::IntoScriptArg for #ident {
            fn into_script_arg(self) -> ::vela_vm::owned_value::OwnedValue {
                match self {
                    #(#encode_arms),*
                }
            }
        }

        impl ::vela_engine::args::FromScriptArg for #ident {
            const TYPE_NAME: &'static str = #qualified_type_name;

            fn from_script_arg(
                value: &::vela_vm::owned_value::OwnedValue,
            ) -> ::vela_vm::error::VmResult<Self> {
                let ::vela_vm::owned_value::OwnedValue::Enum {
                    enum_name,
                    variant,
                    fields,
                } = value else {
                    return Err(::vela_vm::error::VmError::new(
                        ::vela_vm::error::VmErrorKind::TypeMismatch {
                            operation: #decode_operation,
                        },
                    ));
                };
                if enum_name != #qualified_type_name {
                    return Err(::vela_vm::error::VmError::new(
                        ::vela_vm::error::VmErrorKind::TypeMismatch {
                            operation: #decode_operation,
                        },
                    ));
                }
                match variant.as_str() {
                    #(#decode_arms),*,
                    _ => Err(::vela_vm::error::VmError::new(
                        ::vela_vm::error::VmErrorKind::TypeMismatch {
                            operation: #decode_operation,
                        },
                    )),
                }
            }
        }

    })
}

fn collect_fields(fields: &syn::FieldsNamed, stable_type_path: &str) -> Result<Vec<ValueField>> {
    collect_named_fields(fields, stable_type_path, "value_field")
}

fn collect_variants(data: &DataEnum, stable_type_path: &str) -> Result<Vec<ValueVariant>> {
    let mut seen_names = BTreeSet::new();
    let mut seen_stable_names = BTreeSet::new();
    let mut seen_ids = BTreeSet::new();
    let mut result = Vec::with_capacity(data.variants.len());
    for variant in &data.variants {
        let attrs = parse_script_attrs(&variant.attrs)?;
        if attrs.skip {
            return Err(spanned_error(
                variant,
                "Value variants cannot be skipped because structural encoding must cover every Rust value",
            ));
        }
        let rust_ident = variant.ident.clone();
        let rust_name = rust_ident.to_string();
        let script_name = attrs.field_name(&rust_name);
        if script_name.is_empty() || !seen_names.insert(script_name.clone()) {
            return Err(error(
                rust_ident.span(),
                "Value variant names must be non-empty and unique",
            ));
        }
        let stable_name = attrs.alias.unwrap_or_else(|| script_name.clone());
        if !seen_stable_names.insert(stable_name.clone()) {
            return Err(error(rust_ident.span(), "duplicate Value variant alias"));
        }
        let id = vela_common::stable_id("value_variant", stable_type_path, &stable_name);
        if !seen_ids.insert(id) {
            return Err(error(
                rust_ident.span(),
                "duplicate generated Value variant id",
            ));
        }
        let fields = match &variant.fields {
            Fields::Unit => Vec::new(),
            Fields::Named(fields) => {
                let stable_owner = format!("{stable_type_path}::{stable_name}");
                collect_named_fields(fields, &stable_owner, "value_variant_field")?
            }
            Fields::Unnamed(fields) => {
                return Err(spanned_error(
                    fields,
                    "Value enum variants must be unit variants or have named fields",
                ));
            }
        };
        result.push(ValueVariant {
            rust_ident,
            script_name,
            stable_name,
            id,
            fields,
            docs: attrs.docs,
            attrs: attrs.attrs,
        });
    }
    Ok(result)
}

fn collect_named_fields(
    fields: &syn::FieldsNamed,
    stable_owner: &str,
    id_namespace: &str,
) -> Result<Vec<ValueField>> {
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
        let id = vela_common::stable_id(id_namespace, stable_owner, &stable_name);
        if !seen_ids.insert(id) {
            return Err(error(
                rust_ident.span(),
                "duplicate generated Value field id",
            ));
        }
        let explicit_type_hint = attrs.type_hint.is_some();
        result.push(ValueField {
            rust_ident,
            rust_type: field.ty.clone(),
            script_name,
            stable_name,
            id,
            type_hint: attrs.type_hint.or_else(|| inferred_type_hint(&field.ty)),
            explicit_type_hint,
            docs: attrs.docs,
            attrs: attrs.attrs,
        });
    }
    Ok(result)
}

fn variant_desc_tokens(variant: &ValueVariant) -> TokenStream {
    let id = u128::from(variant.id);
    let script_name = &variant.script_name;
    let rust_name = variant.rust_ident.to_string();
    let docs = variant.docs.as_ref().map(|docs| quote! { .docs(#docs) });
    let attrs = variant.attrs.iter().map(|(name, value)| {
        quote! { .attr(#name, #value) }
    });
    let fields = variant.fields.iter().map(field_desc_tokens);
    quote! {
        ::vela_reflect::registry::VariantDesc::new(
            ::vela_def::VariantId::new(#id),
            #script_name,
        )
        .attr("rust_name", #rust_name)
        #(#attrs)*
        #docs
        #(
            .field(#fields)
        )*
    }
}

fn field_desc_tokens(field: &ValueField) -> TokenStream {
    let id = u128::from(field.id);
    let script_name = &field.script_name;
    let rust_name = field.rust_ident.to_string();
    let hint = if field.explicit_type_hint {
        field
            .type_hint
            .as_ref()
            .map(|hint| quote! { .type_hint(#hint) })
    } else {
        let rust_type = &field.rust_type;
        Some(quote! {
            .type_hint(
                <#rust_type as ::vela_engine::interop::VelaValueBoundary>::
                    vela_type_hint().display_name()
            )
        })
    };
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

fn enum_schema_hash(
    type_name: &str,
    module: &str,
    attrs: &[(String, String)],
    traits: &[String],
    variants: &[ValueVariant],
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
    let mut variants = variants.iter().collect::<Vec<_>>();
    variants.sort_by_key(|variant| (variant.id, variant.script_name.as_str()));
    for variant in variants {
        hasher.write_u64(variant.id);
        hasher.write_str(&variant.script_name);
        hasher.write_str(&variant.stable_name);
        for (name, value) in &variant.attrs {
            hasher.write_str(name);
            hasher.write_str(value);
        }
        let mut fields = variant.fields.iter().collect::<Vec<_>>();
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
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use proc_macro2::TokenStream;
    use quote::quote;
    use rstest::rstest;

    use super::expand_result;

    #[rstest]
    #[case::record(quote! {
        #[vela(path = "host::Record")]
        struct Record { count: i64 }
    })]
    #[case::enum_value(quote! {
        #[vela(path = "host::Decision")]
        enum Decision { Pending, Granted { count: i64 } }
    })]
    fn value_codecs_keep_owned_copies_without_host_or_service_members(#[case] item: TokenStream) {
        let expanded = expand_result(item)
            .expect("Value codec expands")
            .to_string();

        assert!(expanded.contains("VelaValueBoundary for"));
        assert!(expanded.contains("ToScriptValueRef for"));
        assert!(expanded.contains("register_value_type_closure"));
        assert!(expanded.contains("IntoScriptArg for"));
        assert!(expanded.contains("FromScriptArg for"));
        for removed in [
            "VelaSharedBoundary",
            "StoragePolicy",
            "DetachedHostValue",
            "ScriptHostObject",
            "ScriptHostFieldAccess",
            "HostTarget",
            "host_id",
            "register_shared_type_closure",
            "push_shared_service_arg",
            "decode_shared_temporary",
        ] {
            assert!(!expanded.contains(removed), "unexpected member: {removed}");
        }
    }

    #[rstest]
    #[case::fields(quote! { fields })]
    #[case::deref(quote! { deref })]
    #[case::get(quote! { get })]
    #[case::set(quote! { set })]
    #[case::host(quote! { host = "data::Host" })]
    fn rejects_host_projection_attributes_instead_of_serializing_host_wrappers(
        #[case] attribute: TokenStream,
    ) {
        let error = expand_result(quote! {
            #[vela(path = "data::Record")]
            struct Record {
                #[vela(#attribute)]
                count: i64,
            }
        })
        .expect_err("host projections do not become Value codecs");
        assert_eq!(
            error.to_string(),
            "host projection attributes are removed; Value only supports script-owned data"
        );
    }

    #[rstest]
    #[case::skipped_fields(
        quote! {
            #[vela(path = "host::PartialValue")]
            struct PartialValue {
                visible: i64,
                #[vela(skip)]
                hidden: i64,
            }
        },
        "Value fields cannot be skipped because structural decoding must reconstruct the exact Rust value"
    )]
    #[case::generic_structs(
        quote! {
            #[vela(path = "host::Envelope")]
            struct Envelope<T> {
                value: T,
            }
        },
        "Value does not support generic Rust types; register a concrete manual TypeBinding"
    )]
    #[case::skipped_variants(
        quote! {
            #[vela(path = "host::State")]
            enum State {
                Ready,
                #[vela(skip)]
                Hidden,
            }
        },
        "Value variants cannot be skipped because structural encoding must cover every Rust value"
    )]
    #[case::tuple_variants(
        quote! {
            #[vela(path = "host::State")]
            enum State {
                Ready(i64),
            }
        },
        "Value enum variants must be unit variants or have named fields"
    )]
    fn rejects_unsupported_value_shapes(#[case] input: TokenStream, #[case] expected: &str) {
        let error = expand_result(input).expect_err("unsupported structural Value");
        assert_eq!(error.to_string(), expected);
    }
}
