use std::collections::{BTreeMap, BTreeSet};

use vela_common::{Diagnostic, Span};

use crate::attributes::derived_traits;
use crate::binding::{BindingMap, LocalBindingKind};
use crate::ids::{HirDeclId, ModuleId};
use crate::type_hint::{
    EnumShape, EnumVariantFieldsHint, FunctionSignature, HirTypeHint, ImplMetadata,
    ImplMetadataKind, StructShape,
};

use super::names::{candidate_distance, import_binding_name};
use super::{DeclarationKind, ImportResolution, ModuleGraph};

pub(super) fn validate_once(graph: &mut ModuleGraph) {
    if graph.schema_references_validated {
        return;
    }
    graph.schema_references_validated = true;

    let mut diagnostics = Vec::new();
    for module in &graph.modules {
        diagnostics.extend(schema_reference_diagnostics_for_module(graph, module.id));
    }
    diagnostics.extend(duplicate_script_method_diagnostics(graph));
    diagnostics.extend(builtin_operator_trait_prerequisite_diagnostics(graph));
    diagnostics.extend(derived_operator_trait_diagnostics(graph));
    graph.diagnostics.extend(diagnostics);
}

fn duplicate_script_method_diagnostics(graph: &ModuleGraph) -> Vec<Diagnostic> {
    let mut methods: BTreeMap<(String, String), Span> = BTreeMap::new();
    let mut diagnostics = Vec::new();
    for declaration in graph.declarations.values() {
        let Some(metadata) = graph.impl_metadata.get(&declaration.id) else {
            continue;
        };
        let receiver = qualified_path_name(graph, declaration, &metadata.target_path);
        for method in &metadata.methods {
            let key = (receiver.clone(), method.name.clone());
            if let Some(previous_span) = methods.insert(key, method.span) {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "duplicate script method `{}.{}`",
                        receiver, method.name
                    ))
                    .with_code("hir::duplicate_script_method")
                    .with_span(method.span)
                    .with_label(previous_span, "previous method is here")
                    .with_label(method.span, "duplicate method is here"),
                );
            }
        }
    }
    diagnostics
}

fn builtin_operator_trait_prerequisite_diagnostics(graph: &ModuleGraph) -> Vec<Diagnostic> {
    let impls = builtin_operator_trait_impls(graph);

    let mut diagnostics = Vec::new();
    for ((receiver, trait_name), span) in &impls {
        match trait_name.as_str() {
            "Eq" => {
                push_missing_operator_trait_prerequisite(
                    &mut diagnostics,
                    &impls,
                    receiver,
                    "Eq",
                    "PartialEq",
                    *span,
                );
            }
            "Ord" => {
                push_missing_operator_trait_prerequisite(
                    &mut diagnostics,
                    &impls,
                    receiver,
                    "Ord",
                    "Eq",
                    *span,
                );
                push_missing_operator_trait_prerequisite(
                    &mut diagnostics,
                    &impls,
                    receiver,
                    "Ord",
                    "PartialOrd",
                    *span,
                );
            }
            "PartialEq" | "PartialOrd" => {}
            _ => unreachable!("builtin_operator_trait_name only returns known traits"),
        }
    }
    diagnostics
}

fn builtin_operator_trait_impls(graph: &ModuleGraph) -> BTreeMap<(String, String), Span> {
    let declared_types = declared_script_type_names(graph);
    let mut impls = BTreeMap::new();
    for declaration in graph.declarations.values() {
        let Some(metadata) = graph.impl_metadata.get(&declaration.id) else {
            continue;
        };
        let ImplMetadataKind::Trait { trait_path } = &metadata.kind else {
            continue;
        };
        let Some(trait_name) = builtin_operator_trait_name(trait_path) else {
            continue;
        };
        let receiver = qualified_path_name(graph, declaration, &metadata.target_path);
        if declared_types.contains(&receiver) {
            impls.insert((receiver, trait_name.to_owned()), declaration.span);
        }
    }
    impls
}

fn push_missing_operator_trait_prerequisite(
    diagnostics: &mut Vec<Diagnostic>,
    impls: &BTreeMap<(String, String), Span>,
    receiver: &str,
    trait_name: &'static str,
    prerequisite: &'static str,
    span: Span,
) {
    if impls.contains_key(&(receiver.to_owned(), prerequisite.to_owned())) {
        return;
    }
    diagnostics.push(
        Diagnostic::error(format!(
            "`{receiver}` implements `{trait_name}` without required `{prerequisite}`"
        ))
        .with_code("hir::missing_comparison_trait_prerequisite")
        .with_span(span)
        .with_label(
            span,
            format!("`{trait_name}` requires `{prerequisite}` for `{receiver}`"),
        ),
    );
}

