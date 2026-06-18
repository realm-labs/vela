use std::collections::BTreeMap;

use vela_analysis::type_fact::TypeFact;
use vela_common::{SourceId, Span};
use vela_hir::binding::{BindingMap, BindingResolution, LocalBinding};
use vela_hir::ids::{HirDeclId, HirLocalId};
use vela_hir::module_graph::{Declaration, DeclarationKind, Import, ImportResolution, ModuleGraph};
use vela_hir::type_hint::{EnumVariantFieldsHint, FunctionSignature, HirTypeHint};
use vela_syntax::ast::Visibility;
use vela_syntax::token::Keyword;

use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, LineIndex, Position, QueryContext,
    SymbolRef, TextRange,
    symbol_ref::{
        qualified_source_declaration_path, schema_member_symbol, schema_symbol,
        schema_variant_symbol, source_enum_variant_symbol, source_member_symbol,
        source_symbol_for_declaration,
    },
};

mod edit;
mod fields;
mod methods;
mod schema;
mod variants;

pub use edit::{
    DocumentTextEdit, EditPlan, PrepareRename, RenameRisk, RenameRiskKind, TextEdit, WorkspaceEdit,
};

#[derive(Debug, Clone, Eq, PartialEq)]
struct RenameToken {
    range: TextRange,
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn prepare_rename(
        &self,
        document_id: &DocumentId,
        position: Position,
    ) -> Option<PrepareRename> {
        let query = QueryContext::from_databases(self, document_id, position)?;
        let source_id = query.source_id()?;
        let member_receiver = query
            .member_receiver_range()
            .or_else(|| query.call_member_receiver_range());
        let member_receiver_fact =
            member_receiver.and_then(|receiver| query.type_fact_for_range(self, receiver));
        let target = rename_target(
            self,
            &query,
            source_id,
            query.text(),
            RenameToken {
                range: query.identifier_range()?,
            },
            query
                .member_receiver_text()
                .or_else(|| query.call_member_receiver_text()),
            member_receiver_fact.as_ref(),
        )?;
        let token_range = diagnostic_range(query.text(), target.token_range());
        Some(PrepareRename {
            document_id: document_id.clone(),
            range: token_range,
            placeholder: target.placeholder().to_owned(),
            symbol: target.symbol(self.hir_db().graph(), self)?,
        })
    }

    #[must_use]
    pub fn rename(
        &self,
        document_id: &DocumentId,
        position: Position,
        new_name: &str,
    ) -> Option<WorkspaceEdit> {
        if !is_valid_rename_identifier(new_name) {
            return None;
        }
        let query = QueryContext::from_databases(self, document_id, position)?;
        let source_id = query.source_id()?;
        let member_receiver = query
            .member_receiver_range()
            .or_else(|| query.call_member_receiver_range());
        let member_receiver_fact =
            member_receiver.and_then(|receiver| query.type_fact_for_range(self, receiver));
        let target = rename_target(
            self,
            &query,
            source_id,
            query.text(),
            RenameToken {
                range: query.identifier_range()?,
            },
            query
                .member_receiver_text()
                .or_else(|| query.call_member_receiver_text()),
            member_receiver_fact.as_ref(),
        )?;
        let symbol = target.symbol(self.hir_db().graph(), self)?;
        match target {
            RenameTarget::Local(target) => {
                self.rename_local(document_id, query.text(), target, new_name)
            }
            RenameTarget::Declaration(target) => self.rename_declaration(target, new_name),
            RenameTarget::ScriptField(target) => {
                fields::rename_script_field(self, target, new_name)
            }
            RenameTarget::ScriptMethod(target) => {
                methods::rename_script_method(self, target, new_name)
            }
            RenameTarget::SchemaMember(target) => {
                schema::rename_schema_member(self, target, new_name)
            }
            RenameTarget::SchemaType(target) => schema::rename_schema_type(self, target, new_name),
            RenameTarget::SchemaFunction(target) => {
                schema::rename_schema_function(self, target, new_name)
            }
            RenameTarget::SchemaVariant(target) => {
                schema::rename_schema_variant(self, target, new_name)
            }
            RenameTarget::EnumVariant(target) => {
                variants::rename_enum_variant(self, target, new_name)
            }
        }
        .map(|edit| edit.with_symbol(symbol))
    }

