use vela_analysis::registry::{RegistryEffectFact, RegistryFacts};
use vela_analysis::type_fact::TypeFact;

use crate::{DiagnosticRange, DisplayParts, SymbolRef};

use super::{Hover, HoverKind};

pub(super) fn member_hover(
    schema: &RegistryFacts,
    receiver_fact: &TypeFact,
    member: &str,
    range: DiagnosticRange,
) -> Option<Hover> {
    let owner = owner_name(receiver_fact)?;
    if let Some(fact) = schema.field_fact(&owner, member) {
        let detail = schema.field_access_fact(&owner, member).map_or_else(
            || fact.display_name(),
            |access| {
                let permissions = permissions_detail(&access.required_permissions);
                format!(
                    "{}; writable: {}; reflect_readable: {}; reflect_writable: {}; permissions: {permissions}",
                    fact.display_name(),
                    access.writable,
                    access.reflect_readable,
                    access.reflect_writable
                )
            },
        );
        return Some(Hover::plain_detail(
            range,
            format!("{owner}.{member}"),
            HoverKind::Field,
            detail,
            schema.field_docs(&owner, member).map(str::to_owned),
            Some(SymbolRef::Schema(format!("{owner}.{member}"))),
        ));
    }
    method_fact(schema, &owner, member).map(|fact| {
        Hover::plain_detail(
            range,
            format!("{owner}.{member}"),
            HoverKind::Method,
            method_detail(schema, &owner, member, fact),
            method_docs(schema, &owner, member).map(str::to_owned),
            Some(SymbolRef::Schema(format!("{owner}.{member}"))),
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
            Some(SymbolRef::Schema(name.to_owned())),
        ));
    }
    if let Some(fact) = schema.trait_fact(name) {
        return Some(Hover::new(
            range,
            name.to_owned(),
            HoverKind::Trait,
            DisplayParts::type_name(fact.display_name()),
            schema.trait_docs(name).map(str::to_owned),
            Some(SymbolRef::Schema(name.to_owned())),
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
            Hover::plain_detail(
                range,
                function.name.clone(),
                HoverKind::Function,
                function_detail(schema, &function.name, &function.fact),
                schema.function_docs(&function.name).map(str::to_owned),
                Some(SymbolRef::Schema(function.name.clone())),
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
            Some(SymbolRef::Schema(name.to_owned())),
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
            Some(SymbolRef::Schema(format!(
                "{}::{}",
                variant.owner, variant.name
            ))),
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

fn function_detail(schema: &RegistryFacts, name: &str, fact: &TypeFact) -> String {
    let effects = schema
        .function_effect_fact(name)
        .map_or_else(|| "effects: unknown".to_owned(), effect_detail);
    format!("{}; {effects}", fact.display_name())
}

fn method_detail(schema: &RegistryFacts, owner: &str, method: &str, fact: &TypeFact) -> String {
    let effects = schema
        .method_effect_fact(owner, method)
        .or_else(|| schema.trait_method_effect_fact(owner, method))
        .map_or_else(|| "effects: unknown".to_owned(), effect_detail);
    let permissions = schema.method_access_fact(owner, method).map_or_else(
        || "none".to_owned(),
        |access| permissions_detail(&access.required_permissions),
    );
    format!(
        "{}; {effects}; permissions: {permissions}",
        fact.display_name()
    )
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
