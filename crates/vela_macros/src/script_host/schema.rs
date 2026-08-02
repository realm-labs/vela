use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{Data, DeriveInput, Fields, GenericArgument, PathArguments, Result, Type};

use crate::attrs::{error, inferred_type_hint, parse_script_attrs, spanned_error};
use crate::hash::StableHasher;

#[derive(Clone, Debug)]
pub(super) struct FieldMeta {
    pub(super) rust_name: String,
    pub(super) rust_type: TokenStream,
    pub(super) deref: bool,
    pub(super) script_name: String,
    pub(super) stable_name: String,
    pub(super) id: u64,
    pub(super) readable: bool,
    pub(super) writable: bool,
    pub(super) registered_host: Option<String>,
    pub(super) type_hint: Option<String>,
    pub(super) type_hint_explicit: bool,
    pub(super) docs: Option<String>,
    pub(super) attrs: Vec<(String, String)>,
    pub(super) permissions: Vec<String>,
}

#[derive(Clone, Debug)]
pub(super) struct VariantMeta {
    pub(super) script_name: String,
    pub(super) stable_name: String,
    pub(super) id: u64,
    pub(super) fields: Vec<FieldMeta>,
    pub(super) docs: Option<String>,
    pub(super) attrs: Vec<(String, String)>,
}

pub(super) fn collect_fields(
    input: &DeriveInput,
    type_stable_path: &str,
    expose_all: bool,
) -> Result<Vec<FieldMeta>> {
    let Data::Struct(data) = &input.data else {
        return Err(spanned_error(input, "ScriptHost only supports structs"));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(spanned_error(
            input,
            "ScriptHost requires named struct fields",
        ));
    };

    let mut seen_stable_names = BTreeSet::new();
    let mut seen_ids = BTreeSet::new();
    let mut seen_names = BTreeSet::new();
    let mut result = Vec::new();
    for field in &fields.named {
        let attrs = parse_script_attrs(&field.attrs)?;
        if attrs.skip || (!expose_all && !attrs.has_script_attr) {
            continue;
        }
        let ident = field
            .ident
            .as_ref()
            .ok_or_else(|| spanned_error(field, "ScriptHost requires named struct fields"))?;
        let rust_name = ident.to_string();
        let script_name = attrs.field_name(&rust_name);
        if script_name.is_empty() {
            return Err(error(ident.span(), "script field name cannot be empty"));
        }
        if !seen_names.insert(script_name.clone()) {
            return Err(error(ident.span(), "duplicate script field name"));
        }
        let stable_name = attrs.alias.clone().unwrap_or_else(|| script_name.clone());
        if !seen_stable_names.insert(stable_name.clone()) {
            return Err(error(ident.span(), "duplicate script field alias"));
        }
        let id = vela_common::stable_id("host_field", type_stable_path, &stable_name);
        if !seen_ids.insert(id) {
            return Err(error(ident.span(), "duplicate generated script field id"));
        }
        if attrs.deref && attrs.set {
            return Err(error(
                ident.span(),
                "deref-projected host fields cannot replace their storage wrapper; mutate the projected child instead",
            ));
        }
        if attrs.deref && attrs.host.is_some() {
            return Err(error(
                ident.span(),
                "registered Host fields cannot also use deref projection",
            ));
        }
        if attrs.host.is_some() && attrs.type_hint.is_some() {
            return Err(error(
                ident.span(),
                "registered Host fields derive their type hint from `host`",
            ));
        }
        let rust_type = if attrs.deref {
            deref_target_type(&field.ty)?.to_token_stream()
        } else {
            field.ty.to_token_stream()
        };
        let default_access = expose_all && !attrs.get && !attrs.set && attrs.host.is_none();
        let type_hint_explicit = attrs.type_hint.is_some() || attrs.host.is_some();
        let registered_host = attrs.host.clone();
        result.push(FieldMeta {
            script_name,
            stable_name,
            rust_name,
            rust_type,
            deref: attrs.deref,
            id,
            readable: attrs.get || attrs.host.is_some() || default_access,
            writable: attrs.set || default_access && !attrs.deref,
            registered_host,
            type_hint: attrs.host.or(attrs.type_hint).or_else(|| {
                if attrs.deref {
                    deref_target_type(&field.ty)
                        .ok()
                        .and_then(inferred_type_hint)
                } else {
                    inferred_type_hint(&field.ty)
                }
            }),
            type_hint_explicit,
            docs: attrs.docs,
            attrs: attrs.attrs,
            permissions: attrs.permissions,
        });
    }

    Ok(result)
}