    fn rename_local(
        &self,
        document_id: &DocumentId,
        text: &str,
        target: LocalRenameTarget<'_>,
        new_name: &str,
    ) -> Option<WorkspaceEdit> {
        if local_name_conflicts(target.bindings, target.local, new_name) {
            return None;
        }

        let mut edits = Vec::new();
        if let Some(binding) = target.bindings.local(target.local)
            && let Some(range) = local_binding_name_range(text, binding)
        {
            edits.push(TextEdit {
                range: diagnostic_range(text, range),
                new_text: new_name.to_owned(),
            });
        }
        edits.extend(
            target
                .bindings
                .resolutions()
                .filter_map(|(expression, resolution)| match resolution {
                    BindingResolution::Local(local) if *local == target.local => {
                        let expression = target.bindings.expression(expression)?;
                        Some(TextEdit {
                            range: diagnostic_range(text, span_text_range(expression.span)?),
                            new_text: new_name.to_owned(),
                        })
                    }
                    BindingResolution::Local(_)
                    | BindingResolution::Declaration(_)
                    | BindingResolution::Import(_)
                    | BindingResolution::QualifiedPath(_) => None,
                }),
        );

        edits.sort_by_key(|edit| {
            let start = edit.range.start();
            (start.line, start.character)
        });

        WorkspaceEdit::checked(
            vec![document_text_edit_for_rename(
                self,
                document_id.clone(),
                edits,
            )],
            Vec::new(),
        )
    }

    fn rename_declaration(
        &self,
        target: DeclarationRenameTarget<'_>,
        new_name: &str,
    ) -> Option<WorkspaceEdit> {
        let graph = self.hir_db().graph();
        if declaration_name_conflicts(graph, target.declaration, new_name) {
            return None;
        }

        let mut edits_by_document = BTreeMap::<DocumentId, Vec<TextEdit>>::new();
        self.push_declaration_edit(target.declaration, new_name, &mut edits_by_document)?;
        self.push_import_edits(target.declaration, new_name, &mut edits_by_document);
        self.push_declaration_use_edits(target.declaration, new_name, &mut edits_by_document);
        self.push_type_hint_use_edits(target.declaration, new_name, &mut edits_by_document);

        workspace_edit_for_rename(
            self,
            edits_by_document,
            rename_risks_for_declaration(target.declaration),
        )
    }

    fn push_declaration_edit(
        &self,
        declaration: &Declaration,
        new_name: &str,
        edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
    ) -> Option<()> {
        let source = self.source_record_for_rename(declaration.span.source)?;
        let span_range = span_text_range(declaration.span)?;
        let range = name_range_in_text(source.text(), span_range, &declaration.name)?;
        edits_by_document
            .entry(source.document_id().clone())
            .or_default()
            .push(TextEdit {
                range: diagnostic_range(source.text(), range),
                new_text: new_name.to_owned(),
            });
        Some(())
    }

    fn push_import_edits(
        &self,
        declaration: &Declaration,
        new_name: &str,
        edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
    ) {
        let graph = self.hir_db().graph();
        for module in graph.module_ids() {
            let Some(imports) = graph.imports(module) else {
                continue;
            };
            for import in imports {
                let Some(ImportResolution::Declaration(resolved)) = import.resolution else {
                    continue;
                };
                if resolved != declaration.id {
                    continue;
                }
                let Some(source) = self.source_record_for_rename(import.span.source) else {
                    continue;
                };
                let Some(span_range) = span_text_range(import.span) else {
                    continue;
                };
                let Some(name) = import.path.last() else {
                    continue;
                };
                let Some(range) = name_range_in_text(source.text(), span_range, name) else {
                    continue;
                };
                edits_by_document
                    .entry(source.document_id().clone())
                    .or_default()
                    .push(TextEdit {
                        range: diagnostic_range(source.text(), range),
                        new_text: new_name.to_owned(),
                    });
            }
        }
    }

