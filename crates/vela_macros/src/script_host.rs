mod emission;
mod schema;

use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Result, parse2};

use crate::attrs::{error, parse_script_attrs, spanned_error};

struct TypeIdentity {
    name: String,
    module: String,
    stable_path: String,
    type_id: u64,
    host_id: u64,
}

struct EnumExpansion {
    input: DeriveInput,
    generated_method: GeneratedMethod,
    type_id: u64,
    host_id: u64,
    type_name: String,
    module_name: String,
    stable_path: String,
    docs: Option<String>,
    type_attrs: Vec<(String, String)>,
    trait_names: Vec<String>,
}

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
    let type_identity = type_identity(
        &input.ident,
        attrs.path,
        attrs.module,
        attrs.name,
        attrs.alias,
    )?;
    let type_id = type_identity.type_id;
    let host_id = type_identity.host_id;
    let type_name = type_identity.name;
    let module_name = type_identity.module;
    let stable_path = type_identity.stable_path;
    let docs = attrs.docs;
    let type_attrs = attrs.attrs;
    let trait_names = attrs.traits;
    if matches!(input.data, Data::Enum(_)) {
        return expand_enum_result(EnumExpansion {
            input,
            generated_method,
            type_id,
            host_id,
            type_name,
            module_name,
            stable_path,
            docs,
            type_attrs,
            trait_names,
        });
    }
    let fields = schema::collect_fields(&input, &stable_path)?;
    let schema_hash = schema::schema_hash(
        &type_name,
        Some(&module_name),
        &type_attrs,
        &trait_names,
        &fields,
    );

    let ident = input.ident;
    let method = generated_method.ident();
    let trait_impl = generated_method.trait_impl_tokens(&ident, &method);
    let module_tokens = quote! { .attr("module", #module_name) };
    let docs_tokens = docs.map(|docs| quote! { .docs(#docs) });
    let type_attr_tokens = type_attrs.iter().map(|(name, value)| {
        quote! {
            desc = desc.attr(#name, #value);
        }
    });
    let trait_tokens = trait_names.iter().map(|trait_name| {
        quote! {
            desc = desc.trait_impl(::vela_reflect::registry::TraitDesc::new(#trait_name));
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
    let type_helper_tokens = match generated_method {
        GeneratedMethod::Host => quote! {
            #[must_use]
            pub const fn vela_type_id() -> ::vela_common::TypeId {
                ::vela_common::TypeId::new(#type_id)
            }

            #[must_use]
            pub const fn vela_host_type_id() -> ::vela_common::HostTypeId {
                ::vela_common::HostTypeId::new(#host_id)
            }

            #[must_use]
            pub const fn vela_stable_type_path() -> &'static str {
                #stable_path
            }
        },
        GeneratedMethod::Reflect => quote! {},
    };

    Ok(quote! {
        impl #ident {
            #type_helper_tokens

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
                #(#trait_tokens)*
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

fn expand_enum_result(expansion: EnumExpansion) -> Result<TokenStream> {
    let EnumExpansion {
        input,
        generated_method,
        type_id,
        host_id,
        type_name,
        module_name,
        stable_path,
        docs,
        type_attrs,
        trait_names,
    } = expansion;
    if matches!(generated_method, GeneratedMethod::Host) {
        return Err(spanned_error(
            &input,
            "ScriptHost enum schemas are not supported; use ScriptReflect for enum metadata",
        ));
    }
    let variants = schema::collect_variants(&input, &type_name, &stable_path)?;
    let schema_hash = schema::enum_schema_hash(
        &type_name,
        Some(&module_name),
        &type_attrs,
        &trait_names,
        &variants,
    );

    let ident = input.ident;
    let method = generated_method.ident();
    let trait_impl = generated_method.trait_impl_tokens(&ident, &method);
    let module_tokens = quote! { .attr("module", #module_name) };
    let docs_tokens = docs.map(|docs| quote! { .docs(#docs) });
    let type_attr_tokens = type_attrs.iter().map(|(name, value)| {
        quote! {
            desc = desc.attr(#name, #value);
        }
    });
    let trait_tokens = trait_names.iter().map(|trait_name| {
        quote! {
            desc = desc.trait_impl(::vela_reflect::registry::TraitDesc::new(#trait_name));
        }
    });
    let variant_tokens = variants.iter().map(emission::variant_tokens);

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
                #(#trait_tokens)*
                #(
                    desc = desc.variant(#variant_tokens);
                )*
                desc
            }
        }

        #trait_impl
    })
}

fn type_identity(
    ident: &Ident,
    path: Option<String>,
    module_attr: Option<String>,
    name_attr: Option<String>,
    alias: Option<String>,
) -> Result<TypeIdentity> {
    let (module, name) = match path {
        Some(path) => {
            let (module, path_name) = split_type_path(&path, ident)?;
            if let Some(module_attr) = module_attr
                && module_attr != module
            {
                return Err(error(ident.span(), "script path and module disagree"));
            }
            if let Some(name_attr) = name_attr
                && name_attr != path_name
            {
                return Err(error(ident.span(), "script path and name disagree"));
            }
            (module, path_name)
        }
        None => {
            let module = module_attr.ok_or_else(|| {
                error(
                    ident.span(),
                    "ScriptHost requires #[script(path = \"module::Type\")] or #[script(module = \"module\")]",
                )
            })?;
            let name = name_attr.unwrap_or_else(|| ident.to_string());
            if name.is_empty() {
                return Err(error(ident.span(), "script type name cannot be empty"));
            }
            (module, name)
        }
    };
    let current_path = format!("{module}::{name}");
    let stable_path = alias
        .map(|alias| {
            if alias.contains("::") {
                alias
            } else {
                format!("{module}::{alias}")
            }
        })
        .unwrap_or_else(|| current_path.clone());
    let type_id = vela_common::stable_id("host_type", "", &stable_path);
    let host_id = vela_common::stable_id("host_ref_type", "", &stable_path);
    Ok(TypeIdentity {
        name,
        module,
        stable_path,
        type_id,
        host_id,
    })
}

fn split_type_path(path: &str, ident: &Ident) -> Result<(String, String)> {
    let Some((module, name)) = path.rsplit_once("::") else {
        return Err(error(
            ident.span(),
            "script path must include a module and type name",
        ));
    };
    if module.is_empty() || name.is_empty() {
        return Err(error(
            ident.span(),
            "script path must include a module and type name",
        ));
    }
    Ok((module.to_owned(), name.to_owned()))
}

#[cfg(test)]
mod tests {
    use quote::quote;

    use super::{GeneratedMethod, expand_result};

    #[test]
    fn rejects_duplicate_field_aliases() {
        let error = expand_result(
            quote! {
                #[script(path = "game::player::Player")]
                struct Player {
                    #[script(get, alias = "score")]
                    level: u32,
                    #[script(get, alias = "score")]
                    exp: u64,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("duplicate field aliases should fail macro expansion");

        assert!(error.to_string().contains("duplicate script field alias"));
    }

    #[test]
    fn rejects_duplicate_field_names() {
        let error = expand_result(
            quote! {
                #[script(path = "game::player::Player")]
                struct Player {
                    #[script(get, name = "level")]
                    level: u32,
                    #[script(get, name = "level")]
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
                #[script(path = "game::player::Player")]
                struct Player {
                    #[script(get, name = "")]
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
                #[script(path = "game::player::Player")]
                struct Player {
                    #[script(get, permission = "")]
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
    fn rejects_duplicate_type_attrs() {
        let error = expand_result(
            quote! {
                #[script(path = "game::player::Player", attr = "domain=gameplay", attr = "domain=combat")]
                struct Player {
                    #[script(get)]
                    level: u32,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("duplicate type attr keys should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("script attr metadata key `domain` is duplicated")
        );
    }

    #[test]
    fn rejects_duplicate_field_attrs() {
        let error = expand_result(
            quote! {
                #[script(path = "game::player::Player")]
                struct Player {
                    #[script(get, attr = "unit=level", attr = "unit=rank")]
                    level: u32,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("duplicate field attr keys should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("script attr metadata key `unit` is duplicated")
        );
    }

    #[test]
    fn rejects_empty_field_type_hints() {
        let error = expand_result(
            quote! {
                #[script(path = "game::player::Player")]
                struct Player {
                    #[script(get, hint = "")]
                    inventory: Vec<String>,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("empty field type hint should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("script type hint must be a non-generic `::` qualified name")
        );
    }

    #[test]
    fn rejects_generic_field_type_hints() {
        let error = expand_result(
            quote! {
                #[script(path = "game::player::Player")]
                struct Player {
                    #[script(get, hint = "Array<Item>")]
                    inventory: Vec<String>,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("generic field type hint should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("script type hint must be a non-generic `::` qualified name")
        );
    }

    #[test]
    fn rejects_malformed_field_type_hints() {
        let error = expand_result(
            quote! {
                #[script(path = "game::player::Player")]
                struct Player {
                    #[script(get, type = "game::::Inventory")]
                    inventory: Vec<String>,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("malformed field type hint should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("script type hint must be a non-generic `::` qualified name")
        );
    }

    #[test]
    fn rejects_missing_type_path() {
        let error = expand_result(
            quote! {
                struct Player {
                    #[script(get)]
                    level: u32,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("missing type path should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("requires #[script(path = \"module::Type\")]")
        );
    }

    #[test]
    fn rejects_empty_type_names() {
        let error = expand_result(
            quote! {
                #[script(module = "game::player", name = "")]
                struct Player {
                    #[script(get)]
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
                #[script(module = "")]
                struct Player {
                    #[script(get)]
                    level: u32,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("empty module name should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("script module must be a non-empty `::` qualified name")
        );
    }

    #[test]
    fn rejects_malformed_module_names() {
        for module in ["::game", "game::", "game::::player", "game.player"] {
            let error = expand_result(
                quote! {
                    #[script(module = #module)]
                    struct Player {
                        #[script(get)]
                        level: u32,
                    }
                },
                GeneratedMethod::Host,
            )
            .expect_err("malformed module name should fail macro expansion");

            assert!(
                error
                    .to_string()
                    .contains("script module must be a non-empty `::` qualified name")
            );
        }
    }

    #[test]
    fn rejects_malformed_static_attrs() {
        let error = expand_result(
            quote! {
                #[script(path = "game::player::Player", attr = "gameplay")]
                struct Player {
                    #[script(get)]
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
                #[script(path = "game::spawn::SpawnTable")]
                struct SpawnTable {
                    #[script(get)]
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
                #[script(path = "game::inventory::Inventory")]
                struct Inventory<T>
                where
                    T: Clone,
                {
                    #[script(get)]
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
