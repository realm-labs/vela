mod emission;
mod schema;

use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Result, parse2};

use crate::attrs::{error, parse_script_attrs, spanned_error};

pub(crate) struct TypeIdentity {
    pub(crate) name: String,
    pub(crate) module: String,
    pub(crate) stable_path: String,
    pub(crate) type_id: u128,
    pub(crate) host_id: u64,
}

struct EnumExpansion {
    input: DeriveInput,
    generated_method: GeneratedMethod,
    type_id: u128,
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

    fn trait_impl_tokens(
        self,
        ident: &Ident,
        method: &Ident,
        registration_types: &[TokenStream],
    ) -> TokenStream {
        match self {
            Self::Host => quote! {
                impl ::vela_engine::schema::ScriptHostSchema for #ident {
                    fn script_host_type_desc() -> ::vela_reflect::registry::TypeDesc {
                        Self::#method()
                    }
                }

                impl ::vela_engine::type_registration::VelaType for #ident {
                    fn register(
                        builder: ::vela_engine::builder::EngineBuilder,
                    ) -> ::vela_engine::builder::EngineBuilder {
                        let builder = builder.register_generated_type_binding::<Self>(
                            <Self as ::vela_engine::schema::ScriptHostSchema>::
                                script_host_binding(),
                        );
                        #(
                            let builder = builder.register_type_dependency::<#registration_types>();
                        )*
                        builder
                    }
                }
            },
            Self::Reflect => quote! {
                impl ::vela_engine::schema::ScriptReflectSchema for #ident {
                    fn script_reflect_type_desc() -> ::vela_reflect::registry::TypeDesc {
                        Self::#method()
                    }
                }

                impl ::vela_engine::interop::VelaValueBoundary for #ident {
                    fn vela_type_hint() -> ::vela_engine::native::TypeHint {
                        let desc = Self::#method();
                        match desc.kind {
                            ::vela_reflect::registry::TypeKind::ScriptEnum => {
                                ::vela_engine::native::TypeHint::Enum(desc.key)
                            }
                            _ => ::vela_engine::native::TypeHint::Record(desc.key),
                        }
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
    let expose_all_fields = attrs.fields;
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
    let fields = schema::collect_fields(&input, &stable_path, expose_all_fields)?;
    let schema_hash = schema::schema_hash(
        &type_name,
        Some(&module_name),
        &type_attrs,
        &trait_names,
        &fields,
    );

    let ident = input.ident;
    let method = generated_method.ident();
    let registration_types = schema::registration_types(&fields);
    let trait_impl = generated_method.trait_impl_tokens(&ident, &method, &registration_types);
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
    let dynamic_field_type_hints = matches!(generated_method, GeneratedMethod::Host);
    let field_tokens = fields
        .iter()
        .map(|field| emission::field_tokens(field, dynamic_field_type_hints));
    let field_helper_tokens = match generated_method {
        GeneratedMethod::Host => {
            let helpers = fields.iter().map(emission::field_helper_tokens);
            quote! { #(#helpers)* }
        }
        GeneratedMethod::Reflect => quote! {},
    };
    let field_access_impl = match generated_method {
        GeneratedMethod::Host => emission::field_access_impl_tokens(&ident, &fields),
        GeneratedMethod::Reflect => quote! {},
    };
    let host_object_impl = match generated_method {
        GeneratedMethod::Host => {
            crate::host_object::base_script_host_object_impl_tokens(&syn::parse_quote!(#ident))
        }
        GeneratedMethod::Reflect => quote! {},
    };
    let type_helper_tokens = match generated_method {
        GeneratedMethod::Host => quote! {
            #[must_use]
            pub const fn vela_type_id() -> ::vela_def::TypeId {
                ::vela_def::TypeId::new(#type_id)
            }

            #[must_use]
            pub const fn vela_host_type_id() -> ::vela_common::HostTypeId {
                ::vela_common::HostTypeId::new(#host_id)
            }

            #[must_use]
            pub const fn vela_stable_type_path() -> &'static str {
                #stable_path
            }

            #[must_use]
            pub fn vela_type_binding() -> ::vela_engine::type_binding::TypeBinding<Self> {
                <Self as ::vela_engine::schema::ScriptHostSchema>::script_host_binding()
            }

            #[must_use]
            pub fn vela_type() -> ::vela_engine::registration::TypeRegistration<Self> {
                ::vela_engine::registration::TypeRegistration::of()
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
                        ::vela_def::TypeId::new(#type_id),
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

        #field_access_impl
        #host_object_impl
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
        let schema_hash = schema::opaque_enum_schema_hash(
            &input,
            &type_name,
            Some(&module_name),
            &type_attrs,
            &trait_names,
        );
        let ident = input.ident;
        let method = generated_method.ident();
        let trait_impl = generated_method.trait_impl_tokens(&ident, &method, &[]);
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
        let field_access_impl = emission::field_access_impl_tokens(&ident, &[]);
        let host_object_impl =
            crate::host_object::base_script_host_object_impl_tokens(&syn::parse_quote!(#ident));
        return Ok(quote! {
            impl #ident {
                #[must_use]
                pub const fn vela_type_id() -> ::vela_def::TypeId {
                    ::vela_def::TypeId::new(#type_id)
                }

                #[must_use]
                pub const fn vela_host_type_id() -> ::vela_common::HostTypeId {
                    ::vela_common::HostTypeId::new(#host_id)
                }

                #[must_use]
                pub const fn vela_stable_type_path() -> &'static str {
                    #stable_path
                }

                #[must_use]
                pub fn vela_type_binding() -> ::vela_engine::type_binding::TypeBinding<Self> {
                    <Self as ::vela_engine::schema::ScriptHostSchema>::script_host_binding()
                }

                #[must_use]
                pub fn vela_type() -> ::vela_engine::registration::TypeRegistration<Self> {
                    ::vela_engine::registration::TypeRegistration::of()
                }

                #[must_use]
                pub fn #method() -> ::vela_reflect::registry::TypeDesc {
                    let mut desc = ::vela_reflect::registry::TypeDesc::new(
                        ::vela_reflect::registry::TypeKey::new(
                            ::vela_def::TypeId::new(#type_id),
                            #type_name,
                        ),
                    )
                    .kind(::vela_reflect::registry::TypeKind::Host)
                    .schema_hash(::vela_reflect::registry::SchemaHash::new(#schema_hash))
                    .host_type(::vela_common::HostTypeId::new(#host_id))
                    #module_tokens
                    #docs_tokens
                    .attr("host_shape", "opaque_enum");
                    #(#type_attr_tokens)*
                    #(#trait_tokens)*
                    desc
                }
            }

            #trait_impl
            #field_access_impl
            #host_object_impl
        });
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
    let trait_impl = generated_method.trait_impl_tokens(&ident, &method, &[]);
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
                        ::vela_def::TypeId::new(#type_id),
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

pub(crate) fn type_identity(
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
                    "ScriptHost requires #[vela(path = \"module::Type\")] or #[vela(module = \"module\")]",
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
    let type_id = u128::from(vela_common::stable_id("host_type", "", &stable_path));
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
    fn fields_mode_projects_deref_wrappers_and_registers_dependencies() {
        let expanded = expand_result(
            quote! {
                #[vela(path = "game::Actor", fields)]
                struct Actor {
                    player: Player,
                    #[vela(deref)]
                    equipment: Tracked<Equipment>,
                }
            },
            GeneratedMethod::Host,
        )
        .expect("fields mode should expand");
        let output = expanded.to_string();

        assert!(output.contains("register_type_dependency :: < Player >"));
        assert!(output.contains("register_type_dependency :: < Equipment >"));
        assert!(output.contains("Deref :: deref"));
        assert!(output.contains("DerefMut :: deref_mut"));
        assert!(output.contains(
            "< Equipment as :: vela_host :: object :: ScriptHostFieldAccess > :: script_host_type_shape"
        ));
    }

    #[test]
    fn registered_host_field_projects_arbitrary_rust_type_without_trait_dependency() {
        let expanded = expand_result(
            quote! {
                #[vela(path = "game::Actor")]
                struct Actor {
                    #[vela(host = "game::Outbox")]
                    outbox: std::collections::VecDeque<Frame>,
                }
            },
            GeneratedMethod::Host,
        )
        .expect("registered Host field should expand");
        let output = expanded.to_string();

        assert!(output.contains("registered_shared_host"));
        assert!(output.contains("registered_exclusive_host"));
        assert!(output.contains("game::Outbox"));
        assert!(!output.contains("register_type_dependency :: < std :: collections :: VecDeque"));
        assert!(!output.contains("VecDeque < Frame > as :: vela_host :: object"));
    }

    #[test]
    fn registered_host_field_rejects_conflicting_projection_attributes() {
        let error = expand_result(
            quote! {
                #[vela(path = "game::Actor")]
                struct Actor {
                    #[vela(host = "game::Outbox", deref)]
                    outbox: Wrapper<Outbox>,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("registered Host and deref projection must be unambiguous");

        assert!(
            error
                .to_string()
                .contains("registered Host fields cannot also use deref projection")
        );
    }

    #[test]
    fn deref_projection_rejects_replacing_the_storage_wrapper() {
        let error = expand_result(
            quote! {
                #[vela(path = "game::Actor", fields)]
                struct Actor {
                    #[vela(deref, set)]
                    equipment: Tracked<Equipment>,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("deref wrapper replacement should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("deref-projected host fields cannot replace their storage wrapper")
        );
    }

    #[test]
    fn rejects_duplicate_field_aliases() {
        let error = expand_result(
            quote! {
                #[vela(path = "game::player::Player")]
                struct Player {
                    #[vela(get, alias = "score")]
                    level: u32,
                    #[vela(get, alias = "score")]
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
                #[vela(path = "game::player::Player")]
                struct Player {
                    #[vela(get, name = "level")]
                    level: u32,
                    #[vela(get, name = "level")]
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
                #[vela(path = "game::player::Player")]
                struct Player {
                    #[vela(get, name = "")]
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
                #[vela(path = "game::player::Player")]
                struct Player {
                    #[vela(get, permission = "")]
                    level: u32,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("empty field permission should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("vela permission cannot be empty")
        );
    }

    #[test]
    fn rejects_duplicate_type_attrs() {
        let error = expand_result(
            quote! {
                #[vela(path = "game::player::Player", attr = "domain=gameplay", attr = "domain=combat")]
                struct Player {
                    #[vela(get)]
                    level: u32,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("duplicate type attr keys should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("vela attr metadata key `domain` is duplicated")
        );
    }

    #[test]
    fn rejects_duplicate_field_attrs() {
        let error = expand_result(
            quote! {
                #[vela(path = "game::player::Player")]
                struct Player {
                    #[vela(get, attr = "unit=level", attr = "unit=rank")]
                    level: u32,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("duplicate field attr keys should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("vela attr metadata key `unit` is duplicated")
        );
    }

    #[test]
    fn rejects_empty_field_type_hints() {
        let error = expand_result(
            quote! {
                #[vela(path = "game::player::Player")]
                struct Player {
                    #[vela(get, hint = "")]
                    inventory: Vec<String>,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("empty field type hint should fail macro expansion");

        assert!(error.to_string().contains(
            "vela type hint must be a non-generic name or supported builtin type-hint contract"
        ));
    }

    #[test]
    fn derives_host_access_target_helpers_and_mutation_forwarder() {
        let expanded = expand_result(
            quote! {
                #[vela(path = "game::player::Player")]
                struct Player {
                    #[vela(get, set)]
                    level: i64,
                }
            },
            GeneratedMethod::Host,
        )
        .expect("host access helpers should expand")
        .to_string();

        assert!(expanded.contains("HostTargetPlan :: new"));
        assert!(expanded.contains("mutate_host_target_from"));
        assert!(expanded.contains("mutate_host_value"));
    }

    #[test]
    fn rejects_unsupported_generic_field_type_hints() {
        let error = expand_result(
            quote! {
                #[vela(path = "game::player::Player")]
                struct Player {
                    #[vela(get, hint = "Player<i64>")]
                    inventory: Vec<String>,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("unsupported generic field type hint should fail macro expansion");

        assert!(error.to_string().contains(
            "vela type hint must be a non-generic name or supported builtin type-hint contract"
        ));
    }

    #[test]
    fn accepts_value_keyed_generic_field_type_hints() {
        let expanded = expand_result(
            quote! {
                #[vela(path = "game::player::Player")]
                struct Player {
                    #[vela(get, hint = "Map<i64, String>")]
                    scores: std::collections::BTreeMap<String, String>,
                    #[vela(get, hint = "Set<Player>")]
                    seen: std::collections::BTreeSet<String>,
                }
            },
            GeneratedMethod::Host,
        )
        .expect("value-keyed field type hints should expand")
        .to_string();

        assert!(expanded.contains("Map<i64, String>"));
        assert!(expanded.contains("Set<Player>"));
    }

    #[test]
    fn rejects_non_keyable_generic_field_type_hints() {
        let error = expand_result(
            quote! {
                #[vela(path = "game::player::Player")]
                struct Player {
                    #[vela(get, hint = "Map<PathProxy, String>")]
                    inventory: Vec<String>,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("non-keyable map key hint should fail macro expansion");

        assert!(error.to_string().contains(
            "vela type hint must be a non-generic name or supported builtin type-hint contract"
        ));
    }

    #[test]
    fn rejects_malformed_field_type_hints() {
        let error = expand_result(
            quote! {
                #[vela(path = "game::player::Player")]
                struct Player {
                    #[vela(get, type = "game::::Inventory")]
                    inventory: Vec<String>,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("malformed field type hint should fail macro expansion");

        assert!(error.to_string().contains(
            "vela type hint must be a non-generic name or supported builtin type-hint contract"
        ));
    }

    #[test]
    fn rejects_missing_type_path() {
        let error = expand_result(
            quote! {
                struct Player {
                    #[vela(get)]
                    level: u32,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("missing type path should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("requires #[vela(path = \"module::Type\")]")
        );
    }

    #[test]
    fn rejects_empty_type_names() {
        let error = expand_result(
            quote! {
                #[vela(module = "game::player", name = "")]
                struct Player {
                    #[vela(get)]
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
                #[vela(module = "")]
                struct Player {
                    #[vela(get)]
                    level: u32,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("empty module name should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("vela module must be a non-empty `::` qualified name")
        );
    }

    #[test]
    fn rejects_malformed_module_names() {
        for module in ["::game", "game::", "game::::player", "game.player"] {
            let error = expand_result(
                quote! {
                    #[vela(module = #module)]
                    struct Player {
                        #[vela(get)]
                        level: u32,
                    }
                },
                GeneratedMethod::Host,
            )
            .expect_err("malformed module name should fail macro expansion");

            assert!(
                error
                    .to_string()
                    .contains("vela module must be a non-empty `::` qualified name")
            );
        }
    }

    #[test]
    fn rejects_malformed_static_attrs() {
        let error = expand_result(
            quote! {
                #[vela(path = "game::player::Player", attr = "gameplay")]
                struct Player {
                    #[vela(get)]
                    level: u32,
                }
            },
            GeneratedMethod::Host,
        )
        .expect_err("malformed attrs should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("vela attr metadata must use `key=value`")
        );
    }

    #[test]
    fn infers_fixed_array_field_type_hints() {
        let tokens = expand_result(
            quote! {
                #[vela(path = "game::spawn::SpawnTable")]
                struct SpawnTable {
                    #[vela(get)]
                    weights: [i64; 3],
                }
            },
            GeneratedMethod::Host,
        )
        .expect("fixed array host schema should expand")
        .to_string();

        assert!(tokens.contains(
            "< [i64 ; 3] as :: vela_host :: object :: ScriptHostFieldAccess > :: script_host_type_shape"
        ));
    }

    #[test]
    fn infers_value_keyed_map_and_set_field_type_hints() {
        let tokens = expand_result(
            quote! {
                #[vela(path = "game::player::Scores")]
                struct Scores {
                    #[vela(get)]
                    by_id: std::collections::BTreeMap<i64, String>,
                    #[vela(get)]
                    seen: std::collections::HashSet<i64>,
                }
            },
            GeneratedMethod::Host,
        )
        .expect("value-keyed map and set fields should expand")
        .to_string();

        assert!(tokens.contains(
            "< std :: collections :: BTreeMap < i64 , String > as :: vela_host :: object :: ScriptHostFieldAccess > :: script_host_type_shape"
        ));
        assert!(tokens.contains(
            "< std :: collections :: HashSet < i64 > as :: vela_host :: object :: ScriptHostFieldAccess > :: script_host_type_shape"
        ));
    }

    #[test]
    fn rejects_generic_host_schemas() {
        let error = expand_result(
            quote! {
                #[vela(path = "game::inventory::Inventory")]
                struct Inventory<T>
                where
                    T: Clone,
                {
                    #[vela(get)]
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
