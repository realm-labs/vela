mod emission;
mod schema;

use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::{DeriveInput, Result, parse2};

use crate::attrs::{error, parse_script_attrs, spanned_error};

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
                impl ::vela_engine::schema::ScriptHostSchema for #ident {
                    fn script_host_type_desc() -> ::vela_reflect::registry::TypeDesc {
                        Self::#method()
                    }
                }
            },
            Self::Reflect => quote! {
                impl ::vela_engine::schema::ScriptReflectSchema for #ident {
                    fn script_reflect_type_desc() -> ::vela_reflect::registry::TypeDesc {
                        Self::#method()
                    }
                }
            },
        }
    }
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
    if type_name.is_empty() {
        return Err(error(
            input.ident.span(),
            "script type name cannot be empty",
        ));
    }
    let module_name = attrs.module;
    let docs = attrs.docs;
    let type_attrs = attrs.attrs;
    let fields = schema::collect_fields(&input)?;
    let schema_hash = schema::schema_hash(&type_name, module_name.as_deref(), &type_attrs, &fields);

    let ident = input.ident;
    let method = generated_method.ident();
    let trait_impl = generated_method.trait_impl_tokens(&ident, &method);
    let module_tokens = module_name.map(|module| quote! { .attr("module", #module) });
    let docs_tokens = docs.map(|docs| quote! { .docs(#docs) });
    let type_attr_tokens = type_attrs.iter().map(|(name, value)| {
        quote! {
            desc = desc.attr(#name, #value);
        }
    });
    let field_tokens = fields.iter().map(emission::field_tokens);
    let field_helper_tokens = match generated_method {
        GeneratedMethod::Host => {
            let helpers = fields.iter().map(emission::field_helper_tokens);
            quote! { #(#helpers)* }
        }
        GeneratedMethod::Reflect => quote! {},
    };

    Ok(quote! {
        impl #ident {
            #[must_use]
            pub fn #method() -> ::vela_reflect::registry::TypeDesc {
                let mut desc = ::vela_reflect::registry::TypeDesc::new(
                    ::vela_reflect::registry::TypeKey::new(
                        ::vela_common::TypeId::new(#type_id),
                        #type_name,
                    ),
                )
                .kind(::vela_reflect::registry::TypeKind::Host)
                .schema_hash(::vela_reflect::registry::SchemaHash::new(#schema_hash))
                .host_type(::vela_common::HostTypeId::new(#host_id))
                #module_tokens
                #docs_tokens;
                #(#type_attr_tokens)*
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
    fn rejects_duplicate_field_names() {
        let error = expand_result(
            quote! {
                #[script(id = 100)]
                struct Player {
                    #[script(get, id = 1, name = "level")]
                    level: u32,
                    #[script(get, id = 2, name = "level")]
                    exp: u64,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("duplicate field names should fail macro expansion");

        assert!(error.to_string().contains("duplicate script field name"));
    }

    #[test]
    fn rejects_empty_field_names() {
        let error = expand_result(
            quote! {
                #[script(id = 100)]
                struct Player {
                    #[script(get, id = 1, name = "")]
                    level: u32,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("empty field name should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("script field name cannot be empty")
        );
    }

    #[test]
    fn rejects_empty_field_permissions() {
        let error = expand_result(
            quote! {
                #[script(id = 100)]
                struct Player {
                    #[script(get, id = 1, permission = "")]
                    level: u32,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("empty field permission should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("script permission cannot be empty")
        );
    }

    #[test]
    fn rejects_empty_field_type_hints() {
        let error = expand_result(
            quote! {
                #[script(id = 100)]
                struct Player {
                    #[script(get, id = 1, hint = "")]
                    inventory: Vec<String>,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("empty field type hint should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("script type hint must be a non-generic dotted name")
        );
    }

    #[test]
    fn rejects_generic_field_type_hints() {
        let error = expand_result(
            quote! {
                #[script(id = 100)]
                struct Player {
                    #[script(get, id = 1, hint = "Array<Item>")]
                    inventory: Vec<String>,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("generic field type hint should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("script type hint must be a non-generic dotted name")
        );
    }

    #[test]
    fn rejects_malformed_field_type_hints() {
        let error = expand_result(
            quote! {
                #[script(id = 100)]
                struct Player {
                    #[script(get, id = 1, type = "game..Inventory")]
                    inventory: Vec<String>,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("malformed field type hint should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("script type hint must be a non-generic dotted name")
        );
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
    fn rejects_empty_type_names() {
        let error = expand_result(
            quote! {
                #[script(id = 100, name = "")]
                struct Player {
                    #[script(get, id = 1)]
                    level: u32,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("empty type name should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("script type name cannot be empty")
        );
    }

    #[test]
    fn rejects_empty_module_names() {
        let error = expand_result(
            quote! {
                #[script(id = 100, module = "")]
                struct Player {
                    #[script(get, id = 1)]
                    level: u32,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("empty module name should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("script module must be a non-empty dotted name")
        );
    }

    #[test]
    fn rejects_malformed_module_names() {
        for module in [".game", "game.", "game..player"] {
            let error = expand_result(
                quote! {
                    #[script(id = 100, module = #module)]
                    struct Player {
                        #[script(get, id = 1)]
                        level: u32,
                    }
                },
                GeneratedMethod::Host,
            )
            .expect_err("malformed module name should fail macro expansion");

            assert!(
                error
                    .to_string()
                    .contains("script module must be a non-empty dotted name")
            );
        }
    }

    #[test]
    fn rejects_malformed_static_attrs() {
        let error = expand_result(
            quote! {
                #[script(id = 100, attr = "gameplay")]
                struct Player {
                    #[script(get, id = 1)]
                    level: u32,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("malformed attrs should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("script attr metadata must use `key=value`")
        );
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