fn derived_operator_trait_diagnostics(graph: &ModuleGraph) -> Vec<Diagnostic> {
    let impls = builtin_operator_trait_impls(graph);
    let derives = derived_operator_traits_by_type(graph);
    let declared_types = declared_script_type_names(graph);
    let mut diagnostics = Vec::new();

    for declaration in graph.declarations.values() {
        if declaration.kind != DeclarationKind::Struct {
            continue;
        }
        let type_name =
            qualified_path_name(graph, declaration, std::slice::from_ref(&declaration.name));
        let traits = derived_traits(graph.declaration_attrs(declaration.id));
        if traits.is_empty() {
            continue;
        }

        push_missing_derived_operator_trait_prerequisite(
            &mut diagnostics,
            OperatorTraitLookup {
                impls: &impls,
                derives: &derives,
            },
            &type_name,
            &traits,
            "Eq",
            "PartialEq",
            declaration.span,
        );
        push_missing_derived_operator_trait_prerequisite(
            &mut diagnostics,
            OperatorTraitLookup {
                impls: &impls,
                derives: &derives,
            },
            &type_name,
            &traits,
            "PartialOrd",
            "PartialEq",
            declaration.span,
        );
        push_missing_derived_operator_trait_prerequisite(
            &mut diagnostics,
            OperatorTraitLookup {
                impls: &impls,
                derives: &derives,
            },
            &type_name,
            &traits,
            "Ord",
            "Eq",
            declaration.span,
        );
        push_missing_derived_operator_trait_prerequisite(
            &mut diagnostics,
            OperatorTraitLookup {
                impls: &impls,
                derives: &derives,
            },
            &type_name,
            &traits,
            "Ord",
            "PartialOrd",
            declaration.span,
        );

        let Some(shape) = graph.struct_shape(declaration.id) else {
            continue;
        };
        for trait_name in traits
            .iter()
            .filter_map(|trait_name| builtin_operator_trait_name(std::slice::from_ref(trait_name)))
        {
            for field in &shape.fields {
                if field_supports_operator_trait(
                    graph,
                    declaration,
                    field,
                    trait_name,
                    &declared_types,
                    &impls,
                    &derives,
                ) {
                    continue;
                }
                diagnostics.push(
                    Diagnostic::error(format!(
                        "`{type_name}` cannot derive `{trait_name}` because field `{}` does not satisfy `{trait_name}`",
                        field.name
                    ))
                    .with_code("hir::unsupported_comparison_derive_field")
                    .with_span(field.span)
                    .with_label(
                        field.span,
                        format!("field `{}` must have `{trait_name}` support", field.name),
                    ),
                );
            }
        }
    }

    diagnostics
}

fn derived_operator_traits_by_type(graph: &ModuleGraph) -> BTreeMap<(String, String), Span> {
    let mut derives = BTreeMap::new();
    for declaration in graph.declarations.values() {
        if declaration.kind != DeclarationKind::Struct {
            continue;
        }
        let type_name =
            qualified_path_name(graph, declaration, std::slice::from_ref(&declaration.name));
        for trait_name in derived_traits(graph.declaration_attrs(declaration.id)) {
            if builtin_operator_trait_name(std::slice::from_ref(&trait_name)).is_some() {
                derives.insert((type_name.clone(), trait_name), declaration.span);
            }
        }
    }
    derives
}

fn push_missing_derived_operator_trait_prerequisite(
    diagnostics: &mut Vec<Diagnostic>,
    lookup: OperatorTraitLookup<'_>,
    receiver: &str,
    traits: &BTreeSet<String>,
    trait_name: &'static str,
    prerequisite: &'static str,
    span: Span,
) {
    if !traits.contains(trait_name)
        || type_has_operator_trait(receiver, prerequisite, lookup.impls, lookup.derives)
    {
        return;
    }
    diagnostics.push(
        Diagnostic::error(format!(
            "`{receiver}` derives `{trait_name}` without required `{prerequisite}`"
        ))
        .with_code("hir::missing_comparison_derive_prerequisite")
        .with_span(span)
        .with_label(
            span,
            format!("`derive({trait_name})` requires `{prerequisite}` for `{receiver}`"),
        ),
    );
}

