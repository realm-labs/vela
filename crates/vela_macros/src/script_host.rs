use std::collections::BTreeSet;

use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Result, parse2};

use crate::attrs::{error, inferred_type_hint, parse_script_attrs, spanned_error};
use crate::hash::StableHasher;

#[derive(Clone, Copy)]
pub(crate) enum GeneratedMethod {
    Host,
    Reflect,
}

impl GeneratedMethod {
    fn ident(self) -> Ident {
        match self {
            Self::Host => format_ident!("vela_host_type_desc"),
            Self::Reflect => format_ident!("vela_reflect_type_desc"),
        }
    }

    fn trait_impl_tokens(self, ident: &Ident, method: &Ident) -> TokenStream {
        match self {
            Self::Host => quote! {
                impl ::vela_engine::ScriptHostSchema for #ident {
                    fn script_host_type_desc() -> ::vela_reflect::TypeDesc {
                        Self::#method()
                    }
                }
            },
            Self::Reflect => quote! {
                impl ::vela_engine::ScriptReflectSchema for #ident {
                    fn script_reflect_type_desc() -> ::vela_reflect::TypeDesc {
                        Self::#method()
                    }
                }
            },
        }
    }
}

#[derive(Clone, Debug)]
struct FieldMeta {
    rust_name: String,
    script_name: String,
    id: u32,
    readable: bool,
    writable: bool,
    type_hint: Option<String>,
    docs: Option<String>,
    permissions: Vec<String>,
}

pub(crate) fn expand(input: TokenStream, generated_method: GeneratedMethod) -> TokenStream {
    match expand_result(input, generated_method) {
        Ok(tokens) => tokens,
        Err(error) => error.to_compile_error(),
    }
}