    fn push_declaration_use_edits(
        &self,
        declaration: &Declaration,
        new_name: &str,
        edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
    ) {
        let graph = self.hir_db().graph();
        for owner in graph.declarations() {
            let Some(bindings) = graph.bindings(owner.id) else {
                continue;
            };
            for (expression, resolution) in bindings.resolutions() {
                let BindingResolution::Declaration(resolved) = resolution else {
                    continue;
                };
                if *resolved != declaration.id {
                    continue;
                }
                let Some(expression) = bindings.expression(expression) else {
                    continue;
                };
                let Some(source) = self.source_record_for_rename(expression.span.source) else {
                    continue;
                };
                let Some(range) = span_text_range(expression.span) else {
                    continue;
                };
                if token_text(source.text(), range) != Some(declaration.name.as_str()) {
                    continue;
                }
                edits_by_document
                    .entry(source.document_id().clone())
                    .or_default()
                    .push(TextEdit {
                        range: diagnostic_range(source.text(), range),
                        new_text: new_name.to_owned(),
                    });
            }
        }
    }

    fn push_type_hint_use_edits(
        &self,
        declaration: &Declaration,
        new_name: &str,
        edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
    ) {
        if !is_type_declaration(declaration) {
            return;
        }

        let graph = self.hir_db().graph();
        for owner in graph.declarations() {
            for_each_type_hint_in_declaration(graph, owner, |hint| {
                push_matching_type_hint_edits(
                    self,
                    graph,
                    owner,
                    hint,
                    declaration,
                    new_name,
                    edits_by_document,
                );
            });
        }
    }

    fn source_record_for_rename(&self, source_id: SourceId) -> Option<&crate::SourceRecord> {
        self.source_db()
            .records()
            .values()
            .find(|record| record.source_id() == source_id)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum RenameTarget<'a> {
    Local(LocalRenameTarget<'a>),
    Declaration(DeclarationRenameTarget<'a>),
    ScriptField(fields::ScriptFieldRenameTarget),
    ScriptMethod(methods::ScriptMethodRenameTarget),
    SchemaMember(schema::SchemaMemberRenameTarget),
    SchemaType(schema::SchemaTypeRenameTarget),
    SchemaFunction(schema::SchemaFunctionRenameTarget),
    SchemaVariant(schema::SchemaVariantRenameTarget),
    EnumVariant(variants::EnumVariantRenameTarget),
}

impl RenameTarget<'_> {
    const fn token_range(&self) -> TextRange {
        match self {
            Self::Local(target) => target.token.range,
            Self::Declaration(target) => target.token.range,
            Self::ScriptField(target) => target.token.range,
            Self::ScriptMethod(target) => target.token.range,
            Self::SchemaMember(target) => target.token.range,
            Self::SchemaType(target) => target.token.range,
            Self::SchemaFunction(target) => target.token.range,
            Self::SchemaVariant(target) => target.token.range,
            Self::EnumVariant(target) => target.token.range,
        }
    }

    fn placeholder(&self) -> &str {
        match self {
            Self::Local(target) => &target.placeholder,
            Self::Declaration(target) => &target.declaration.name,
            Self::ScriptField(target) => &target.field,
            Self::ScriptMethod(target) => &target.method,
            Self::SchemaMember(target) => &target.member,
            Self::SchemaType(target) => &target.name,
            Self::SchemaFunction(target) => &target.name,
            Self::SchemaVariant(target) => &target.variant,
            Self::EnumVariant(target) => &target.variant,
        }
    }

    fn symbol(
        &self,
        graph: &ModuleGraph,
        databases: &LanguageServiceDatabases,
    ) -> Option<SymbolRef> {
        match self {
            Self::Local(target) => Some(local_symbol_for_target(databases, target)),
            Self::Declaration(target) => {
                Some(source_symbol_for_declaration(graph, target.declaration))
            }
            Self::ScriptField(target) => source_member_symbol(graph, target.owner, &target.field),
            Self::ScriptMethod(target) => source_member_symbol(graph, target.owner, &target.method),
            Self::EnumVariant(target) => {
                source_enum_variant_symbol(graph, target.owner, &target.variant)
            }
            Self::SchemaMember(target) => Some(schema_member_symbol(&target.owner, &target.member)),
            Self::SchemaType(target) => Some(schema_symbol(&target.name)),
            Self::SchemaFunction(target) => Some(schema_symbol(&target.name)),
            Self::SchemaVariant(target) => {
                Some(schema_variant_symbol(&target.owner, &target.variant))
            }
        }
    }
}

fn local_symbol_for_target(
    databases: &LanguageServiceDatabases,
    target: &LocalRenameTarget<'_>,
) -> SymbolRef {
    let Some(binding) = target.bindings.local(target.local) else {
        return SymbolRef::local(target.placeholder.clone());
    };
    let Some(source) = databases
        .source_db()
        .records()
        .values()
        .find(|source| source.source_id() == binding.span.source)
    else {
        return SymbolRef::local(binding.name.clone());
    };
    SymbolRef::local_from_span(
        binding.name.clone(),
        source.document_id().clone(),
        source.text(),
        binding.span,
    )
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct LocalRenameTarget<'a> {
    bindings: &'a BindingMap,
    local: HirLocalId,
    token: RenameToken,
    placeholder: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct DeclarationRenameTarget<'a> {
    declaration: &'a Declaration,
    token: RenameToken,
}

fn rename_target<'a>(
    databases: &'a LanguageServiceDatabases,
    query: &QueryContext<'_>,
    source_id: SourceId,
    text: &str,
    token: RenameToken,
    member_receiver_text: Option<&str>,
    member_receiver_fact: Option<&TypeFact>,
) -> Option<RenameTarget<'a>> {
    let graph = databases.hir_db().graph();
    let offset = u32::try_from(token.range.start).ok()?;
    let parsed_source = query
        .source_record()
        .and_then(|source| databases.parse_db().parsed_source(source.document_id()));