#[derive(Clone, Copy)]
struct OperatorTraitLookup<'a> {
    impls: &'a BTreeMap<(String, String), Span>,
    derives: &'a BTreeMap<(String, String), Span>,
}

fn type_has_operator_trait(
    type_name: &str,
    trait_name: &str,
    impls: &BTreeMap<(String, String), Span>,
    derives: &BTreeMap<(String, String), Span>,
) -> bool {
    let key = (type_name.to_owned(), trait_name.to_owned());
    impls.contains_key(&key) || derives.contains_key(&key)
}

fn field_supports_operator_trait(
    graph: &ModuleGraph,
    owner: &super::Declaration,
    field: &crate::type_hint::StructFieldHint,
    trait_name: &str,
    declared_types: &BTreeSet<String>,
    impls: &BTreeMap<(String, String), Span>,
    derives: &BTreeMap<(String, String), Span>,
) -> bool {
    let Some(hint) = &field.type_hint else {
        return false;
    };
    let type_name = qualified_hint_name(graph, owner, hint);
    if builtin_hint_supports_operator_trait(&type_name, trait_name) {
        return true;
    }
    declared_types.contains(&type_name)
        && type_has_operator_trait(&type_name, trait_name, impls, derives)
}

fn qualified_hint_name(
    graph: &ModuleGraph,
    owner: &super::Declaration,
    hint: &HirTypeHint,
) -> String {
    if is_builtin_type_hint(&hint.path) || !hint.args.is_empty() {
        hint.display()
    } else {
        qualified_path_name(graph, owner, &hint.path)
    }
}

fn builtin_hint_supports_operator_trait(type_name: &str, trait_name: &str) -> bool {
    match trait_name {
        "PartialEq" => matches!(
            type_name,
            "bool"
                | "char"
                | "i8"
                | "i16"
                | "i32"
                | "i64"
                | "u8"
                | "u16"
                | "u32"
                | "u64"
                | "f32"
                | "f64"
                | "String"
                | "Bytes"
        ),
        "Eq" | "Ord" => matches!(
            type_name,
            "bool"
                | "char"
                | "i8"
                | "i16"
                | "i32"
                | "i64"
                | "u8"
                | "u16"
                | "u32"
                | "u64"
                | "String"
                | "Bytes"
        ),
        "PartialOrd" => matches!(
            type_name,
            "bool"
                | "char"
                | "i8"
                | "i16"
                | "i32"
                | "i64"
                | "u8"
                | "u16"
                | "u32"
                | "u64"
                | "f32"
                | "f64"
                | "String"
                | "Bytes"
        ),
        _ => false,
    }
}

fn declared_script_type_names(graph: &ModuleGraph) -> BTreeSet<String> {
    graph
        .declarations
        .values()
        .filter(|declaration| {
            matches!(
                declaration.kind,
                DeclarationKind::Struct | DeclarationKind::Enum
            )
        })
        .map(|declaration| {
            qualified_path_name(graph, declaration, std::slice::from_ref(&declaration.name))
        })
        .collect()
}

fn schema_reference_diagnostics_for_module(
    graph: &ModuleGraph,
    module: ModuleId,
) -> Vec<Diagnostic> {
    let candidates = visible_schema_candidates(graph, module);
    let mut diagnostics = Vec::new();

    for declaration in graph.declarations.values() {
        if declaration.module != module {
            continue;
        }
        if let Some(signature) = graph.function_signatures.get(&declaration.id) {
            diagnostics.extend(signature_schema_diagnostics(signature, &candidates));
        }
        if let Some(metadata) = graph.const_metadata.get(&declaration.id)
            && let Some(type_hint) = &metadata.type_hint
        {
            diagnostics.extend(schema_hint_diagnostics(type_hint, &candidates, None));
        }
        if let Some(shape) = graph.struct_shapes.get(&declaration.id) {
            diagnostics.extend(struct_shape_schema_diagnostics(shape, &candidates));
        }
        if let Some(shape) = graph.enum_shapes.get(&declaration.id) {
            diagnostics.extend(enum_shape_schema_diagnostics(shape, &candidates));
        }
        if let Some(shape) = graph.trait_shapes.get(&declaration.id) {
            for method in &shape.methods {
                diagnostics.extend(signature_schema_diagnostics(&method.signature, &candidates));
            }
        }
        if let Some(metadata) = graph.impl_metadata.get(&declaration.id) {
            diagnostics.extend(impl_schema_diagnostics(
                metadata,
                declaration.span,
                &candidates,
            ));
        }
    }

    for bindings in graph
        .bindings
        .values()
        .chain(graph.impl_method_bindings.values())
    {
        if graph
            .declarations
            .get(&bindings.declaration)
            .is_some_and(|declaration| declaration.module == module)
        {
            diagnostics.extend(binding_schema_diagnostics(bindings, &candidates));
        }
    }
    for bindings in graph.trait_default_method_bindings.values() {
        if graph
            .declarations
            .get(&bindings.declaration)
            .is_some_and(|declaration| declaration.module == module)
        {
            diagnostics.extend(binding_schema_diagnostics(bindings, &candidates));
        }
    }

    diagnostics
}

