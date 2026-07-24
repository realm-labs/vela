use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Attribute, Fields, ItemStruct, Path, Result, Type, TypeParamBound, Visibility, parse::Parser,
    parse2,
};

use crate::service::{
    composition_function_ident, registration_function_ident, schema_function_ident,
};
use crate::signature::reject_generic_signature;

pub(crate) fn expand(attr: TokenStream, input: TokenStream) -> TokenStream {
    match expand_result(attr, input) {
        Ok(tokens) => tokens,
        Err(error) => error.to_compile_error(),
    }
}

fn expand_result(attr: TokenStream, input: TokenStream) -> Result<TokenStream> {
    let item = parse2::<ItemStruct>(input)?;
    validate_struct(&item)?;
    let context = parse_context(attr)?;
    let Fields::Named(fields) = &item.fields else {
        unreachable!("validated service set has named fields");
    };
    let services = fields
        .named
        .iter()
        .map(parse_service_field)
        .collect::<Result<Vec<_>>>()?;
    if services.is_empty() {
        return Err(syn::Error::new_spanned(
            &item,
            "#[vela::service_set] requires at least one service field",
        ));
    }

    let set_ident = &item.ident;
    let generation_ident = format_ident!("{set_ident}Generation");
    let root_ident = format_ident!("{set_ident}Root");
    let candidate_ident = format_ident!("{set_ident}Candidate");
    let rollback_ident = format_ident!("{set_ident}Rollback");
    let register_calls = services.iter().map(|service| {
        let function = service.registration_path();
        quote! {
            let builder = #function(builder);
        }
    });
    let schema_calls = services.iter().map(|service| {
        let function = service.schema_path();
        quote! { #function(registry)? }
    });
    let generation_fields = services.iter().map(|service| {
        let field = &service.field;
        let trait_path = &service.trait_path;
        quote! {
            #field: ::std::sync::Arc<dyn #trait_path>
        }
    });
    let composed_services = services.iter().map(|service| {
        let field = &service.field;
        let trait_path = &service.trait_path;
        let default = &service.default;
        let composition = service.composition_path();
        quote! {
            let #field: ::std::sync::Arc<dyn #trait_path> = #composition(
                ::std::sync::Arc::new(#default),
                runtime,
                options.clone(),
                &selections,
            );
        }
    });
    let generation_arguments = services.iter().map(|service| {
        let field = &service.field;
        let trait_path = &service.trait_path;
        quote! {
            #field: ::std::sync::Arc<dyn #trait_path>
        }
    });
    let generation_initializers = services
        .iter()
        .map(|service| &service.field)
        .collect::<Vec<_>>();
    let default_initializers = services.iter().map(|service| {
        let field = &service.field;
        let default = &service.default;
        quote! {
            #field: ::std::sync::Arc::new(#default)
        }
    });
    let generation_accessors = services.iter().map(|service| {
        let field = &service.field;
        let trait_path = &service.trait_path;
        quote! {
            #[must_use]
            pub fn #field(&self) -> &(dyn #trait_path + 'static) {
                self.#field.as_ref()
            }
        }
    });
    let root_accessors = services.iter().map(|service| {
        let field = &service.field;
        let trait_path = &service.trait_path;
        quote! {
            #[must_use]
            pub fn #field(&self) -> &(dyn #trait_path + 'static) {
                self.root.services().#field()
            }
        }
    });
    let doc_attrs = item
        .attrs
        .iter()
        .filter(|attribute| attribute.path().is_ident("doc"));
    let vis = &item.vis;

    Ok(quote! {
        #(#doc_attrs)*
        #vis struct #generation_ident {
            #(#generation_fields,)*
            selections: ::std::option::Option<
                ::vela_engine::service::ServiceSelectionTable<
                    ::vela_engine::service::LinkedVelaServiceMethod
                >
            >,
        }

        impl #generation_ident {
            #[must_use]
            pub fn new(#(#generation_arguments),*) -> Self {
                Self {
                    #(#generation_initializers,)*
                    selections: None,
                }
            }

            #[must_use]
            pub fn defaults() -> Self {
                Self {
                    #(#default_initializers,)*
                    selections: None,
                }
            }

            fn __vela_composed(
                runtime: ::vela_engine::service::ServiceRuntimeBinding,
                options: ::vela_engine::runtime::CallOptions,
                selections: ::vela_engine::service::ServiceSelectionTable<
                    ::vela_engine::service::LinkedVelaServiceMethod
                >,
            ) -> Self {
                #(#composed_services)*
                Self {
                    #(#generation_initializers,)*
                    selections: Some(selections),
                }
            }

            #[must_use]
            pub fn selections(
                &self,
            ) -> ::std::option::Option<
                &::vela_engine::service::ServiceSelectionTable<
                    ::vela_engine::service::LinkedVelaServiceMethod
                >
            > {
                self.selections.as_ref()
            }

            #(#generation_accessors)*
        }

        #vis struct #set_ident {
            controller: ::vela_engine::service::ServiceController<#generation_ident>,
            schema: ::vela_engine::service::ServiceSetSchema,
            _context: ::std::marker::PhantomData<fn(&mut #context)>,
        }

        impl #set_ident {
            #[must_use]
            pub fn register_types(
                builder: ::vela_engine::builder::EngineBuilder,
            ) -> ::vela_engine::builder::EngineBuilder {
                let builder = builder;
                #(#register_calls)*
                builder
            }

            pub fn new(
                registry: &::vela_engine::type_binding::TypeBindingRegistry,
            ) -> ::std::result::Result<
                Self,
                ::vela_engine::service::ServiceSchemaError,
            > {
                let path = ::std::concat!(::std::module_path!(), "::", ::std::stringify!(#set_ident));
                let id = ::vela_common::ServiceSetId::new(
                    u128::from(::vela_common::stable_id("vela_service_set", "", path)),
                );
                let schema = ::vela_engine::service::ServiceSetSchema::new(
                    id,
                    path,
                    vec![#(#schema_calls),*],
                    registry,
                )?;
                Ok(Self {
                    controller: ::vela_engine::service::ServiceController::new(
                        id,
                        #generation_ident::defaults(),
                    ),
                    schema,
                    _context: ::std::marker::PhantomData,
                })
            }

            #[must_use]
            pub fn schema(&self) -> &::vela_engine::service::ServiceSetSchema {
                &self.schema
            }

            #[must_use]
            pub fn pin(&self) -> #root_ident {
                #root_ident {
                    root: self.controller.pin(),
                    _context: ::std::marker::PhantomData,
                }
            }

            pub fn stage_rust(
                &self,
                base: &#root_ident,
                generation: #generation_ident,
            ) -> ::std::result::Result<
                #candidate_ident,
                ::vela_engine::service::ServicePublicationError,
            > {
                self.controller
                    .stage(&base.root, generation)
                    .map(|candidate| #candidate_ident { candidate })
            }

            pub fn stage_snapshot(
                &self,
                base: &#root_ident,
                update: ::vela_engine::service::LinkedServiceSourceManifest,
                runtime: ::vela_engine::service::ServiceRuntimeBinding,
                options: ::vela_engine::runtime::CallOptions,
            ) -> ::std::result::Result<
                #candidate_ident,
                ::vela_engine::service::ServiceStagingError,
            > {
                if !runtime.matches::<#context>() {
                    return Err(
                        ::vela_engine::service::ServiceStagingError::ContextTypeMismatch {
                            expected: ::core::any::type_name::<#context>(),
                            actual: runtime.context_name(),
                        }
                    );
                }
                let selections = update.into_snapshot(&self.schema)?;
                let generation = #generation_ident::__vela_composed(
                    runtime,
                    options,
                    selections,
                );
                self.controller
                    .stage(&base.root, generation)
                    .map(|candidate| #candidate_ident { candidate })
                    .map_err(::vela_engine::service::ServiceStagingError::from)
            }

            pub fn stage_delta(
                &self,
                base: &#root_ident,
                update: ::vela_engine::service::LinkedServiceSourceManifest,
                runtime: ::vela_engine::service::ServiceRuntimeBinding,
                options: ::vela_engine::runtime::CallOptions,
            ) -> ::std::result::Result<
                #candidate_ident,
                ::vela_engine::service::ServiceStagingError,
            > {
                if !runtime.matches::<#context>() {
                    return Err(
                        ::vela_engine::service::ServiceStagingError::ContextTypeMismatch {
                            expected: ::core::any::type_name::<#context>(),
                            actual: runtime.context_name(),
                        }
                    );
                }
                let base_selections = match base.selections() {
                    Some(selections) => selections.clone(),
                    None => ::vela_engine::service::ServiceSelectionTable::snapshot(
                        &self.schema,
                        ::core::iter::empty::<
                            ::vela_engine::service::ServiceMethodUpdate<
                                ::vela_engine::service::LinkedVelaServiceMethod
                            >
                        >(),
                    )?,
                };
                let selections = update.into_delta(
                    &self.schema,
                    base.generation_id(),
                    base.generation_id(),
                    &base_selections,
                )?;
                let generation = #generation_ident::__vela_composed(
                    runtime,
                    options,
                    selections,
                );
                self.controller
                    .stage(&base.root, generation)
                    .map(|candidate| #candidate_ident { candidate })
                    .map_err(::vela_engine::service::ServiceStagingError::from)
            }

            pub fn activate_if_current(
                &self,
                candidate: #candidate_ident,
            ) -> ::std::result::Result<
                #rollback_ident,
                ::vela_engine::service::ServicePublicationError,
            > {
                self.controller
                    .activate_if_current(candidate.candidate)
                    .map(|token| #rollback_ident { token })
            }

            pub fn rollback_if_current(
                &self,
                rollback: #rollback_ident,
            ) -> ::std::result::Result<
                #root_ident,
                ::vela_engine::service::ServicePublicationError,
            > {
                self.controller
                    .rollback_if_current(rollback.token)
                    .map(|root| #root_ident {
                        root,
                        _context: ::std::marker::PhantomData,
                    })
            }
        }

        #vis struct #root_ident {
            root: ::vela_engine::service::ServiceRoot<#generation_ident>,
            _context: ::std::marker::PhantomData<fn(&mut #context)>,
        }

        impl ::std::clone::Clone for #root_ident {
            fn clone(&self) -> Self {
                Self {
                    root: self.root.clone(),
                    _context: ::std::marker::PhantomData,
                }
            }
        }

        impl #root_ident {
            #[must_use]
            pub fn generation_id(&self) -> ::vela_common::ServiceGenerationId {
                self.root.generation_id()
            }

            #[must_use]
            pub fn service_set_id(&self) -> ::vela_common::ServiceSetId {
                self.root.service_set_id()
            }

            #[must_use]
            pub fn selections(
                &self,
            ) -> ::std::option::Option<
                &::vela_engine::service::ServiceSelectionTable<
                    ::vela_engine::service::LinkedVelaServiceMethod
                >
            > {
                self.root.services().selections()
            }

            #(#root_accessors)*
        }

        #vis struct #candidate_ident {
            candidate: ::vela_engine::service::ServiceGenerationCandidate<#generation_ident>,
        }

        impl #candidate_ident {
            #[must_use]
            pub fn generation_id(&self) -> ::vela_common::ServiceGenerationId {
                self.candidate.generation_id()
            }

            #[must_use]
            pub fn base_generation_id(&self) -> ::vela_common::ServiceGenerationId {
                self.candidate.base_generation_id()
            }
        }

        #vis struct #rollback_ident {
            token: ::vela_engine::service::ServiceRollbackToken<#generation_ident>,
        }

        impl #rollback_ident {
            #[must_use]
            pub fn replaced_generation_id(&self) -> ::vela_common::ServiceGenerationId {
                self.token.replaced_generation_id()
            }

            #[must_use]
            pub fn installed_generation_id(&self) -> ::vela_common::ServiceGenerationId {
                self.token.installed_generation_id()
            }
        }
    })
}