    if let Some(target) = fields::script_field_declaration_target(graph, source_id, text, &token) {
        return Some(RenameTarget::ScriptField(target));
    }
    if let Some(target) = methods::script_method_declaration_target(graph, source_id, text, &token)
    {
        return Some(RenameTarget::ScriptMethod(target));
    }
    if let Some(target) = variants::enum_variant_declaration_target(graph, source_id, text, &token)
    {
        return Some(RenameTarget::EnumVariant(target));
    }
    if let Some(target) = schema::schema_member_declaration_target(databases, source_id, &token) {
        return Some(RenameTarget::SchemaMember(target));
    }
    if let Some(target) = schema::schema_type_declaration_target(databases, source_id, &token) {
        return Some(RenameTarget::SchemaType(target));
    }
    if let Some(target) = schema::schema_function_declaration_target(databases, source_id, &token) {
        return Some(RenameTarget::SchemaFunction(target));
    }
    if let Some(target) = schema::schema_variant_declaration_target(databases, source_id, &token) {
        return Some(RenameTarget::SchemaVariant(target));
    }

    for declaration in graph.declarations() {
        if declaration.span.source != source_id || !declaration.span.contains(offset) {
            continue;
        }
        if token_text(text, token.range) == Some(declaration.name.as_str())
            && can_rename_declaration_target(declaration)
        {
            return Some(RenameTarget::Declaration(DeclarationRenameTarget {
                declaration,
                token,
            }));
        }
        let Some(bindings) = graph.bindings(declaration.id) else {
            continue;
        };
        if let Some(binding) = local_declaration_at_token(text, bindings, &token) {
            return Some(RenameTarget::Local(LocalRenameTarget {
                bindings,
                local: binding.id,
                token,
                placeholder: binding.name.clone(),
            }));
        }
        if let Some(local) = local_use_at_token(bindings, &token)
            && let Some(binding) = bindings.local(local)
        {
            return Some(RenameTarget::Local(LocalRenameTarget {
                bindings,
                local,
                token,
                placeholder: binding.name.clone(),
            }));
        }
        if let Some(target) =
            variants::enum_variant_use_target(graph, bindings, parsed_source, text, &token)
        {
            return Some(RenameTarget::EnumVariant(target));
        }
        if let Some(declaration_id) = declaration_use_at_token(bindings, &token)
            && let Some(target) = graph.declaration(declaration_id)
            && can_rename_declaration_target(target)
        {
            return Some(RenameTarget::Declaration(DeclarationRenameTarget {
                declaration: target,
                token,
            }));
        }
        if let Some(target) = type_hint_declaration_at_token(graph, declaration, text, &token)
            && can_rename_declaration_target(target)
        {
            return Some(RenameTarget::Declaration(DeclarationRenameTarget {
                declaration: target,
                token,
            }));
        }
        if let Some(target) = schema::schema_type_use_target(databases, declaration, text, &token) {
            return Some(RenameTarget::SchemaType(target));
        }
        if let Some(target) = schema::schema_function_use_target(databases, query, text, &token) {
            return Some(RenameTarget::SchemaFunction(target));
        }
        if let Some(target) = schema::schema_variant_use_target(databases, query, text, &token) {
            return Some(RenameTarget::SchemaVariant(target));
        }
        if let Some(target) = member_receiver_fact.and_then(|receiver| {
            token_text(text, token.range).and_then(|field| {
                fields::script_field_target_for_receiver_fact(graph, receiver, field, &token)
            })
        }) {
            return Some(RenameTarget::ScriptField(target));
        }
        if let Some(target) = token_text(text, token.range)
            .filter(|_| is_call_callee(text, token.range))
            .and_then(|method| {
                if member_receiver_text == Some("self") {
                    methods::script_method_target_for_self_receiver(
                        graph,
                        declaration.id,
                        method,
                        &token,
                    )
                } else {
                    member_receiver_fact.and_then(|receiver| {
                        methods::script_method_target_for_receiver_fact(
                            graph, receiver, method, &token,
                        )
                    })
                }
            })
        {
            return Some(RenameTarget::ScriptMethod(target));
        }
        if let Some(target) = member_receiver_fact.and_then(|receiver| {
            token_text(text, token.range).and_then(|member| {
                schema::schema_member_target_for_receiver_fact(
                    databases,
                    receiver,
                    member,
                    is_call_callee(text, token.range),
                    &token,
                )
            })
        }) {
            return Some(RenameTarget::SchemaMember(target));
        }
    }