fn signature_schema_diagnostics(
    signature: &FunctionSignature,
    candidates: &[SchemaCandidate],
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for param in &signature.params {
        if let Some(type_hint) = &param.type_hint {
            diagnostics.extend(schema_hint_diagnostics(type_hint, candidates, None));
        }
    }
    if let Some(type_hint) = &signature.return_type {
        diagnostics.extend(schema_hint_diagnostics(type_hint, candidates, None));
    }
    diagnostics
}

fn struct_shape_schema_diagnostics(
    shape: &StructShape,
    candidates: &[SchemaCandidate],
) -> Vec<Diagnostic> {
    shape
        .fields
        .iter()
        .filter_map(|field| field.type_hint.as_ref())
        .flat_map(|hint| schema_hint_diagnostics(hint, candidates, None))
        .collect()
}

fn enum_shape_schema_diagnostics(
    shape: &EnumShape,
    candidates: &[SchemaCandidate],
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for variant in &shape.variants {
        match &variant.fields {
            EnumVariantFieldsHint::Unit => {}
            EnumVariantFieldsHint::Tuple(params) => {
                for param in params {
                    if let Some(type_hint) = &param.type_hint {
                        diagnostics.extend(schema_hint_diagnostics(type_hint, candidates, None));
                    }
                }
            }
            EnumVariantFieldsHint::Record(fields) => {
                for field in fields {
                    if let Some(type_hint) = &field.type_hint {
                        diagnostics.extend(schema_hint_diagnostics(type_hint, candidates, None));
                    }
                }
            }
        }
    }
    diagnostics
}

fn impl_schema_diagnostics(
    metadata: &ImplMetadata,
    span: Span,
    candidates: &[SchemaCandidate],
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    if let ImplMetadataKind::Trait { trait_path } = &metadata.kind {
        diagnostics.extend(schema_path_diagnostics(
            trait_path,
            span,
            candidates,
            Some(&[DeclarationKind::Trait]),
            "trait",
        ));
    }
    diagnostics.extend(schema_path_diagnostics(
        &metadata.target_path,
        span,
        candidates,
        Some(&[DeclarationKind::Struct, DeclarationKind::Enum]),
        "schema",
    ));
    for method in &metadata.methods {
        diagnostics.extend(signature_schema_diagnostics(&method.signature, candidates));
    }
    diagnostics
}

fn binding_schema_diagnostics(
    bindings: &BindingMap,
    candidates: &[SchemaCandidate],
) -> Vec<Diagnostic> {
    bindings
        .locals()
        .filter(|local| local.kind != LocalBindingKind::Parameter)
        .filter_map(|local| local.type_hint.as_ref())
        .flat_map(|hint| schema_hint_diagnostics(hint, candidates, None))
        .collect()
}

fn schema_hint_diagnostics(
    hint: &HirTypeHint,
    candidates: &[SchemaCandidate],
    allowed_kinds: Option<&[DeclarationKind]>,
) -> Vec<Diagnostic> {
    let mut diagnostics =
        schema_path_diagnostics(&hint.path, hint.span, candidates, allowed_kinds, "schema");
    for arg in &hint.args {
        diagnostics.extend(schema_hint_diagnostics(arg, candidates, allowed_kinds));
    }
    diagnostics
}

