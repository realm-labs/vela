use std::collections::BTreeSet;

use syn::{Data, DeriveInput, Fields, Result};

use crate::attrs::{error, inferred_type_hint, parse_script_attrs, spanned_error};
use crate::hash::StableHasher;

#[derive(Clone, Debug)]
pub(super) struct FieldMeta {
    pub(super) rust_name: String,
    pub(super) script_name: String,
    pub(super) stable_name: String,
    pub(super) id: u64,
    pub(super) readable: bool,
    pub(super) writable: bool,
    pub(super) type_hint: Option<String>,
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
        if attrs.skip || !attrs.has_script_attr {
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
        result.push(FieldMeta {
            script_name,
            stable_name,
            rust_name,
            id,
            readable: attrs.get,
            writable: attrs.set,
            type_hint: attrs.type_hint.or_else(|| inferred_type_hint(&field.ty)),
            docs: attrs.docs,
            attrs: attrs.attrs,
            permissions: attrs.permissions,
        });
    }

    Ok(result)
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
                result.push(FieldMeta {
                    script_name,
                    stable_name,
                    rust_name,
                    id,
                    readable: attrs.get,
                    writable: attrs.set,
                    type_hint: attrs.type_hint.or_else(|| inferred_type_hint(&field.ty)),
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
