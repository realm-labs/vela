mod detail;
#[cfg(test)]
mod tests;

use vela_common::Span;
use vela_reflect::modules::{FunctionDesc, ModuleDesc};
use vela_reflect::registry::{
    AttrMap, FieldDesc, MethodDesc, TraitDesc, TypeDesc, TypeRegistry, VariantDesc,
};

use detail::{
    field_detail, function_detail, method_detail, module_detail, trait_detail, trait_method_detail,
    type_detail, variant_detail,
};

use crate::registry::RegistryFacts;
use crate::type_fact::TypeFact;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HoverKind {
    Type,
    Field,
    Method,
    Function,
    Trait,
    TraitMethod,
    Variant,
    Module,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HoverInfo {
    pub label: String,
    pub kind: HoverKind,
    pub fact: TypeFact,
    pub docs: Option<String>,
    pub detail: Option<String>,
    pub attrs: Vec<(String, String)>,
    pub source_span: Option<Span>,
}

impl HoverInfo {
    fn new(label: impl Into<String>, kind: HoverKind, fact: TypeFact) -> Self {
        Self {
            label: label.into(),
            kind,
            fact,
            docs: None,
            detail: None,
            attrs: Vec::new(),
            source_span: None,
        }
    }

    fn docs(mut self, docs: &Option<String>) -> Self {
        self.docs.clone_from(docs);
        self
    }

    fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    fn attrs(mut self, attrs: &AttrMap) -> Self {
        self.attrs = attrs
            .iter()
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .collect();
        self
    }

    fn source_span(mut self, source_span: Option<Span>) -> Self {
        self.source_span = source_span;
        self
    }
}

pub fn type_hover(registry: &TypeRegistry, name: &str) -> Option<HoverInfo> {
    let desc = registry.type_by_name(name)?;
    let facts = RegistryFacts::from_registry(registry);
    Some(type_hover_from_desc(desc, &facts))
}

pub fn field_hover(registry: &TypeRegistry, owner: &str, field: &str) -> Option<HoverInfo> {
    let desc = field_desc(registry, owner, field)?;
    let facts = RegistryFacts::from_registry(registry);
    let fact = facts
        .field_fact(owner, field)
        .cloned()
        .unwrap_or(TypeFact::Unknown);
    Some(
        HoverInfo::new(format!("{owner}.{field}"), HoverKind::Field, fact)
            .docs(&desc.docs)
            .detail(field_detail(desc))
            .attrs(&desc.attrs)
            .source_span(desc.source_span),
    )
}

pub fn method_hover(registry: &TypeRegistry, owner: &str, method: &str) -> Option<HoverInfo> {
    let desc = registry
        .type_by_name(owner)?
        .methods
        .iter()
        .find(|desc| desc.name == method)?;
    let facts = RegistryFacts::from_registry(registry);
    let fact = facts
        .method_fact(owner, method)
        .cloned()
        .unwrap_or(TypeFact::Unknown);
    Some(method_hover_from_desc(owner, desc, fact))
}

pub fn function_hover(registry: &TypeRegistry, name: &str) -> Option<HoverInfo> {
    let desc = registry.function_by_name(name)?;
    let facts = RegistryFacts::from_registry(registry);
    let fact = facts
        .function_fact(name)
        .cloned()
        .unwrap_or(TypeFact::Unknown);
    Some(function_hover_from_desc(desc, fact))
}

pub fn trait_hover(registry: &TypeRegistry, name: &str) -> Option<HoverInfo> {
    let desc = registry.trait_by_name(name)?;
    Some(trait_hover_from_desc(desc))
}

pub fn trait_method_hover(
    registry: &TypeRegistry,
    trait_name: &str,
    method: &str,
) -> Option<HoverInfo> {
    let desc = registry
        .trait_by_name(trait_name)?
        .methods
        .iter()
        .find(|desc| desc.name == method)?;
    let facts = RegistryFacts::from_registry(registry);
    let fact = facts
        .trait_method_fact(trait_name, method)
        .cloned()
        .unwrap_or(TypeFact::Unknown);
    Some(
        HoverInfo::new(
            format!("{trait_name}.{method}"),
            HoverKind::TraitMethod,
            fact,
        )
        .docs(&desc.docs)
        .detail(trait_method_detail(desc))
        .attrs(&desc.attrs)
        .source_span(desc.source_span),
    )
}

pub fn variant_hover(registry: &TypeRegistry, owner: &str, variant: &str) -> Option<HoverInfo> {
    let desc = registry
        .type_by_name(owner)?
        .variants
        .iter()
        .find(|desc| desc.name == variant)?;
    let facts = RegistryFacts::from_registry(registry);
    let fact = facts
        .variant_fact(owner, variant)
        .cloned()
        .unwrap_or(TypeFact::Unknown);
    Some(variant_hover_from_desc(owner, desc, fact))
}

pub fn module_hover(registry: &TypeRegistry, name: &str) -> Option<HoverInfo> {
    let desc = registry.module_by_name(name)?;
    Some(module_hover_from_desc(desc))
}

fn type_hover_from_desc(desc: &TypeDesc, facts: &RegistryFacts) -> HoverInfo {
    let fact = facts
        .type_fact(&desc.key.name)
        .cloned()
        .unwrap_or(TypeFact::Unknown);
    HoverInfo::new(&desc.key.name, HoverKind::Type, fact)
        .docs(&desc.docs)
        .detail(type_detail(desc.kind))
        .attrs(&desc.attrs)
        .source_span(desc.source_span)
}

fn method_hover_from_desc(owner: &str, desc: &MethodDesc, fact: TypeFact) -> HoverInfo {
    HoverInfo::new(format!("{owner}.{}", desc.name), HoverKind::Method, fact)
        .docs(&desc.docs)
        .detail(method_detail(desc))
        .attrs(&desc.attrs)
        .source_span(desc.source_span)
}

fn function_hover_from_desc(desc: &FunctionDesc, fact: TypeFact) -> HoverInfo {
    HoverInfo::new(&desc.name, HoverKind::Function, fact)
        .docs(&desc.docs)
        .detail(function_detail(desc))
        .attrs(&desc.attrs)
        .source_span(desc.source_span)
}

fn trait_hover_from_desc(desc: &TraitDesc) -> HoverInfo {
    HoverInfo::new(
        &desc.name,
        HoverKind::Trait,
        TypeFact::trait_type(&desc.name),
    )
    .docs(&desc.docs)
    .detail(trait_detail(desc))
    .attrs(&desc.attrs)
    .source_span(desc.source_span)
}

fn variant_hover_from_desc(owner: &str, desc: &VariantDesc, fact: TypeFact) -> HoverInfo {
    HoverInfo::new(format!("{owner}.{}", desc.name), HoverKind::Variant, fact)
        .docs(&desc.docs)
        .detail(variant_detail(desc))
        .attrs(&desc.attrs)
        .source_span(desc.source_span)
}

fn module_hover_from_desc(desc: &ModuleDesc) -> HoverInfo {
    HoverInfo::new(&desc.name, HoverKind::Module, TypeFact::module(&desc.name))
        .detail(module_detail(desc))
        .attrs(&desc.attrs)
        .source_span(desc.source_span)
}

fn field_desc<'a>(registry: &'a TypeRegistry, owner: &str, field: &str) -> Option<&'a FieldDesc> {
    registry
        .type_by_name(owner)
        .and_then(|desc| desc.fields.iter().find(|desc| desc.name == field))
        .or_else(|| variant_field_desc(registry, owner, field))
}

fn variant_field_desc<'a>(
    registry: &'a TypeRegistry,
    owner: &str,
    field: &str,
) -> Option<&'a FieldDesc> {
    let (enum_name, variant_name) = owner.rsplit_once('.')?;
    registry
        .type_by_name(enum_name)?
        .variants
        .iter()
        .find(|desc| desc.name == variant_name)?
        .fields
        .iter()
        .find(|desc| desc.name == field)
}