fn schema_path_diagnostics(
    path: &[String],
    span: Span,
    candidates: &[SchemaCandidate],
    allowed_kinds: Option<&[DeclarationKind]>,
    noun: &str,
) -> Vec<Diagnostic> {
    if path.is_empty() || is_builtin_type_hint(path) || is_builtin_operator_trait(path) {
        return Vec::new();
    }
    let wanted = path.join("::");
    if candidates.iter().any(|candidate| {
        candidate.name == wanted && schema_kind_allowed(candidate.kind, allowed_kinds)
    }) {
        return Vec::new();
    }

    let ranked = ranked_schema_candidates(&wanted, candidates, allowed_kinds);
    if ranked.is_empty() {
        return Vec::new();
    }
    let mut diagnostic = Diagnostic::error(format!("unknown {noun} `{wanted}`"))
        .with_code("hir::unknown_schema")
        .with_span(span)
        .with_label(
            span,
            format!("`{wanted}` does not resolve to a known {noun}"),
        );
    for candidate in ranked {
        diagnostic = diagnostic.with_label(
            candidate.span,
            format!("candidate `{}` is declared here", candidate.name),
        );
    }
    vec![diagnostic]
}

fn visible_schema_candidates(graph: &ModuleGraph, module: ModuleId) -> Vec<SchemaCandidate> {
    let mut candidates = BTreeMap::<String, SchemaCandidate>::new();
    if let Some(declarations) = graph.module(module) {
        for name in declarations.names() {
            if let Some(declaration) = declarations.get(name) {
                insert_schema_candidate(graph, &mut candidates, name.to_owned(), declaration);
            }
        }
    }

    if let Some(imports) = graph.imports(module) {
        for import in imports {
            let Some(ImportResolution::Declaration(declaration)) = import.resolution else {
                continue;
            };
            let Some(name) = import_binding_name(import) else {
                continue;
            };
            insert_schema_candidate(graph, &mut candidates, name, declaration);
        }
    }

    for (path, declaration) in graph.qualified_declarations_for(module) {
        insert_schema_candidate(graph, &mut candidates, path.join("::"), declaration);
    }

    candidates.into_values().collect()
}

fn insert_schema_candidate(
    graph: &ModuleGraph,
    candidates: &mut BTreeMap<String, SchemaCandidate>,
    name: String,
    declaration: HirDeclId,
) {
    let Some(metadata) = graph.declaration(declaration) else {
        return;
    };
    if !is_schema_declaration(metadata.kind) {
        return;
    }
    candidates.entry(name.clone()).or_insert(SchemaCandidate {
        name,
        kind: metadata.kind,
        span: metadata.span,
    });
}

fn qualified_path_name(graph: &ModuleGraph, owner: &super::Declaration, path: &[String]) -> String {
    if path.len() != 1 {
        return path.join("::");
    }
    let Some(module_path) = graph.module_path(owner.module) else {
        return path[0].clone();
    };
    if module_path.segments().is_empty() {
        path[0].clone()
    } else {
        format!("{}::{}", module_path.join(), path[0])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SchemaCandidate {
    name: String,
    kind: DeclarationKind,
    span: Span,
}

fn is_builtin_type_hint(path: &[String]) -> bool {
    let [name] = path else {
        return false;
    };
    matches!(
        name.as_str(),
        "Any"
            | "null"
            | "bool"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "f32"
            | "f64"
            | "String"
            | "Bytes"
            | "Array"
            | "Map"
            | "Set"
            | "Range"
            | "Iterator"
            | "Function"
            | "Option"
            | "Result"
    )
}

fn is_builtin_operator_trait(path: &[String]) -> bool {
    builtin_operator_trait_name(path).is_some()
}

fn builtin_operator_trait_name(path: &[String]) -> Option<&str> {
    let [name] = path else {
        return None;
    };
    matches!(name.as_str(), "PartialEq" | "Eq" | "PartialOrd" | "Ord").then_some(name.as_str())
}

fn is_schema_declaration(kind: DeclarationKind) -> bool {
    matches!(
        kind,
        DeclarationKind::Struct | DeclarationKind::Enum | DeclarationKind::Trait
    )
}

fn schema_kind_allowed(kind: DeclarationKind, allowed: Option<&[DeclarationKind]>) -> bool {
    allowed.is_none_or(|allowed| allowed.contains(&kind))
}

fn ranked_schema_candidates<'a>(
    wanted: &str,
    candidates: &'a [SchemaCandidate],
    allowed_kinds: Option<&[DeclarationKind]>,
) -> Vec<&'a SchemaCandidate> {
    let mut ranked = candidates
        .iter()
        .filter(|candidate| schema_kind_allowed(candidate.kind, allowed_kinds))
        .map(|candidate| (candidate_distance(wanted, &candidate.name), candidate))
        .filter(|(distance, _)| *distance <= 3)
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.name.cmp(&right.1.name))
    });
    ranked
        .into_iter()
        .take(3)
        .map(|(_, candidate)| candidate)
        .collect()
}
