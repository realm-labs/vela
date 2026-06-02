use std::collections::BTreeSet;

use syn::{Data, DeriveInput, Fields, Result};

use crate::attrs::{error, inferred_type_hint, parse_script_attrs, spanned_error};
use crate::hash::StableHasher;

#[derive(Clone, Debug)]
pub(super) struct FieldMeta {
    pub(super) rust_name: String,
    pub(super) script_name: String,
    pub(super) id: u32,
    pub(super) readable: bool,
    pub(super) writable: bool,
    pub(super) type_hint: Option<String>,
    pub(super) docs: Option<String>,
    pub(super) attrs: Vec<(String, String)>,
    pub(super) permissions: Vec<String>,
}

pub(super) fn collect_fields(input: &DeriveInput) -> Result<Vec<FieldMeta>> {
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
        let script_name = attrs.field_name(&rust_name);
        if script_name.is_empty() {
            return Err(error(ident.span(), "script field name cannot be empty"));
        }
        if !seen_names.insert(script_name.clone()) {
            return Err(error(ident.span(), "duplicate script field name"));
        }
        result.push(FieldMeta {
            script_name,
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
        hasher.write_u32(field.id);
        hasher.write_str(&field.script_name);
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