pub(super) fn registration_types(fields: &[FieldMeta]) -> Vec<TokenStream> {
    let mut seen = BTreeSet::new();
    let mut dependencies = Vec::new();
    for field in fields {
        if field.registered_host.is_some() {
            continue;
        }
        collect_registration_types(&field.rust_type, &mut seen, &mut dependencies);
    }
    dependencies
}

fn collect_registration_types(
    ty: &TokenStream,
    seen: &mut BTreeSet<String>,
    dependencies: &mut Vec<TokenStream>,
) {
    let Ok(ty) = syn::parse2::<Type>(ty.clone()) else {
        return;
    };
    collect_type_dependencies(&ty, seen, dependencies);
}

fn collect_type_dependencies(
    ty: &Type,
    seen: &mut BTreeSet<String>,
    dependencies: &mut Vec<TokenStream>,
) {
    match ty {
        Type::Array(array) => collect_type_dependencies(&array.elem, seen, dependencies),
        Type::Tuple(tuple) => {
            for element in &tuple.elems {
                collect_type_dependencies(element, seen, dependencies);
            }
        }
        Type::Path(path) => {
            let Some(segment) = path.path.segments.last() else {
                return;
            };
            let container = matches!(
                segment.ident.to_string().as_str(),
                "Vec" | "Option" | "Result" | "BTreeMap" | "HashMap" | "BTreeSet" | "HashSet"
            );
            if container {
                if let PathArguments::AngleBracketed(arguments) = &segment.arguments {
                    for argument in &arguments.args {
                        if let GenericArgument::Type(ty) = argument {
                            collect_type_dependencies(ty, seen, dependencies);
                        }
                    }
                }
                return;
            }
            let rendered = ty.to_token_stream().to_string();
            if seen.insert(rendered) {
                dependencies.push(ty.to_token_stream());
            }
        }
        _ => {}
    }
}

fn deref_target_type(ty: &Type) -> Result<&Type> {
    let Type::Path(path) = ty else {
        return Err(spanned_error(
            ty,
            "#[vela(deref)] requires a wrapper type with one type argument",
        ));
    };
    let Some(segment) = path.path.segments.last() else {
        return Err(spanned_error(
            ty,
            "#[vela(deref)] requires a wrapper type with one type argument",
        ));
    };
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Err(spanned_error(
            ty,
            "#[vela(deref)] requires a wrapper type with one type argument",
        ));
    };
    let mut types = arguments.args.iter().filter_map(|argument| match argument {
        GenericArgument::Type(ty) => Some(ty),
        _ => None,
    });
    let Some(target) = types.next() else {
        return Err(spanned_error(
            ty,
            "#[vela(deref)] requires a wrapper type with one type argument",
        ));
    };
    if types.next().is_some() {
        return Err(spanned_error(
            ty,
            "#[vela(deref)] requires exactly one type argument",
        ));
    }
    Ok(target)
}