fn validate_struct(item: &ItemStruct) -> Result<()> {
    if !matches!(item.vis, Visibility::Public(_)) {
        return Err(syn::Error::new_spanned(
            &item.vis,
            "#[vela::service_set] requires a public Rust struct",
        ));
    }
    reject_generic_signature(&item.generics, "#[vela::service_set]")?;
    if !matches!(item.fields, Fields::Named(_)) {
        return Err(syn::Error::new_spanned(
            &item.fields,
            "#[vela::service_set] requires named service fields",
        ));
    }
    Ok(())
}

fn parse_context(attr: TokenStream) -> Result<Type> {
    let mut context = None;
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("context") {
            if context.is_some() {
                return Err(meta.error("service-set context is duplicated"));
            }
            context = Some(meta.value()?.parse::<Type>()?);
            return Ok(());
        }
        Err(meta.error("unsupported service_set attribute"))
    });
    parser.parse2(attr)?;
    context.ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[vela::service_set] requires context = HostContext",
        )
    })
}

struct ServiceField {
    field: syn::Ident,
    trait_path: Path,
    default: Path,
}

impl ServiceField {
    fn registration_path(&self) -> Path {
        replace_trait_ident(
            &self.trait_path,
            registration_function_ident(
                &self
                    .trait_path
                    .segments
                    .last()
                    .expect("service trait path is non-empty")
                    .ident,
            ),
        )
    }

