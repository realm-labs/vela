use vela_analysis::facts::AnalysisFacts;
mod schema;

use vela_analysis::registry::RegistryFacts;
use vela_analysis::stdlib::{
    StdlibFunctionFact, StdlibMethodFact, stdlib_function_completion_facts, stdlib_method_fact,
};
use vela_analysis::type_fact::TypeFact;
use vela_common::Span;
use vela_hir::attributes::HirAttribute;
use vela_hir::binding::{BindingMap, BindingResolution, LocalBinding, LocalBindingKind};
use vela_hir::module_graph::{
    Declaration, DeclarationKind, Import, ImportResolution, ModuleGraph, ModulePath,
};
use vela_hir::type_hint::{
    EnumVariantFieldsHint, EnumVariantHint, FunctionSignature, ImplMetadataKind,
    ImplMethodMetadata, StructFieldHint, TraitMethodMetadata,
};

use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, LineIndex, Position, TextRange,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum HoverKind {
    Local,
    Parameter,
    Const,
    Function,
    Type,
    Field,
    Method,
    Variant,
    Module,
    Unknown,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Hover {
    range: DiagnosticRange,
    label: String,
    kind: HoverKind,
    detail: String,
    docs: Option<String>,
}

impl Hover {
    #[must_use]
    pub const fn range(&self) -> DiagnosticRange {
        self.range
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn kind(&self) -> HoverKind {
        self.kind
    }

    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }

    #[must_use]
    pub fn docs(&self) -> Option<&str> {
        self.docs.as_deref()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct HoverToken {
    text: String,
    range: TextRange,
    member_receiver: Option<TextRange>,
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn hover(&self, document_id: &DocumentId, position: Position) -> Option<Hover> {
        let source = self.source_db().records().get(document_id)?;
        let token = hover_token_at(source.text(), position)?;
        let source_id = source.source_id();
        let offset = u32::try_from(token.range.start).ok()?;
        let range = diagnostic_range(source.text(), token.range);
        let graph = self.hir_db().graph();
        let facts = AnalysisFacts::from_module_graph(graph);

        if let Some(receiver) = token.member_receiver
            && let Some(hover) = self.member_hover(document_id, receiver, &token, range)
        {
            return Some(hover);
        }

        for declaration in graph.declarations() {
            if declaration.span.source != source_id || !declaration.span.contains(offset) {
                continue;
            }
            if let Some(bindings) = graph.bindings(declaration.id) {
                if let Some(hover) =
                    hover_from_resolution_at_token(bindings, &facts, &token, range, self)
                {
                    return Some(hover);
                }
                if let Some(hover) = hover_from_local_declaration(bindings, &facts, &token, range) {
                    return Some(hover);
                }
            }
        }

        if let Some(hover) =
            self.import_hover(document_id, source.text(), source_id, &facts, &token, range)
        {
            return Some(hover);
        }

        if let Some(hover) = struct_field_hover_at_token(graph, source_id, offset, &token, range) {
            return Some(hover);
        }

        if let Some(hover) =
            script_method_hover_at_token(graph, source.text(), source_id, &token, range)
        {
            return Some(hover);
        }

        if let Some(hover) = enum_variant_hover_at_token(graph, source_id, offset, &token, range) {
            return Some(hover);
        }

        if let Some(hover) = graph
            .declarations()
            .find(|declaration| {
                declaration.span.source == source_id
                    && declaration.span.contains(offset)
                    && declaration.name == token.text
            })
            .map(|declaration| hover_from_declaration(graph, &facts, declaration, range))
        {
            return Some(hover);
        }

        schema::symbol_hover(self.schema_db().facts(), &token.text, range)
            .or_else(|| stdlib_function_hover(&token.text, range))
            .or_else(|| type_hint_hover(self.schema_db().facts(), &token.text, range))
    }

    fn member_hover(
        &self,
        document_id: &DocumentId,
        receiver: TextRange,
        token: &HoverToken,
        range: DiagnosticRange,
    ) -> Option<Hover> {
        let receiver_fact = member_receiver_fact(self, document_id, receiver)?;
        if let Some(hover) =
            script_member_hover(self.hir_db().graph(), &receiver_fact, &token.text, range)
        {
            return Some(hover);
        }
        if let Some(hover) =
            script_method_hover(self.hir_db().graph(), &receiver_fact, &token.text, range)
        {
            return Some(hover);
        }
        if let Some(hover) =
            script_trait_method_hover(self.hir_db().graph(), &receiver_fact, &token.text, range)
        {
            return Some(hover);
        }
        if let Some(hover) =
            schema::member_hover(self.schema_db().facts(), &receiver_fact, &token.text, range)
        {
            return Some(hover);
        }
        stdlib_method_hover(&receiver_fact, &token.text, range)
    }

    fn import_hover(
        &self,
        document_id: &DocumentId,
        text: &str,
        source_id: vela_common::SourceId,
        facts: &AnalysisFacts,
        token: &HoverToken,
        range: DiagnosticRange,
    ) -> Option<Hover> {
        let graph = self.hir_db().graph();
        let module_path = self.project_db().module_by_document().get(document_id)?;
        let module = graph.module_id(module_path)?;
        graph.imports(module)?.iter().find_map(|import| {
            if import.span.source != source_id {
                return None;
            }
            let segment = import_path_segment_at(text, import, token)?;
            if segment + 1 == import.path.len() {
                let ImportResolution::Declaration(declaration) = import.resolution?;
                let declaration = graph.declaration(declaration)?;
                return Some(hover_from_declaration(graph, facts, declaration, range));
            }
            module_hover(graph, &import.path[..=segment], range)
        })
    }
}

fn module_hover(graph: &ModuleGraph, path: &[String], range: DiagnosticRange) -> Option<Hover> {
    let module_path = ModulePath::new(path.iter().cloned());
    graph.module_id(&module_path)?;
    let label = module_path.join();
    Some(Hover {
        range,
        label: label.clone(),
        kind: HoverKind::Module,
        detail: format!("module {label}"),
        docs: None,
    })
}

fn import_path_segment_at(text: &str, import: &Import, token: &HoverToken) -> Option<usize> {
    let range = span_text_range(import.span)?;
    if token.range.start < range.start || range.end < token.range.end {
        return None;
    }
    let slice = text.get(range.start..range.end)?;
    let path_text = import.path.join("::");
    slice.match_indices(&path_text).find_map(|(relative, _)| {
        let mut segment_start = range.start + relative;
        for (index, segment) in import.path.iter().enumerate() {
            let segment_end = segment_start + segment.len();
            if segment_start <= token.range.start && token.range.end <= segment_end {
                return Some(index);
            }
            segment_start = segment_end + "::".len();
        }
        None
    })
}

fn stdlib_function_hover(name: &str, range: DiagnosticRange) -> Option<Hover> {
    stdlib_function_completion_facts()
        .into_iter()
        .find(|function| {
            function.name == name
                || function
                    .name
                    .rsplit("::")
                    .next()
                    .is_some_and(|segment| segment == name)
        })
        .map(|function| Hover {
            range,
            label: function.name.to_owned(),
            kind: HoverKind::Function,
            detail: stdlib_function_detail(&function),
            docs: None,
        })
}

fn stdlib_method_hover(receiver: &TypeFact, method: &str, range: DiagnosticRange) -> Option<Hover> {
    stdlib_method_fact(receiver, method, None).map(|fact| Hover {
        range,
        label: format!("{}.{}", receiver.display_name(), fact.method),
        kind: HoverKind::Method,
        detail: stdlib_method_detail(&fact),
        docs: None,
    })
}

fn hover_from_resolution_at_token(
    bindings: &BindingMap,
    facts: &AnalysisFacts,
    token: &HoverToken,
    range: DiagnosticRange,
    databases: &LanguageServiceDatabases,
) -> Option<Hover> {
    let graph = databases.hir_db().graph();
    let resolution = bindings
        .resolutions()
        .find_map(|(expression, resolution)| {
            let expression = bindings.expression(expression)?;
            let start = usize::try_from(expression.span.start).ok()?;
            let end = usize::try_from(expression.span.end).ok()?;
            (expression.span.source == graph.declaration(bindings.declaration)?.span.source
                && start <= token.range.start
                && token.range.end <= end)
                .then_some(resolution)
        })?;
    match resolution {
        BindingResolution::Local(local) => {
            let binding = bindings.local(*local)?;
            let fact = local_fact(binding, facts).unwrap_or(TypeFact::Unknown);
            Some(local_hover(binding, fact, range))
        }
        BindingResolution::Declaration(declaration) => {
            graph.declaration(*declaration).map(|declaration| {
                enum_variant_hover_for_declaration(graph, declaration, &token.text, range)
                    .unwrap_or_else(|| hover_from_declaration(graph, facts, declaration, range))
            })
        }
        BindingResolution::Import(name) => Some(Hover {
            range,
            label: name.clone(),
            kind: HoverKind::Unknown,
            detail: "unresolved import".to_owned(),
            docs: None,
        }),
        BindingResolution::QualifiedPath(path) => {
            let qualified = path.join("::");
            schema::symbol_hover(databases.schema_db().facts(), &qualified, range)
                .or_else(|| stdlib_function_hover(&qualified, range))
                .or_else(|| {
                    Some(Hover {
                        range,
                        label: qualified,
                        kind: HoverKind::Unknown,
                        detail: "unresolved qualified path".to_owned(),
                        docs: None,
                    })
                })
        }
    }
}

fn enum_variant_hover_at_token(
    graph: &vela_hir::module_graph::ModuleGraph,
    source_id: vela_common::SourceId,
    offset: u32,
    token: &HoverToken,
    range: DiagnosticRange,
) -> Option<Hover> {
    graph.declarations().find_map(|declaration| {
        if declaration.kind != DeclarationKind::Enum {
            return None;
        }
        graph
            .enum_shape(declaration.id)?
            .variants
            .iter()
            .find(|variant| {
                variant.span.source == source_id
                    && variant.span.contains(offset)
                    && variant.name == token.text
            })
            .map(|variant| enum_variant_hover(graph, declaration, variant, range))
    })
}

fn struct_field_hover_at_token(
    graph: &vela_hir::module_graph::ModuleGraph,
    source_id: vela_common::SourceId,
    offset: u32,
    token: &HoverToken,
    range: DiagnosticRange,
) -> Option<Hover> {
    graph.declarations().find_map(|declaration| {
        if declaration.kind != DeclarationKind::Struct {
            return None;
        }
        graph
            .struct_shape(declaration.id)?
            .fields
            .iter()
            .find(|field| {
                field.span.source == source_id
                    && field.span.contains(offset)
                    && field.name == token.text
            })
            .map(|field| struct_field_hover(graph, declaration, field, range))
    })
}

fn script_member_hover(
    graph: &vela_hir::module_graph::ModuleGraph,
    receiver: &TypeFact,
    member: &str,
    range: DiagnosticRange,
) -> Option<Hover> {
    let owner_names = record_owner_names(receiver);
    graph.declarations().find_map(|declaration| {
        if declaration.kind != DeclarationKind::Struct
            || !owner_names
                .iter()
                .any(|owner| declaration_name_matches(graph, declaration, owner))
        {
            return None;
        }
        graph
            .struct_shape(declaration.id)?
            .fields
            .iter()
            .find(|field| field.name == member)
            .map(|field| struct_field_hover(graph, declaration, field, range))
    })
}

fn script_method_hover_at_token(
    graph: &vela_hir::module_graph::ModuleGraph,
    text: &str,
    source_id: vela_common::SourceId,
    token: &HoverToken,
    range: DiagnosticRange,
) -> Option<Hover> {
    graph
        .declarations()
        .find_map(|declaration| match declaration.kind {
            DeclarationKind::Impl if declaration.span.source == source_id => {
                let metadata = graph.impl_metadata(declaration.id)?;
                metadata
                    .methods
                    .iter()
                    .find(|method| {
                        method.name == token.text
                            && method_name_range_in_text(text, declaration.span, &method.name)
                                .is_some_and(|name_range| {
                                    name_range.start <= token.range.start
                                        && token.range.end <= name_range.end
                                })
                    })
                    .map(|method| impl_method_hover(graph, declaration, metadata, method, range))
            }
            DeclarationKind::Trait if declaration.span.source == source_id => {
                let shape = graph.trait_shape(declaration.id)?;
                shape
                    .methods
                    .iter()
                    .find(|method| {
                        method.name == token.text
                            && method_name_range_in_text(text, declaration.span, &method.name)
                                .is_some_and(|name_range| {
                                    name_range.start <= token.range.start
                                        && token.range.end <= name_range.end
                                })
                    })
                    .map(|method| trait_method_hover(graph, declaration, method, range))
            }
            DeclarationKind::Const
            | DeclarationKind::Global
            | DeclarationKind::Function
            | DeclarationKind::Struct
            | DeclarationKind::Enum
            | DeclarationKind::Trait
            | DeclarationKind::Impl => None,
        })
}

fn script_method_hover(
    graph: &vela_hir::module_graph::ModuleGraph,
    receiver: &TypeFact,
    method: &str,
    range: DiagnosticRange,
) -> Option<Hover> {
    let owner_names = record_owner_names(receiver);
    graph.declarations().find_map(|declaration| {
        if declaration.kind != DeclarationKind::Impl {
            return None;
        }
        let metadata = graph.impl_metadata(declaration.id)?;
        if !matches!(metadata.kind, ImplMetadataKind::Inherent)
            || !owner_names
                .iter()
                .any(|owner| impl_target_matches(&metadata.target_path, owner))
        {
            return None;
        }
        metadata
            .methods
            .iter()
            .find(|entry| entry.name == method)
            .map(|entry| impl_method_hover(graph, declaration, metadata, entry, range))
    })
}

fn script_trait_method_hover(
    graph: &vela_hir::module_graph::ModuleGraph,
    receiver: &TypeFact,
    method: &str,
    range: DiagnosticRange,
) -> Option<Hover> {
    let owner_names = trait_owner_names(receiver);
    graph.declarations().find_map(|declaration| {
        if declaration.kind != DeclarationKind::Trait
            || !owner_names
                .iter()
                .any(|owner| declaration_name_matches(graph, declaration, owner))
        {
            return None;
        }
        graph
            .trait_shape(declaration.id)?
            .methods
            .iter()
            .find(|entry| entry.name == method)
            .map(|entry| trait_method_hover(graph, declaration, entry, range))
    })
}

fn struct_field_hover(
    graph: &vela_hir::module_graph::ModuleGraph,
    declaration: &Declaration,
    field: &StructFieldHint,
    range: DiagnosticRange,
) -> Hover {
    let owner = qualified_declaration_label(graph, declaration);
    Hover {
        range,
        label: format!("{owner}.{}", field.name),
        kind: HoverKind::Field,
        detail: struct_field_detail(field),
        docs: attr_docs(&field.attrs),
    }
}

fn impl_method_hover(
    graph: &vela_hir::module_graph::ModuleGraph,
    declaration: &Declaration,
    metadata: &vela_hir::type_hint::ImplMetadata,
    method: &ImplMethodMetadata,
    range: DiagnosticRange,
) -> Hover {
    let owner = impl_owner_label(graph, declaration, metadata);
    Hover {
        range,
        label: format!("{owner}.{}", method.name),
        kind: HoverKind::Method,
        detail: signature_detail(&method.signature),
        docs: None,
    }
}

fn trait_method_hover(
    graph: &vela_hir::module_graph::ModuleGraph,
    declaration: &Declaration,
    method: &TraitMethodMetadata,
    range: DiagnosticRange,
) -> Hover {
    let owner = qualified_declaration_label(graph, declaration);
    Hover {
        range,
        label: format!("{owner}.{}", method.name),
        kind: HoverKind::Method,
        detail: signature_detail(&method.signature),
        docs: attr_docs(&method.attrs),
    }
}

fn signature_detail(signature: &FunctionSignature) -> String {
    let params = signature
        .params
        .iter()
        .map(|param| {
            param.type_hint.as_ref().map_or_else(
                || param.name.clone(),
                |hint| format!("{}: {}", param.name, hint.display()),
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    signature.return_type.as_ref().map_or_else(
        || format!("({params})"),
        |return_type| format!("({params}) -> {}", return_type.display()),
    )
}

fn struct_field_detail(field: &StructFieldHint) -> String {
    field
        .type_hint
        .as_ref()
        .map_or_else(|| TypeFact::Any.display_name(), |hint| hint.display())
}

fn enum_variant_hover_for_declaration(
    graph: &vela_hir::module_graph::ModuleGraph,
    declaration: &Declaration,
    variant_name: &str,
    range: DiagnosticRange,
) -> Option<Hover> {
    if declaration.kind != DeclarationKind::Enum {
        return None;
    }
    graph
        .enum_shape(declaration.id)?
        .variants
        .iter()
        .find(|variant| variant.name == variant_name)
        .map(|variant| enum_variant_hover(graph, declaration, variant, range))
}

fn enum_variant_hover(
    graph: &vela_hir::module_graph::ModuleGraph,
    declaration: &Declaration,
    variant: &EnumVariantHint,
    range: DiagnosticRange,
) -> Hover {
    let owner = qualified_declaration_label(graph, declaration);
    let label = format!("{owner}::{}", variant.name);
    Hover {
        range,
        label,
        kind: HoverKind::Variant,
        detail: enum_variant_detail(&owner, variant),
        docs: attr_docs(&variant.attrs),
    }
}

fn enum_variant_detail(owner: &str, variant: &EnumVariantHint) -> String {
    let fact = TypeFact::enum_type(owner, Some(&variant.name));
    match &variant.fields {
        EnumVariantFieldsHint::Unit => fact.display_name(),
        EnumVariantFieldsHint::Tuple(fields) => {
            let fields = fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}({fields})", fact.display_name())
        }
        EnumVariantFieldsHint::Record(fields) => {
            let fields = fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!("{} {{ {fields} }}", fact.display_name())
        }
    }
}

fn hover_from_local_declaration(
    bindings: &BindingMap,
    facts: &AnalysisFacts,
    token: &HoverToken,
    range: DiagnosticRange,
) -> Option<Hover> {
    bindings.locals().find_map(|binding| {
        let start = usize::try_from(binding.span.start).ok()?;
        let end = usize::try_from(binding.span.end).ok()?;
        (binding.name == token.text && start <= token.range.start && token.range.end <= end).then(
            || {
                local_hover(
                    binding,
                    local_fact(binding, facts).unwrap_or(TypeFact::Unknown),
                    range,
                )
            },
        )
    })
}

fn hover_from_declaration(
    graph: &vela_hir::module_graph::ModuleGraph,
    facts: &AnalysisFacts,
    declaration: &Declaration,
    range: DiagnosticRange,
) -> Hover {
    let fact = facts
        .declaration(declaration.id)
        .cloned()
        .unwrap_or(TypeFact::Unknown);
    let label = qualified_declaration_label(graph, declaration);
    let kind = match declaration.kind {
        DeclarationKind::Const => HoverKind::Const,
        DeclarationKind::Function => HoverKind::Function,
        DeclarationKind::Struct | DeclarationKind::Enum | DeclarationKind::Trait => HoverKind::Type,
        DeclarationKind::Global | DeclarationKind::Impl => HoverKind::Unknown,
    };
    Hover {
        range,
        label,
        kind,
        detail: fact.display_name(),
        docs: declaration_docs(graph, declaration),
    }
}

fn local_hover(binding: &LocalBinding, fact: TypeFact, range: DiagnosticRange) -> Hover {
    let kind = match binding.kind {
        LocalBindingKind::Parameter | LocalBindingKind::LambdaParameter => HoverKind::Parameter,
        LocalBindingKind::Let | LocalBindingKind::For | LocalBindingKind::Pattern => {
            HoverKind::Local
        }
    };
    Hover {
        range,
        label: binding.name.clone(),
        kind,
        detail: fact.display_name(),
        docs: None,
    }
}

fn local_fact(binding: &LocalBinding, facts: &AnalysisFacts) -> Option<TypeFact> {
    facts.local(binding.id).cloned()
}

fn type_hint_hover(schema: &RegistryFacts, name: &str, range: DiagnosticRange) -> Option<Hover> {
    starts_like_type_name(name).then(|| Hover {
        range,
        label: name.to_owned(),
        kind: HoverKind::Type,
        detail: schema
            .type_fact(name)
            .cloned()
            .unwrap_or(TypeFact::Any)
            .display_name(),
        docs: None,
    })
}

fn member_receiver_fact(
    databases: &LanguageServiceDatabases,
    document_id: &DocumentId,
    receiver: TextRange,
) -> Option<TypeFact> {
    let source = databases.source_db().records().get(document_id)?;
    let source_id = source.source_id();
    let start = u32::try_from(receiver.start).ok()?;
    let end = u32::try_from(receiver.end).ok()?;
    let receiver_span = Span::new(source_id, start, end);
    let graph = databases.hir_db().graph();
    let facts = AnalysisFacts::from_module_graph(graph);

    graph.declarations().find_map(|declaration| {
        if declaration.span.source != source_id || !declaration.span.contains(start) {
            return None;
        }
        let bindings = graph.bindings(declaration.id)?;
        let resolution = bindings.resolution_at_span(receiver_span)?;
        type_fact_for_resolution(resolution, bindings, &facts, databases.schema_db().facts())
    })
}

fn type_fact_for_resolution(
    resolution: &BindingResolution,
    bindings: &BindingMap,
    facts: &AnalysisFacts,
    schema: &RegistryFacts,
) -> Option<TypeFact> {
    match resolution {
        BindingResolution::Local(local) => {
            let binding = bindings.local(*local)?;
            facts
                .local(*local)
                .cloned()
                .filter(|fact| !matches!(fact, TypeFact::Unknown))
                .or_else(|| schema_fact_for_local_hint(binding, schema))
        }
        BindingResolution::Declaration(declaration) => facts.declaration(*declaration).cloned(),
        BindingResolution::Import(_) | BindingResolution::QualifiedPath(_) => None,
    }
}

fn schema_fact_for_local_hint(binding: &LocalBinding, schema: &RegistryFacts) -> Option<TypeFact> {
    let hint = binding.type_hint.as_ref()?;
    if hint.args.is_empty() {
        let qualified = hint.path.join("::");
        schema
            .type_fact(&qualified)
            .or_else(|| hint.path.last().and_then(|name| schema.type_fact(name)))
            .or_else(|| schema.trait_fact(&qualified))
            .or_else(|| hint.path.last().and_then(|name| schema.trait_fact(name)))
            .cloned()
    } else {
        None
    }
}

fn record_owner_names(fact: &TypeFact) -> Vec<String> {
    let mut names = Vec::new();
    collect_record_owner_names(fact, &mut names);
    names
}

fn collect_record_owner_names(fact: &TypeFact, names: &mut Vec<String>) {
    match fact {
        TypeFact::Record { name } => push_owner_names(names, name),
        TypeFact::Union(facts) => {
            for fact in facts {
                collect_record_owner_names(fact, names);
            }
        }
        TypeFact::Unknown
        | TypeFact::Never
        | TypeFact::Any
        | TypeFact::Primitive(_)
        | TypeFact::Range
        | TypeFact::Array { .. }
        | TypeFact::Map { .. }
        | TypeFact::Set { .. }
        | TypeFact::Iterator { .. }
        | TypeFact::Option { .. }
        | TypeFact::OptionSome { .. }
        | TypeFact::OptionNone
        | TypeFact::Result { .. }
        | TypeFact::ResultOk { .. }
        | TypeFact::ResultErr { .. }
        | TypeFact::Function { .. }
        | TypeFact::Enum { .. }
        | TypeFact::Host { .. }
        | TypeFact::Trait { .. }
        | TypeFact::Module { .. } => {}
    }
}

fn trait_owner_names(fact: &TypeFact) -> Vec<String> {
    let mut names = Vec::new();
    collect_trait_owner_names(fact, &mut names);
    names
}

fn collect_trait_owner_names(fact: &TypeFact, names: &mut Vec<String>) {
    match fact {
        TypeFact::Trait { name } => push_owner_names(names, name),
        TypeFact::Union(facts) => {
            for fact in facts {
                collect_trait_owner_names(fact, names);
            }
        }
        TypeFact::Unknown
        | TypeFact::Never
        | TypeFact::Any
        | TypeFact::Primitive(_)
        | TypeFact::Range
        | TypeFact::Array { .. }
        | TypeFact::Map { .. }
        | TypeFact::Set { .. }
        | TypeFact::Iterator { .. }
        | TypeFact::Option { .. }
        | TypeFact::OptionSome { .. }
        | TypeFact::OptionNone
        | TypeFact::Result { .. }
        | TypeFact::ResultOk { .. }
        | TypeFact::ResultErr { .. }
        | TypeFact::Function { .. }
        | TypeFact::Enum { .. }
        | TypeFact::Host { .. }
        | TypeFact::Record { .. }
        | TypeFact::Module { .. } => {}
    }
}

fn push_owner_names(names: &mut Vec<String>, name: &str) {
    if !names.iter().any(|owner| owner == name) {
        names.push(name.to_owned());
    }
    if let Some(short) = name.rsplit("::").next()
        && short != name
        && !names.iter().any(|owner| owner == short)
    {
        names.push(short.to_owned());
    }
}

fn declaration_name_matches(
    graph: &vela_hir::module_graph::ModuleGraph,
    declaration: &Declaration,
    owner: &str,
) -> bool {
    declaration.name == owner || qualified_declaration_label(graph, declaration) == owner
}

fn impl_owner_label(
    graph: &vela_hir::module_graph::ModuleGraph,
    declaration: &Declaration,
    metadata: &vela_hir::type_hint::ImplMetadata,
) -> String {
    match &metadata.kind {
        ImplMetadataKind::Inherent => metadata
            .target_path
            .last()
            .map(|target| qualified_module_member_label(graph, declaration, target))
            .unwrap_or_else(|| qualified_declaration_label(graph, declaration)),
        ImplMetadataKind::Trait { trait_path } => {
            let trait_name = trait_path.join("::");
            let target = metadata.target_path.join("::");
            format!("{trait_name} for {target}")
        }
    }
}

fn impl_target_matches(path: &[String], owner: &str) -> bool {
    path.last().is_some_and(|name| name == owner) || path.join("::") == owner
}

fn qualified_module_member_label(
    graph: &vela_hir::module_graph::ModuleGraph,
    declaration: &Declaration,
    member: &str,
) -> String {
    let Some(module_path) = graph.module_path(declaration.module) else {
        return member.to_owned();
    };
    let module = module_path.join();
    if module.is_empty() {
        member.to_owned()
    } else {
        format!("{module}::{member}")
    }
}

fn method_name_range_in_text(text: &str, span: Span, name: &str) -> Option<TextRange> {
    let range = span_text_range(span)?;
    let slice = text.get(range.start..range.end)?;
    slice.match_indices(name).find_map(|(offset, matched)| {
        let start = range.start + offset;
        let end = start + matched.len();
        (is_identifier_boundary(text, start, end) && preceded_by_fn_keyword(text, start))
            .then(|| TextRange::new(start, end))
    })
}

fn span_text_range(span: Span) -> Option<TextRange> {
    let start = usize::try_from(span.start).ok()?;
    let end = usize::try_from(span.end).ok()?;
    Some(TextRange::new(start, end))
}

fn is_identifier_boundary(text: &str, start: usize, end: usize) -> bool {
    let before = text
        .get(..start)
        .and_then(|prefix| prefix.chars().next_back());
    let after = text.get(end..).and_then(|suffix| suffix.chars().next());
    before.is_none_or(|ch| !is_identifier_continue(ch))
        && after.is_none_or(|ch| !is_identifier_continue(ch))
}

fn preceded_by_fn_keyword(text: &str, start: usize) -> bool {
    let Some(before_name) = text.get(..start).map(str::trim_end) else {
        return false;
    };
    let end = before_name.len();
    let word_start = before_name
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    if before_name.get(word_start..end) != Some("fn") {
        return false;
    }
    before_name
        .get(..word_start)
        .and_then(|prefix| prefix.chars().next_back())
        .is_none_or(|ch| !is_identifier_continue(ch))
}

fn stdlib_function_detail(function: &StdlibFunctionFact) -> String {
    TypeFact::function(function.params.clone(), function.returns.clone()).display_name()
}

fn stdlib_method_detail(method: &StdlibMethodFact) -> String {
    TypeFact::function(method.params.clone(), method.returns.clone()).display_name()
}

fn declaration_docs(
    graph: &vela_hir::module_graph::ModuleGraph,
    declaration: &Declaration,
) -> Option<String> {
    attr_docs(graph.declaration_attrs(declaration.id))
}

fn attr_docs(attrs: &[HirAttribute]) -> Option<String> {
    attrs
        .iter()
        .find(|attr| attr.name == "doc")
        .map(|attr| attr.string_value().to_owned())
}

fn qualified_declaration_label(
    graph: &vela_hir::module_graph::ModuleGraph,
    declaration: &Declaration,
) -> String {
    let Some(module_path) = graph.module_path(declaration.module) else {
        return declaration.name.clone();
    };
    let module = module_path.join();
    if module.is_empty() {
        declaration.name.clone()
    } else {
        format!("{module}::{}", declaration.name)
    }
}

fn hover_token_at(text: &str, position: Position) -> Option<HoverToken> {
    let offset = LineIndex::new(text).offset(position);
    let range = identifier_range_at(text, offset)?;
    let text_value = text[range.start..range.end].to_owned();
    let member_receiver = member_receiver_range(text, range.start);
    Some(HoverToken {
        text: text_value,
        range,
        member_receiver,
    })
}

fn identifier_range_at(text: &str, offset: usize) -> Option<TextRange> {
    let offset = offset.min(text.len());
    let start = text[..offset]
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    let end = text[offset..]
        .char_indices()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(offset + index))
        .unwrap_or(text.len());
    (start < end).then(|| TextRange::new(start, end))
}

fn member_receiver_range(text: &str, member_start: usize) -> Option<TextRange> {
    let before_member = text[..member_start].trim_end();
    let before_dot = before_member.strip_suffix('.')?.trim_end();
    let end = before_dot.len();
    let start = before_dot
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    (start < end).then(|| TextRange::new(start, end))
}

fn diagnostic_range(text: &str, range: TextRange) -> DiagnosticRange {
    let line_index = LineIndex::new(text);
    DiagnosticRange::new(
        line_index.position(range.start),
        line_index.position(range.end),
    )
}

fn starts_like_type_name(name: &str) -> bool {
    name.chars().next().is_some_and(char::is_uppercase)
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests;