    for module in graph.module_ids() {
        let Some(imports) = graph.imports(module) else {
            continue;
        };
        for import in imports {
            if import.span.source != source_id || !import.span.contains(offset) {
                continue;
            }
            let Some(ImportResolution::Declaration(declaration_id)) = import.resolution else {
                continue;
            };
            let Some(name) = import.path.last() else {
                continue;
            };
            if token_text(text, token.range) != Some(name.as_str()) {
                continue;
            }
            let Some(target) = graph.declaration(declaration_id) else {
                continue;
            };
            if !can_rename_declaration_target(target) {
                continue;
            }
            return Some(RenameTarget::Declaration(DeclarationRenameTarget {
                declaration: target,
                token,
            }));
        }
    }

    None
}

fn can_rename_declaration_target(declaration: &Declaration) -> bool {
    match declaration.kind {
        DeclarationKind::Function => true,
        DeclarationKind::Const | DeclarationKind::Global => {
            declaration.visibility != Visibility::Public
        }
        DeclarationKind::Struct | DeclarationKind::Enum | DeclarationKind::Trait => {
            declaration.visibility != Visibility::Public
        }
        DeclarationKind::Impl => false,
    }
}

fn is_type_declaration(declaration: &Declaration) -> bool {
    matches!(
        declaration.kind,
        DeclarationKind::Struct | DeclarationKind::Enum | DeclarationKind::Trait
    )
}

fn type_hint_declaration_at_token<'a>(
    graph: &'a ModuleGraph,
    owner: &Declaration,
    text: &str,
    token: &RenameToken,
) -> Option<&'a Declaration> {
    let mut target = None;
    for_each_type_hint_in_declaration(graph, owner, |hint| {
        if target.is_none() {
            target = type_hint_declaration_at_token_inner(graph, owner, text, hint, token);
        }
    });
    target
}

fn type_hint_declaration_at_token_inner<'a>(
    graph: &'a ModuleGraph,
    owner: &Declaration,
    text: &str,
    hint: &HirTypeHint,
    token: &RenameToken,
) -> Option<&'a Declaration> {
    if let Some(declaration) = type_hint_target_declaration(graph, owner, hint)
        && let Some(range) = type_hint_name_range(text, hint, &declaration.name)
        && range.start <= token.range.start
        && token.range.end <= range.end
    {
        return Some(declaration);
    }
    None
}

