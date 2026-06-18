use vela_analysis::registry::{RegistryEffectFact, RegistryFacts};
use vela_analysis::type_fact::TypeFact;

use crate::{
    DiagnosticRange, DisplayParts,
    symbol_ref::{schema_member_symbol, schema_symbol, schema_variant_symbol},
};

use super::{Hover, HoverKind};

pub(super) fn member_hover(
    schema: &RegistryFacts,
    receiver_fact: &TypeFact,
    member: &str,
    range: DiagnosticRange,
) -> Option<Hover> {
    let owner = owner_name(receiver_fact)?;
    if let Some(fact) = schema.field_fact(&owner, member) {
        let detail_parts = schema.field_access_fact(&owner, member).map_or_else(
            || DisplayParts::type_name(fact.display_name()),
            |access| field_detail_parts(fact, access),
        );
        return Some(Hover::new(
            range,
            format!("{owner}.{member}"),
            HoverKind::Field,
            detail_parts,
            schema.field_docs(&owner, member).map(str::to_owned),
            Some(schema_member_symbol(&owner, member)),
        ));
    }
    method_fact(schema, &owner, member).map(|fact| {
        Hover::new(
            range,
            format!("{owner}.{member}"),
            HoverKind::Method,
            method_detail_parts(schema, &owner, member, fact),
            method_docs(schema, &owner, member).map(str::to_owned),
            Some(schema_member_symbol(&owner, member)),
        )
    })
}

pub(super) fn symbol_hover(
    schema: &RegistryFacts,
    name: &str,
    range: DiagnosticRange,
) -> Option<Hover> {
    if let Some(fact) = schema.type_fact(name) {
        return Some(Hover::new(
            range,
            name.to_owned(),
            HoverKind::Type,
            DisplayParts::type_name(fact.display_name()),
            schema.type_docs(name).map(str::to_owned),
            Some(schema_symbol(name)),
        ));
    }
    if let Some(fact) = schema.trait_fact(name) {
        return Some(Hover::new(
            range,
            name.to_owned(),
            HoverKind::Trait,
            DisplayParts::type_name(fact.display_name()),
            schema.trait_docs(name).map(str::to_owned),
            Some(schema_symbol(name)),
        ));
    }
    schema
        .functions()
        .find(|function| {
            function.name == name
                || function
                    .name
                    .rsplit("::")
                    .next()
                    .is_some_and(|segment| segment == name)
        })
        .map(|function| {
            Hover::new(
                range,
                function.name.clone(),
                HoverKind::Function,
                function_detail_parts(schema, &function.name, &function.fact),
                schema.function_docs(&function.name).map(str::to_owned),
                Some(schema_symbol(function.name.clone())),
            )
        })
        .or_else(|| qualified_variant_hover(schema, name, range))
        .or_else(|| unique_variant_hover(schema, name, range))
}

fn qualified_variant_hover(
    schema: &RegistryFacts,
    name: &str,
    range: DiagnosticRange,
) -> Option<Hover> {
    let (owner, variant) = name.rsplit_once("::")?;
    schema.variant_fact(owner, variant).map(|fact| {
        Hover::new(
            range,
            name.to_owned(),
            HoverKind::Variant,
            DisplayParts::type_name(fact.display_name()),
            schema.variant_docs(owner, variant).map(str::to_owned),
            Some(schema_symbol(name)),
        )
    })
}

fn unique_variant_hover(
    schema: &RegistryFacts,
    name: &str,
    range: DiagnosticRange,
) -> Option<Hover> {
    let mut variants = schema.variants().filter(|variant| variant.name == name);
    let variant = variants.next()?;
    variants.next().is_none().then(|| {
        let label = format!("{}::{}", variant.owner, variant.name);
        Hover::new(
            range,
            label,
            HoverKind::Variant,
            DisplayParts::type_name(variant.fact.display_name()),
            schema
                .variant_docs(&variant.owner, &variant.name)
                .map(str::to_owned),
            Some(schema_variant_symbol(&variant.owner, &variant.name)),
        )
    })
}

fn method_fact<'a>(schema: &'a RegistryFacts, owner: &str, method: &str) -> Option<&'a TypeFact> {
    schema
        .method_fact(owner, method)
        .or_else(|| schema.trait_method_fact(owner, method))
}

fn method_docs<'a>(schema: &'a RegistryFacts, owner: &str, method: &str) -> Option<&'a str> {
    schema
        .method_docs(owner, method)
        .or_else(|| schema.trait_method_docs(owner, method))
}

fn function_detail_parts(schema: &RegistryFacts, name: &str, fact: &TypeFact) -> DisplayParts {
    let effects = schema
        .function_effect_fact(name)
        .map_or_else(|| "effects: unknown".to_owned(), effect_detail);
    typed_metadata_detail_parts(fact.display_name(), [effects])
}

fn method_detail_parts(
    schema: &RegistryFacts,
    owner: &str,
    method: &str,
    fact: &TypeFact,
) -> DisplayParts {
    let effects = schema
        .method_effect_fact(owner, method)
        .or_else(|| schema.trait_method_effect_fact(owner, method))
        .map_or_else(|| "effects: unknown".to_owned(), effect_detail);
    let permissions = schema.method_access_fact(owner, method).map_or_else(
        || "none".to_owned(),
        |access| permissions_detail(&access.required_permissions),
    );
    typed_metadata_detail_parts(
        fact.display_name(),
        [effects, format!("permissions: {permissions}")],
    )
}

fn field_detail_parts(
    fact: &TypeFact,
    access: &vela_analysis::registry::RegistryFieldAccessFact,
) -> DisplayParts {
    let permissions = permissions_detail(&access.required_permissions);
    typed_metadata_detail_parts(
        fact.display_name(),
        [
            format!("writable: {}", access.writable),
            format!("reflect_readable: {}", access.reflect_readable),
            format!("reflect_writable: {}", access.reflect_writable),
            format!("permissions: {permissions}"),
        ],
    )
}

fn typed_metadata_detail_parts(
    type_name: impl Into<String>,
    metadata: impl IntoIterator<Item = String>,
) -> DisplayParts {
    let mut parts = DisplayParts::type_name(type_name);
    for entry in metadata {
        parts.push(crate::DisplayPartKind::Punctuation, ";");
        parts.push(crate::DisplayPartKind::Text, " ");
        parts.push(crate::DisplayPartKind::Text, entry);
    }
    parts
}

fn effect_detail(effect: &RegistryEffectFact) -> String {
    format!("effects: {}", effect.display_name())
}

fn permissions_detail(permissions: &[String]) -> String {
    if permissions.is_empty() {
        "none".to_owned()
    } else {
        permissions.join(", ")
    }
}

fn owner_name(fact: &TypeFact) -> Option<String> {
    match fact {
        TypeFact::Host { name }
        | TypeFact::Record { name }
        | TypeFact::Enum { name, .. }
        | TypeFact::Trait { name } => Some(name.clone()),
        _ => None,
    }
}