    fn schema_path(&self) -> Path {
        replace_trait_ident(
            &self.trait_path,
            schema_function_ident(
                &self
                    .trait_path
                    .segments
                    .last()
                    .expect("service trait path is non-empty")
                    .ident,
            ),
        )
    }

    fn composition_path(&self) -> Path {
        replace_trait_ident(
            &self.trait_path,
            composition_function_ident(
                &self
                    .trait_path
                    .segments
                    .last()
                    .expect("service trait path is non-empty")
                    .ident,
            ),
        )
    }
}

fn parse_service_field(field: &syn::Field) -> Result<ServiceField> {
    if !matches!(field.vis, Visibility::Public(_)) {
        return Err(syn::Error::new_spanned(
            &field.vis,
            "service-set fields must be public",
        ));
    }
    let field_ident = field.ident.clone().expect("named field");
    let Type::TraitObject(object) = &field.ty else {
        return Err(syn::Error::new_spanned(
            &field.ty,
            "service-set fields must use `dyn ServiceTrait`",
        ));
    };
    let [TypeParamBound::Trait(bound)] = object.bounds.iter().collect::<Vec<_>>().as_slice() else {
        return Err(syn::Error::new_spanned(
            object,
            "service-set fields must name exactly one `dyn ServiceTrait`",
        ));
    };
    if !matches!(bound.modifier, syn::TraitBoundModifier::None)
        || bound.lifetimes.is_some()
        || !bound
            .path
            .segments
            .iter()
            .all(|segment| matches!(segment.arguments, syn::PathArguments::None))
    {
        return Err(syn::Error::new_spanned(
            bound,
            "service-set trait paths cannot use modifiers, binders, or generic arguments",
        ));
    }
    let default = parse_default(&field.attrs)?;
    Ok(ServiceField {
        field: field_ident,
        trait_path: bound.path.clone(),
        default,
    })
}

