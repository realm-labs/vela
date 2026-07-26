use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    Ident, LitStr, Result, Token, Type, bracketed,
    parse::{Parse, ParseStream},
    parse2,
    punctuated::Punctuated,
};

use crate::attrs::parse_qualified_name;
use crate::script_host::type_identity;

pub(crate) fn expand(input: TokenStream) -> TokenStream {
    match expand_result(input) {
        Ok(tokens) => tokens,
        Err(error) => error.to_compile_error(),
    }
}

fn expand_result(input: TokenStream) -> Result<TokenStream> {
    let input = parse2::<ExternalValueEnum>(input)?;
    let rust_ident = type_ident(&input.ty)?;
    let identity = type_identity(&rust_ident, Some(input.path), None, None, None)?;
    let type_name = identity.name;
    let module_name = identity.module;
    let stable_path = identity.stable_path;
    let qualified_name = format!("{module_name}::{type_name}");
    let type_id = identity.type_id;
    let ty = input.ty;
    let variants = input.variants.into_iter().collect::<Vec<_>>();
    let variant_names = variants.iter().map(ToString::to_string).collect::<Vec<_>>();
    let variant_ids = variant_names
        .iter()
        .map(|name| u128::from(vela_common::stable_id("value_variant", &stable_path, name)))
        .collect::<Vec<_>>();
    let schema_hash = enum_schema_hash(&type_name, &module_name, &variant_names, &variant_ids);
    let decode_operation = format!("{qualified_name} Value decode");

    Ok(quote! {
        impl ::vela_engine::schema::ScriptValueSchema for #ty {
            fn script_value_type_desc() -> ::vela_reflect::registry::TypeDesc {
                let mut desc = ::vela_reflect::registry::TypeDesc::new(
                    ::vela_reflect::registry::TypeKey::new(
                        ::vela_def::TypeId::new(#type_id),
                        #qualified_name,
                    ),
                )
                .kind(::vela_reflect::registry::TypeKind::ScriptEnum)
                .schema_hash(::vela_reflect::registry::SchemaHash::new(#schema_hash))
                .attr("module", #module_name);
                #(
                    desc = desc.variant(
                        ::vela_reflect::registry::VariantDesc::new(
                            ::vela_def::VariantId::new(#variant_ids),
                            #variant_names,
                        )
                        .attr("rust_name", #variant_names),
                    );
                )*
                desc
            }
        }

        impl ::vela_engine::type_registration::RustValueType for #ty {
            fn register_value_type_closure(
                builder: ::vela_engine::builder::EngineBuilder,
            ) -> ::vela_engine::builder::EngineBuilder {
                builder.register_generated_type_binding::<Self>(
                    <Self as ::vela_engine::schema::ScriptValueSchema>::
                        script_value_binding(),
                )
            }
        }

        impl ::vela_engine::type_registration::VelaType for #ty {
            fn register(
                builder: ::vela_engine::builder::EngineBuilder,
            ) -> ::vela_engine::builder::EngineBuilder {
                <Self as ::vela_engine::type_registration::RustValueType>::
                    register_value_type_closure(builder)
            }
        }

        impl ::vela_engine::interop::VelaValueBoundary for #ty {
            fn vela_type_hint() -> ::vela_engine::native::TypeHint {
                ::vela_engine::native::TypeHint::Enum(
                    <Self as ::vela_engine::schema::ScriptValueSchema>::
                        script_value_type_desc().key,
                )
            }
        }

        impl ::vela_engine::interop::VelaValueKeyBoundary for #ty {}

        impl ::vela_engine::args::ToScriptValueRef for #ty {
            fn to_script_value_ref(&self) -> ::vela_vm::owned_value::OwnedValue {
                match self {
                    #(
                        Self::#variants => ::vela_vm::owned_value::OwnedValue::enum_variant(
                            #qualified_name,
                            #variant_names,
                            ::std::iter::empty::<(
                                &'static str,
                                ::vela_vm::owned_value::OwnedValue,
                            )>(),
                        ),
                    )*
                }
            }
        }

        impl ::vela_engine::interop::VelaSharedBoundary for #ty {
            const STORAGE: ::vela_common::StoragePolicy =
                ::vela_common::StoragePolicy::Value;

            fn vela_shared_type_hint() -> ::vela_engine::native::TypeHint {
                <Self as ::vela_engine::interop::VelaValueBoundary>::vela_type_hint()
            }

            fn register_shared_type_closure(
                builder: ::vela_engine::builder::EngineBuilder,
            ) -> ::vela_engine::builder::EngineBuilder {
                <Self as ::vela_engine::type_registration::RustValueType>::
                    register_value_type_closure(builder)
            }

            fn push_shared_service_arg<'a>(
                &'a self,
                args: &mut ::vela_engine::runtime::CallArgs<'a>,
            ) {
                args.push(
                    <Self as ::vela_engine::args::ToScriptValueRef>::
                        to_script_value_ref(self),
                );
            }

            fn decode_shared_temporary(
                value: &::vela_vm::owned_value::OwnedValue,
            ) -> ::vela_vm::error::VmResult<Self> {
                <Self as ::vela_engine::args::FromScriptArg>::from_script_arg(value)
            }
        }

        impl ::vela_engine::args::IntoScriptArg for #ty {
            fn into_script_arg(self) -> ::vela_vm::owned_value::OwnedValue {
                match self {
                    #(
                        Self::#variants => ::vela_vm::owned_value::OwnedValue::enum_variant(
                            #qualified_name,
                            #variant_names,
                            ::std::iter::empty::<(
                                &'static str,
                                ::vela_vm::owned_value::OwnedValue,
                            )>(),
                        ),
                    )*
                }
            }
        }

        impl ::vela_engine::args::FromScriptArg for #ty {
            const TYPE_NAME: &'static str = #qualified_name;

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
                if enum_name != #qualified_name || !fields.is_empty() {
                    return Err(::vela_vm::error::VmError::new(
                        ::vela_vm::error::VmErrorKind::TypeMismatch {
                            operation: #decode_operation,
                        },
                    ));
                }
                match variant.as_str() {
                    #(
                        #variant_names => Ok(Self::#variants),
                    )*
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