fn push_matching_type_hint_edits(
    databases: &LanguageServiceDatabases,
    graph: &ModuleGraph,
    owner: &Declaration,
    hint: &HirTypeHint,
    declaration: &Declaration,
    new_name: &str,
    edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
) {
    if type_hint_target_declaration(graph, owner, hint)
        .is_some_and(|target| target.id == declaration.id)
        && let Some(source) = databases.source_record_for_rename(hint.span.source)
        && let Some(range) = type_hint_name_range(source.text(), hint, &declaration.name)
    {
        edits_by_document
            .entry(source.document_id().clone())
            .or_default()
            .push(TextEdit {
                range: diagnostic_range(source.text(), range),
                new_text: new_name.to_owned(),
            });
    }
}

fn type_hint_target_declaration<'a>(
    graph: &'a ModuleGraph,
    owner: &Declaration,
    hint: &HirTypeHint,
) -> Option<&'a Declaration> {
    let name = hint.path.last()?;
    let declaration_id = if hint.path.len() == 1 {
        graph
            .module(owner.module)
            .and_then(|declarations| declarations.get(name))
            .or_else(|| imported_declaration_for_name(graph, owner, name))?
    } else {
        graph
            .declarations()
            .find(|declaration| qualified_source_declaration_path(graph, declaration) == hint.path)?
            .id
    };
    graph.declaration(declaration_id)
}

fn imported_declaration_for_name(
    graph: &ModuleGraph,
    owner: &Declaration,
    name: &str,
) -> Option<HirDeclId> {
    graph.imports(owner.module)?.iter().find_map(|import| {
        let binding_name = import.alias.as_ref().or_else(|| import.path.last())?;
        if binding_name != name {
            return None;
        }
        let ImportResolution::Declaration(declaration) = import.resolution?;
        Some(declaration)
    })
}

pub(super) fn for_each_type_hint_in_declaration(
    graph: &ModuleGraph,
    declaration: &Declaration,
    mut visit: impl FnMut(&HirTypeHint),
) {
    if let Some(metadata) = graph.const_metadata(declaration.id)
        && let Some(type_hint) = &metadata.type_hint
    {
        visit_type_hint_and_args(type_hint, &mut visit);
    }
    if let Some(metadata) = graph.global_metadata(declaration.id) {
        visit_type_hint_and_args(&metadata.type_hint, &mut visit);
    }
    if let Some(signature) = graph.function_signature(declaration.id) {
        visit_signature_type_hints(signature, &mut visit);
    }
    if let Some(shape) = graph.struct_shape(declaration.id) {
        for field in &shape.fields {
            if let Some(type_hint) = &field.type_hint {
                visit_type_hint_and_args(type_hint, &mut visit);
            }
        }
    }
    if let Some(shape) = graph.enum_shape(declaration.id) {
        for variant in &shape.variants {
            match &variant.fields {
                EnumVariantFieldsHint::Unit => {}
                EnumVariantFieldsHint::Tuple(params) => {
                    for param in params {
                        if let Some(type_hint) = &param.type_hint {
                            visit_type_hint_and_args(type_hint, &mut visit);
                        }
                    }
                }
                EnumVariantFieldsHint::Record(fields) => {
                    for field in fields {
                        if let Some(type_hint) = &field.type_hint {
                            visit_type_hint_and_args(type_hint, &mut visit);
                        }
                    }
                }
            }
        }
    }
    if let Some(shape) = graph.trait_shape(declaration.id) {
        for method in &shape.methods {
            visit_signature_type_hints(&method.signature, &mut visit);
            if let Some(node) = method.default_body_node
                && let Some(bindings) = graph.trait_default_method_bindings(node)
            {
                visit_binding_type_hints(bindings, &mut visit);
            }
        }
    }
    if let Some(metadata) = graph.impl_metadata(declaration.id) {
        for method in &metadata.methods {
            visit_signature_type_hints(&method.signature, &mut visit);
            if let Some(bindings) = graph.impl_method_bindings(method.node) {
                visit_binding_type_hints(bindings, &mut visit);
            }
        }
    }
    if let Some(bindings) = graph.bindings(declaration.id) {
        visit_binding_type_hints(bindings, &mut visit);
    }
}