fn expand_result(input: TokenStream, generated_method: GeneratedMethod) -> Result<TokenStream> {
    let input = parse2::<DeriveInput>(input)?;
    if !input.generics.params.is_empty() || input.generics.where_clause.is_some() {
        return Err(spanned_error(
            &input.generics,
            "ScriptHost and ScriptReflect do not support generic host schemas",
        ));
    }
    let attrs = parse_script_attrs(&input.attrs)?;
    let type_id = attrs
        .id
        .ok_or_else(|| error(input.ident.span(), "ScriptHost requires #[script(id = N)]"))?;
    let host_id = attrs.host_id.unwrap_or(type_id);
    let type_name = attrs.name.unwrap_or_else(|| input.ident.to_string());
    let module_name = attrs.module;
    let docs = attrs.docs;
    let fields = collect_fields(&input)?;
    let schema_hash = schema_hash(&type_name, module_name.as_deref(), &fields);

    let ident = input.ident;
    let method = generated_method.ident();
    let trait_impl = generated_method.trait_impl_tokens(&ident, &method);
    let module_tokens = module_name.map(|module| quote! { .attr("module", #module) });
    let docs_tokens = docs.map(|docs| quote! { .docs(#docs) });
    let field_tokens = fields.iter().map(field_tokens);
    let field_helper_tokens = match generated_method {
        GeneratedMethod::Host => {
            let helpers = fields.iter().map(field_helper_tokens);
            quote! { #(#helpers)* }
        }
        GeneratedMethod::Reflect => quote! {},
    };

    Ok(quote! {
        impl #ident {
            #[must_use]
            pub fn #method() -> ::vela_reflect::TypeDesc {
                let mut desc = ::vela_reflect::TypeDesc::new(
                    ::vela_reflect::TypeKey::new(
                        ::vela_common::TypeId::new(#type_id),
                        #type_name,
                    ),
                )
                .kind(::vela_reflect::TypeKind::Host)
                .schema_hash(::vela_reflect::SchemaHash::new(#schema_hash))
                .host_type(::vela_common::HostTypeId::new(#host_id))
                #module_tokens
                #docs_tokens;
                #(
                    desc = desc.field(#field_tokens);
                )*
                desc
            }

            #field_helper_tokens
        }

        #trait_impl
    })
}

fn collect_fields(input: &DeriveInput) -> Result<Vec<FieldMeta>> {
    let Data::Struct(data) = &input.data else {
        return Err(spanned_error(input, "ScriptHost only supports structs"));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(spanned_error(
            input,
            "ScriptHost requires named struct fields",
        ));
    };

    let mut seen_ids = BTreeSet::new();
    let mut result = Vec::new();
    for field in &fields.named {
        let attrs = parse_script_attrs(&field.attrs)?;
        if attrs.skip || !attrs.has_script_attr {
            continue;
        }
        let ident = field
            .ident
            .as_ref()
            .ok_or_else(|| spanned_error(field, "ScriptHost requires named struct fields"))?;
        let id = attrs.id.ok_or_else(|| {
            error(
                ident.span(),
                "script-exposed fields require #[script(id = N)]",
            )
        })?;
        if !seen_ids.insert(id) {
            return Err(error(ident.span(), "duplicate script field id"));
        }

        let rust_name = ident.to_string();
        result.push(FieldMeta {
            script_name: attrs.field_name(&rust_name),
            rust_name,
            id,
            readable: attrs.get,
            writable: attrs.set,
            type_hint: attrs.type_hint.or_else(|| inferred_type_hint(&field.ty)),
            docs: attrs.docs,
            permissions: attrs.permissions,
        });
    }

    Ok(result)
}

fn field_tokens(field: &FieldMeta) -> TokenStream {
    let id = field.id;
    let script_name = &field.script_name;
    let rust_name = &field.rust_name;
    let readable = field.readable;
    let writable = field.writable;
    let permission_tokens = field
        .permissions
        .iter()
        .map(|permission| quote! { .require_permission(#permission) });
    let hint_tokens = field
        .type_hint
        .as_ref()
        .map(|hint| quote! { .type_hint(#hint) });
    let docs_tokens = field.docs.as_ref().map(|docs| quote! { .docs(#docs) });

    quote! {
        ::vela_reflect::FieldDesc::new(::vela_common::FieldId::new(#id), #script_name)
            .access(
                ::vela_reflect::FieldAccess::new()
                    .readable(#readable)
                    .writable(#writable)
                    .reflect_readable(#readable)
                    .reflect_writable(#writable)
                    #(#permission_tokens)*
            )
            .attr("rust_name", #rust_name)
            #hint_tokens
            #docs_tokens
    }
}

fn field_helper_tokens(field: &FieldMeta) -> TokenStream {
    let id = field.id;
    let field_id_ident = format_ident!("vela_field_id_{}", field.rust_name);
    let field_path_ident = format_ident!("vela_field_path_{}", field.rust_name);

    quote! {
        #[must_use]
        pub const fn #field_id_ident() -> ::vela_engine::FieldId {
            ::vela_engine::FieldId::new(#id)
        }

        #[must_use]
        pub fn #field_path_ident(host_ref: ::vela_engine::HostRef) -> ::vela_engine::HostPath {
            ::vela_engine::HostPath::new(host_ref).field(Self::#field_id_ident())
        }
    }
}

fn schema_hash(type_name: &str, module_name: Option<&str>, fields: &[FieldMeta]) -> u64 {
    let mut hasher = StableHasher::new();
    hasher.write_str(type_name);
    if let Some(module_name) = module_name {
        hasher.write_str(module_name);
    }
    for field in fields {
        hasher.write_u32(field.id);
        hasher.write_str(&field.script_name);
        hasher.write_bool(field.readable);
        hasher.write_bool(field.writable);
        hasher.write_str(field.type_hint.as_deref().unwrap_or(""));
        for permission in &field.permissions {
            hasher.write_str(permission);
        }
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use quote::quote;

    use super::{GeneratedMethod, expand_result};

    #[test]
    fn rejects_duplicate_field_ids() {
        let error = expand_result(
            quote! {
                #[script(id = 100)]
                struct Player {
                    #[script(get, id = 1)]
                    level: u32,
                    #[script(get, id = 1)]
                    exp: u64,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("duplicate field IDs should fail macro expansion");

        assert!(error.to_string().contains("duplicate script field id"));
    }

    #[test]
    fn rejects_missing_type_id() {
        let error = expand_result(
            quote! {
                struct Player {
                    #[script(get, id = 1)]
                    level: u32,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("missing type ID should fail macro expansion");

        assert!(error.to_string().contains("requires #[script(id = N)]"));
    }

    #[test]
    fn infers_fixed_array_field_type_hints() {
        let tokens = expand_result(
            quote! {
                #[script(id = 100)]
                struct SpawnTable {
                    #[script(get, id = 1)]
                    weights: [i64; 3],
                }
            },
            GeneratedMethod::Host,
        )
        .expect("fixed array host schema should expand")
        .to_string();

        assert!(tokens.contains("type_hint (\"array\")"));
    }

    #[test]
    fn rejects_generic_host_schemas() {
        let error = expand_result(
            quote! {
                #[script(id = 100)]
                struct Inventory<T>
                where
                    T: Clone,
                {
                    #[script(get, id = 1)]
                    items: Vec<T>,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("generic host schema should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("do not support generic host schemas")
        );
    }
}