pub(super) fn collect_variants(
    input: &DeriveInput,
    type_name: &str,
    type_stable_path: &str,
) -> Result<Vec<VariantMeta>> {
    let Data::Enum(data) = &input.data else {
        return Err(spanned_error(
            input,
            "ScriptReflect enum metadata requires an enum",
        ));
    };

    let mut seen_stable_names = BTreeSet::new();
    let mut seen_ids = BTreeSet::new();
    let mut seen_names = BTreeSet::new();
    let mut result = Vec::new();
    for variant in &data.variants {
        let attrs = parse_script_attrs(&variant.attrs)?;
        if attrs.skip {
            continue;
        }
        let rust_name = variant.ident.to_string();
        let script_name = attrs.field_name(&rust_name);
        if script_name.is_empty() {
            return Err(error(
                variant.ident.span(),
                "script variant name cannot be empty",
            ));
        }
        if !seen_names.insert(script_name.clone()) {
            return Err(error(variant.ident.span(), "duplicate script variant name"));
        }
        let stable_name = attrs.alias.clone().unwrap_or_else(|| script_name.clone());
        if !seen_stable_names.insert(stable_name.clone()) {
            return Err(error(
                variant.ident.span(),
                "duplicate script variant alias",
            ));
        }
        let id = vela_common::stable_id("variant", type_stable_path, &stable_name);
        if !seen_ids.insert(id) {
            return Err(error(
                variant.ident.span(),
                "duplicate generated script variant id",
            ));
        }
        let fields = collect_variant_fields(&variant.fields, type_name, &script_name)?;
        result.push(VariantMeta {
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

fn collect_variant_fields(
    fields: &Fields,
    type_name: &str,
    variant_name: &str,
) -> Result<Vec<FieldMeta>> {
    match fields {
        Fields::Unit => Ok(Vec::new()),
        Fields::Named(fields) => {
            let mut seen_stable_names = BTreeSet::new();
            let mut seen_ids = BTreeSet::new();
            let mut seen_names = BTreeSet::new();
            let owner = format!("{type_name}::{variant_name}");
            let mut result = Vec::new();
            for field in &fields.named {
                let attrs = parse_script_attrs(&field.attrs)?;
                if attrs.skip || !attrs.has_script_attr {
                    continue;
                }
                let ident = field.ident.as_ref().ok_or_else(|| {
                    spanned_error(field, "ScriptReflect enum variant fields must be named")
                })?;
                let rust_name = ident.to_string();
                let script_name = attrs.field_name(&rust_name);
                if script_name.is_empty() {
                    return Err(error(
                        ident.span(),
                        "script variant field name cannot be empty",
                    ));
                }
                if !seen_names.insert(script_name.clone()) {
                    return Err(error(ident.span(), "duplicate script variant field name"));
                }
                let stable_name = attrs.alias.clone().unwrap_or_else(|| script_name.clone());
                if !seen_stable_names.insert(stable_name.clone()) {
                    return Err(error(ident.span(), "duplicate script variant field alias"));
                }
                let id = vela_common::stable_id("field", &owner, &stable_name);
                if !seen_ids.insert(id) {
                    return Err(error(
                        ident.span(),
                        "duplicate generated script variant field id",
                    ));
                }
                let type_hint_explicit = attrs.type_hint.is_some();
                result.push(FieldMeta {
                    script_name,
                    stable_name,
                    rust_name,
                    rust_type: field.ty.to_token_stream(),
                    deref: false,
                    id,
                    readable: attrs.get,
                    writable: attrs.set,
                    registered_host: None,
                    type_hint: attrs.type_hint.or_else(|| inferred_type_hint(&field.ty)),
                    type_hint_explicit,
                    docs: attrs.docs,
                    attrs: attrs.attrs,
                    permissions: attrs.permissions,
                });
            }
            Ok(result)
        }
        Fields::Unnamed(fields) => Err(spanned_error(
            fields,
            "ScriptReflect enum metadata requires named variant fields",
        )),
    }
}

pub(super) fn schema_hash(
    type_name: &str,
    module_name: Option<&str>,
    attrs: &[(String, String)],
    traits: &[String],
    fields: &[FieldMeta],
) -> u64 {
    let mut hasher = StableHasher::new();
    hasher.write_str(type_name);
    if let Some(module_name) = module_name {
        hasher.write_str(module_name);
    }
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
        hasher.write_bool(field.readable);
        hasher.write_bool(field.writable);
        hasher.write_str(field.type_hint.as_deref().unwrap_or(""));
        for (name, value) in &field.attrs {
            hasher.write_str(name);
            hasher.write_str(value);
        }
        for permission in &field.permissions {
            hasher.write_str(permission);
        }
    }
    hasher.finish()
}

pub(super) fn enum_schema_hash(
    type_name: &str,
    module_name: Option<&str>,
    attrs: &[(String, String)],
    traits: &[String],
    variants: &[VariantMeta],
) -> u64 {
    let mut hasher = StableHasher::new();
    hasher.write_str(type_name);
    if let Some(module_name) = module_name {
        hasher.write_str(module_name);
    }
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
            hasher.write_bool(field.readable);
            hasher.write_bool(field.writable);
            hasher.write_str(field.type_hint.as_deref().unwrap_or(""));
            for (name, value) in &field.attrs {
                hasher.write_str(name);
                hasher.write_str(value);
            }
            for permission in &field.permissions {
                hasher.write_str(permission);
            }
        }
    }
    hasher.finish()
}

pub(super) fn opaque_enum_schema_hash(
    input: &DeriveInput,
    type_name: &str,
    module_name: Option<&str>,
    attrs: &[(String, String)],
    traits: &[String],
) -> u64 {
    let mut hasher = StableHasher::new();
    hasher.write_str(type_name);
    if let Some(module_name) = module_name {
        hasher.write_str(module_name);
    }
    for (name, value) in attrs {
        hasher.write_str(name);
        hasher.write_str(value);
    }
    for trait_name in traits {
        hasher.write_str(trait_name);
    }
    if let Data::Enum(data) = &input.data {
        for variant in &data.variants {
            hasher.write_str(&variant.to_token_stream().to_string());
        }
    }
    hasher.finish()
}