fn visit_signature_type_hints(signature: &FunctionSignature, visit: &mut impl FnMut(&HirTypeHint)) {
    for param in &signature.params {
        if let Some(type_hint) = &param.type_hint {
            visit_type_hint_and_args(type_hint, visit);
        }
    }
    if let Some(type_hint) = &signature.return_type {
        visit_type_hint_and_args(type_hint, visit);
    }
}

fn visit_binding_type_hints(bindings: &BindingMap, visit: &mut impl FnMut(&HirTypeHint)) {
    for binding in bindings.locals() {
        if let Some(type_hint) = &binding.type_hint {
            visit_type_hint_and_args(type_hint, visit);
        }
    }
}

fn visit_type_hint_and_args(hint: &HirTypeHint, visit: &mut impl FnMut(&HirTypeHint)) {
    visit(hint);
    for arg in &hint.args {
        visit_type_hint_and_args(arg, visit);
    }
}

pub(super) fn type_hint_name_range(
    text: &str,
    hint: &HirTypeHint,
    name: &str,
) -> Option<TextRange> {
    let span_range = span_text_range(hint.span)?;
    last_name_range_in_text(text, span_range, name)
}

pub(super) fn document_text_edit_for_rename(
    databases: &LanguageServiceDatabases,
    document_id: DocumentId,
    edits: Vec<TextEdit>,
) -> DocumentTextEdit {
    let Some(source) = databases.source_db().records().get(&document_id) else {
        return DocumentTextEdit::new(document_id, edits);
    };
    DocumentTextEdit::new_versioned(document_id, source.version(), edits)
}

pub(super) fn workspace_edit_for_rename(
    databases: &LanguageServiceDatabases,
    edits_by_document: BTreeMap<DocumentId, Vec<TextEdit>>,
    risks: Vec<RenameRisk>,
) -> Option<WorkspaceEdit> {
    let document_edits = edits_by_document
        .into_iter()
        .map(|(document_id, mut edits)| {
            edits.sort_by_key(|edit| {
                let start = edit.range.start();
                (start.line, start.character)
            });
            edits.dedup();
            document_text_edit_for_rename(databases, document_id, edits)
        })
        .collect::<Vec<_>>();
    WorkspaceEdit::checked(document_edits, risks)
}

fn local_use_at_token(bindings: &BindingMap, token: &RenameToken) -> Option<HirLocalId> {
    let resolution = narrowest_resolution_at_token(bindings, token)?;
    match resolution {
        BindingResolution::Local(local) => Some(*local),
        BindingResolution::Declaration(_)
        | BindingResolution::Import(_)
        | BindingResolution::QualifiedPath(_) => None,
    }
}

fn declaration_use_at_token(bindings: &BindingMap, token: &RenameToken) -> Option<HirDeclId> {
    let resolution = narrowest_resolution_at_token(bindings, token)?;
    match resolution {
        BindingResolution::Declaration(declaration) => Some(*declaration),
        BindingResolution::Local(_)
        | BindingResolution::Import(_)
        | BindingResolution::QualifiedPath(_) => None,
    }
}

fn narrowest_resolution_at_token<'a>(
    bindings: &'a BindingMap,
    token: &RenameToken,
) -> Option<&'a BindingResolution> {
    bindings
        .resolutions()
        .filter_map(|(expression, resolution)| {
            let expression = bindings.expression(expression)?;
            let start = usize::try_from(expression.span.start).ok()?;
            let end = usize::try_from(expression.span.end).ok()?;
            (start <= token.range.start && token.range.end <= end)
                .then_some((end.saturating_sub(start), resolution))
        })
        .min_by_key(|(len, _)| *len)
        .map(|(_, resolution)| resolution)
}

fn local_declaration_at_token<'a>(
    text: &str,
    bindings: &'a BindingMap,
    token: &RenameToken,
) -> Option<&'a LocalBinding> {
    bindings.locals().find(|binding| {
        let Some(range) = local_binding_name_range(text, binding) else {
            return false;
        };
        range.start <= token.range.start && token.range.end <= range.end
    })
}