fn parse_default(attrs: &[Attribute]) -> Result<Path> {
    let mut default = None;
    for attr in attrs {
        let segments = &attr.path().segments;
        let is_default = segments
            .last()
            .is_some_and(|segment| segment.ident == "default")
            && (segments.len() == 1
                || segments
                    .first()
                    .is_some_and(|segment| segment.ident == "vela"));
        if !is_default {
            continue;
        }
        if default.is_some() {
            return Err(syn::Error::new_spanned(
                attr,
                "service default is duplicated",
            ));
        }
        default = Some(attr.parse_args::<Path>()?);
    }
    default.ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "service-set field requires #[vela::default(RustService)]",
        )
    })
}

fn replace_trait_ident(path: &Path, ident: syn::Ident) -> Path {
    let mut path = path.clone();
    path.segments
        .last_mut()
        .expect("service trait path is non-empty")
        .ident = ident;
    path
}

#[cfg(test)]
mod tests {
    use quote::quote;

    use super::expand_result;

    #[test]
    fn service_set_generates_one_whole_generation_controller() {
        let output = expand_result(
            quote! { context = RequestContext },
            quote! {
                pub struct GameServices {
                    #[vela::default(RustRewardService)]
                    pub reward: dyn RewardService,
                    #[vela::default(RustInventoryService)]
                    pub inventory: dyn InventoryService,
                }
            },
        )
        .expect("service set should expand")
        .to_string();

        assert!(output.contains("GameServicesGeneration"));
        assert!(output.contains("ServiceController < GameServicesGeneration >"));
        assert!(output.contains("stage_snapshot"));
        assert!(output.contains("stage_delta"));
        assert!(output.contains("__vela_compose_service_RewardService"));
        assert_eq!(output.matches("ServiceController <").count(), 1);
        assert!(!output.contains("HostRef"));
        assert!(!output.contains("runtime : :: vela_engine :: runtime :: Runtime"));
    }

    #[test]
    fn service_set_requires_explicit_default() {
        let error = expand_result(
            quote! { context = RequestContext },
            quote! {
                pub struct GameServices {
                    pub reward: dyn RewardService,
                }
            },
        )
        .expect_err("missing default must fail");

        assert!(error.to_string().contains("requires #[vela::default"));
    }
}