struct ExternalValueEnum {
    path: String,
    ty: Type,
    variants: Punctuated<Ident, Token![,]>,
}

impl Parse for ExternalValueEnum {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        expect_key(input, "path")?;
        input.parse::<Token![=]>()?;
        let path = parse_qualified_name(input.parse::<LitStr>()?, "external Value path")?;
        input.parse::<Token![,]>()?;
        expect_key(input, "ty")?;
        input.parse::<Token![=]>()?;
        let ty = input.parse::<Type>()?;
        input.parse::<Token![,]>()?;
        expect_key(input, "variants")?;
        input.parse::<Token![=]>()?;
        let content;
        bracketed!(content in input);
        let variants = content.parse_terminated(Ident::parse, Token![,])?;
        if variants.is_empty() {
            return Err(content.error("external Value enum requires at least one variant"));
        }
        if !input.is_empty() {
            let _ = input.parse::<Token![,]>();
        }
        if !input.is_empty() {
            return Err(input.error("unexpected external Value enum input"));
        }
        Ok(Self { path, ty, variants })
    }
}

fn expect_key(input: ParseStream<'_>, expected: &str) -> Result<()> {
    let key = input.parse::<Ident>()?;
    if key != expected {
        return Err(syn::Error::new(
            key.span(),
            format!("expected `{expected}`"),
        ));
    }
    Ok(())
}

fn type_ident(ty: &Type) -> Result<Ident> {
    let Type::Path(path) = ty else {
        return Err(syn::Error::new_spanned(
            ty,
            "external Value enum must be a named type",
        ));
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.clone())
        .ok_or_else(|| syn::Error::new_spanned(ty, "missing enum type name"))
}

fn enum_schema_hash(type_name: &str, module_name: &str, variants: &[String], ids: &[u128]) -> u64 {
    let mut hasher = crate::hash::StableHasher::new();
    hasher.write_str(type_name);
    hasher.write_str(module_name);
    let mut entries = ids.iter().copied().zip(variants).collect::<Vec<_>>();
    entries.sort_by_key(|(id, name)| (*id, name.as_str()));
    for (id, name) in entries {
        hasher.write_u64(id as u64);
        hasher.write_str(name);
        hasher.write_str(name);
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use quote::quote;

    use super::expand_result;

    #[test]
    fn external_unit_enum_generates_traits_without_an_inherent_impl() {
        let expanded = expand_result(quote! {
            path = "config::Quality",
            ty = crate::Quality,
            variants = [Normal, Rare],
        })
        .expect("external unit enum expands")
        .to_string();

        assert!(expanded.contains("ScriptValueSchema for crate :: Quality"));
        assert!(expanded.contains("Self :: Normal"));
        assert!(!expanded.contains("impl crate :: Quality {"));
    }
}