fn local_binding_name_range(text: &str, binding: &LocalBinding) -> Option<TextRange> {
    span_text_range(binding.span).and_then(|range| name_range_in_text(text, range, &binding.name))
}

fn local_name_conflicts(bindings: &BindingMap, local: HirLocalId, new_name: &str) -> bool {
    bindings
        .locals()
        .any(|binding| binding.id != local && binding.name == new_name)
}

fn declaration_name_conflicts(
    graph: &ModuleGraph,
    declaration: &Declaration,
    new_name: &str,
) -> bool {
    graph
        .module(declaration.module)
        .and_then(|declarations| declarations.get(new_name))
        .is_some_and(|existing| existing != declaration.id)
        || import_binding_name_conflicts(graph, declaration, new_name)
}

fn import_binding_name_conflicts(
    graph: &ModuleGraph,
    declaration: &Declaration,
    new_name: &str,
) -> bool {
    let target = ImportResolution::Declaration(declaration.id);
    for module in graph.module_ids() {
        let Some(imports) = graph.imports(module) else {
            continue;
        };
        if !imports
            .iter()
            .any(|import| import.resolution == Some(target) && import.alias.is_none())
        {
            continue;
        }
        if graph
            .module(module)
            .and_then(|declarations| declarations.get(new_name))
            .is_some_and(|existing| existing != declaration.id)
        {
            return true;
        }
        if imports.iter().any(|import| {
            if import.resolution == Some(target) && import.alias.is_none() {
                return false;
            }
            import_binding_name(import).is_some_and(|name| name == new_name)
        }) {
            return true;
        }
    }
    false
}

fn import_binding_name(import: &Import) -> Option<&str> {
    import
        .alias
        .as_deref()
        .or_else(|| import.path.last().map(String::as_str))
}

fn rename_risks_for_declaration(declaration: &Declaration) -> Vec<RenameRisk> {
    if declaration.visibility != Visibility::Public {
        return Vec::new();
    }

    vec![RenameRisk {
        kind: RenameRiskKind::HotReloadAbi,
        message: format!(
            "renaming public function `{}` can break hot-reload ABI compatibility and external callers",
            declaration.name
        ),
    }]
}

fn is_valid_rename_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(is_identifier_continue)
        && Keyword::from_text(name).is_none()
}

fn diagnostic_range(text: &str, range: TextRange) -> DiagnosticRange {
    let line_index = LineIndex::new(text);
    DiagnosticRange::new(
        line_index.position(range.start),
        line_index.position(range.end),
    )
}

fn span_text_range(span: Span) -> Option<TextRange> {
    let start = usize::try_from(span.start).ok()?;
    let end = usize::try_from(span.end).ok()?;
    Some(TextRange::new(start, end))
}

fn name_range_in_text(text: &str, range: TextRange, name: &str) -> Option<TextRange> {
    let slice = text.get(range.start..range.end)?;
    slice.match_indices(name).find_map(|(offset, matched)| {
        let start = range.start + offset;
        let end = start + matched.len();
        is_identifier_boundary(text, start, end).then(|| TextRange::new(start, end))
    })
}

fn last_name_range_in_text(text: &str, range: TextRange, name: &str) -> Option<TextRange> {
    let slice = text.get(range.start..range.end)?;
    slice.rmatch_indices(name).find_map(|(offset, matched)| {
        let start = range.start + offset;
        let end = start + matched.len();
        is_identifier_boundary(text, start, end).then(|| TextRange::new(start, end))
    })
}

fn is_identifier_boundary(text: &str, start: usize, end: usize) -> bool {
    let before = text[..start].chars().next_back();
    let after = text[end..].chars().next();
    before.is_none_or(|ch| !is_identifier_continue(ch))
        && after.is_none_or(|ch| !is_identifier_continue(ch))
}

fn is_call_callee(text: &str, range: TextRange) -> bool {
    text.get(range.end..)
        .is_some_and(|suffix| suffix.trim_start().starts_with('('))
}

fn token_text(text: &str, range: TextRange) -> Option<&str> {
    text.get(range.start..range.end)
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests;
