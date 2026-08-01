use proc_macro2::TokenStream;
use syn::{
    Fields, GenericArgument, ItemStruct, Path, PathArguments, Result, Type, TypeParamBound,
    Visibility, parse::Parser, parse_quote,
};

use crate::service::{
    composition_function_ident, dispatch_module_ident, registration_function_ident,
    rust_async_dispatch_function_ident, rust_dispatch_function_ident, schema_function_ident,
    service_id_function_ident,
};
use crate::signature::reject_generic_signature;

pub(super) fn validate_struct(item: &ItemStruct) -> Result<()> {
    if !matches!(item.vis, Visibility::Public(_)) {
        return Err(syn::Error::new_spanned(
            &item.vis,
            "#[vela_macros::service_domain] requires a public Rust struct",
        ));
    }
    reject_generic_signature(&item.generics, "#[vela_macros::service_domain]")?;
    if !matches!(item.fields, Fields::Named(_)) {
        return Err(syn::Error::new_spanned(
            &item.fields,
            "#[vela_macros::service_domain] requires named service fields",
        ));
    }
    Ok(())
}

pub(super) fn validate_services_not_empty(
    item: &ItemStruct,
    services: &[ServiceField],
) -> Result<()> {
    if services.is_empty() {
        return Err(syn::Error::new_spanned(
            item,
            "#[vela_macros::service_domain] requires at least one service field",
        ));
    }
    Ok(())
}

pub(super) fn parse_context(attr: TokenStream) -> Result<Type> {
    let mut context = None;
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("context") {
            if context.is_some() {
                return Err(meta.error("service-domain context is duplicated"));
            }
            context = Some(meta.value()?.parse::<Type>()?);
            return Ok(());
        }
        Err(meta.error("unsupported service_domain attribute"))
    });
    parser.parse2(attr)?;
    Ok(context.unwrap_or_else(|| parse_quote!(())))
}

pub(super) struct ServiceField {
    pub(super) field: syn::Ident,
    pub(super) marker: Path,
    pub(super) trait_path: Path,
}

impl ServiceField {
    pub(super) fn dispatch_trait_path(&self) -> Path {
        let mut path = replace_trait_ident(
            &self.trait_path,
            dispatch_module_ident(
                &self
                    .trait_path
                    .segments
                    .last()
                    .expect("service trait path is non-empty")
                    .ident,
            ),
        );
        path.segments.push(parse_quote!(Dispatch));
        path
    }

    pub(super) fn registration_path(&self) -> Path {
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

    pub(super) fn schema_path(&self) -> Path {
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

    pub(super) fn composition_path(&self) -> Path {
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

    pub(super) fn service_id_path(&self) -> Path {
        replace_trait_ident(
            &self.trait_path,
            service_id_function_ident(
                &self
                    .trait_path
                    .segments
                    .last()
                    .expect("service trait path is non-empty")
                    .ident,
            ),
        )
    }

    pub(super) fn rust_dispatch_path(&self) -> Path {
        replace_trait_ident(
            &self.trait_path,
            rust_dispatch_function_ident(
                &self
                    .trait_path
                    .segments
                    .last()
                    .expect("service trait path is non-empty")
                    .ident,
            ),
        )
    }

    pub(super) fn rust_async_dispatch_path(&self) -> Path {
        replace_trait_ident(
            &self.trait_path,
            rust_async_dispatch_function_ident(
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

pub(super) fn parse_service_field(field: &syn::Field) -> Result<ServiceField> {
    if !matches!(field.vis, Visibility::Public(_)) {
        return Err(syn::Error::new_spanned(
            &field.vis,
            "service-domain fields must be public",
        ));
    }
    let field_ident = field.ident.clone().expect("named field");
    let Type::Path(marker) = &field.ty else {
        return Err(syn::Error::new_spanned(
            &field.ty,
            "service-domain fields must use `Service<dyn ServiceTrait>`",
        ));
    };
    let Some(segment) = marker.path.segments.last() else {
        unreachable!("type paths contain at least one segment")
    };
    if segment.ident != "Service" {
        return Err(syn::Error::new_spanned(
            &field.ty,
            "service-domain fields must use `Service<dyn ServiceTrait>`",
        ));
    }
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Err(syn::Error::new_spanned(
            &field.ty,
            "service-domain `Service` marker requires one `dyn ServiceTrait` argument",
        ));
    };
    let [GenericArgument::Type(Type::TraitObject(object))] =
        arguments.args.iter().collect::<Vec<_>>().as_slice()
    else {
        return Err(syn::Error::new_spanned(
            &field.ty,
            "service-domain `Service` marker requires one `dyn ServiceTrait` argument",
        ));
    };
    let [TypeParamBound::Trait(bound)] = object.bounds.iter().collect::<Vec<_>>().as_slice() else {
        return Err(syn::Error::new_spanned(
            object,
            "service-domain fields must name exactly one `dyn ServiceTrait`",
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
            "service-domain trait paths cannot use modifiers, binders, or generic arguments",
        ));
    }
    let mut marker_path = marker.path.clone();
    marker_path
        .segments
        .last_mut()
        .expect("service marker path is non-empty")
        .arguments = PathArguments::None;
    Ok(ServiceField {
        field: field_ident,
        marker: marker_path,
        trait_path: bound.path.clone(),
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
