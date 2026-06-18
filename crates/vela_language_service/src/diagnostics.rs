use std::collections::BTreeSet;

use vela_common::{Diagnostic, Severity, SourceId, Span};
use vela_hir::{
    binding::{BindingMap, BindingResolution},
    ids::{HirDeclId, ModuleId},
    module_graph::{Declaration, Import, ImportResolution, ModuleGraph},
    type_hint::{EnumVariantFieldsHint, FunctionSignature, HirTypeHint},
};

use crate::{
    DisplayParts, DocumentId, LanguageServiceDatabases, LineIndex, Position, SymbolRef,
    WorkspaceGeneration,
    symbol_ref::{qualified_source_declaration_path, source_symbol_for_declaration},
};

pub(crate) const UNUSED_IMPORT_CODE: &str = "lsp::unused_import";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ServiceDiagnosticSeverity {
    Error,
    Warning,
    Note,
    Help,
}

impl From<Severity> for ServiceDiagnosticSeverity {
    fn from(value: Severity) -> Self {
        match value {
            Severity::Error => Self::Error,
            Severity::Warning => Self::Warning,
            Severity::Note => Self::Note,
            Severity::Help => Self::Help,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct DiagnosticRange {
    start: Position,
    end: Position,
}

impl DiagnosticRange {
    #[must_use]
    pub const fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    #[must_use]
    pub const fn start(self) -> Position {
        self.start
    }

    #[must_use]
    pub const fn end(self) -> Position {
        self.end
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DiagnosticLabel {
    document_id: DocumentId,
    range: DiagnosticRange,
    message: String,
    message_parts: DisplayParts,
}

impl DiagnosticLabel {
    #[must_use]
    pub fn document_id(&self) -> &DocumentId {
        &self.document_id
    }

    #[must_use]
    pub const fn range(&self) -> DiagnosticRange {
        self.range
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub fn message_parts(&self) -> &DisplayParts {
        &self.message_parts
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DiagnosticCandidate {
    replacement: String,
    replacement_parts: DisplayParts,
}

impl DiagnosticCandidate {
    #[must_use]
    pub fn replacement(&self) -> &str {
        &self.replacement
    }

    #[must_use]
    pub fn replacement_parts(&self) -> &DisplayParts {
        &self.replacement_parts
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DiagnosticRepairHint {
    document_id: DocumentId,
    range: DiagnosticRange,
    title: String,
    title_parts: DisplayParts,
    replacement: String,
    replacement_parts: DisplayParts,
}

impl DiagnosticRepairHint {
    #[must_use]
    pub fn document_id(&self) -> &DocumentId {
        &self.document_id
    }

    #[must_use]
    pub const fn range(&self) -> DiagnosticRange {
        self.range
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn title_parts(&self) -> &DisplayParts {
        &self.title_parts
    }

    #[must_use]
    pub fn replacement(&self) -> &str {
        &self.replacement
    }

    #[must_use]
    pub fn replacement_parts(&self) -> &DisplayParts {
        &self.replacement_parts
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ServiceDiagnostic {
    severity: ServiceDiagnosticSeverity,
    code: Option<String>,
    message: String,
    message_parts: DisplayParts,
    symbol: Option<SymbolRef>,
    range: Option<DiagnosticRange>,
    labels: Vec<DiagnosticLabel>,
    candidates: Vec<DiagnosticCandidate>,
    repair_hints: Vec<DiagnosticRepairHint>,
}

impl ServiceDiagnostic {
    #[must_use]
    pub const fn severity(&self) -> ServiceDiagnosticSeverity {
        self.severity
    }

    #[must_use]
    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub fn message_parts(&self) -> &DisplayParts {
        &self.message_parts
    }

    #[must_use]
    pub fn symbol(&self) -> Option<&SymbolRef> {
        self.symbol.as_ref()
    }

    #[must_use]
    pub const fn range(&self) -> Option<DiagnosticRange> {
        self.range
    }

    #[must_use]
    pub fn labels(&self) -> &[DiagnosticLabel] {
        &self.labels
    }

    #[must_use]
    pub fn candidates(&self) -> &[DiagnosticCandidate] {
        &self.candidates
    }

    #[must_use]
    pub fn repair_hints(&self) -> &[DiagnosticRepairHint] {
        &self.repair_hints
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DiagnosticStatus {
    Complete,
    Partial,
    Stale,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DocumentDiagnostics {
    document_id: DocumentId,
    generation: WorkspaceGeneration,
    status: DiagnosticStatus,
    diagnostics: Vec<ServiceDiagnostic>,
}

impl DocumentDiagnostics {
    #[must_use]
    pub fn document_id(&self) -> &DocumentId {
        &self.document_id
    }

    #[must_use]
    pub const fn generation(&self) -> WorkspaceGeneration {
        self.generation
    }

    #[must_use]
    pub const fn status(&self) -> DiagnosticStatus {
        self.status
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[ServiceDiagnostic] {
        &self.diagnostics
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct OpenDiagnosticsBatch {
    generation: WorkspaceGeneration,
    documents: Vec<DocumentDiagnostics>,
    pending_workspace_documents: Vec<DocumentId>,
}

impl OpenDiagnosticsBatch {
    #[must_use]
    pub const fn generation(&self) -> WorkspaceGeneration {
        self.generation
    }

    #[must_use]
    pub fn documents(&self) -> &[DocumentDiagnostics] {
        &self.documents
    }

    #[must_use]
    pub fn pending_workspace_documents(&self) -> &[DocumentId] {
        &self.pending_workspace_documents
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WorkspaceDiagnosticsBatch {
    generation: WorkspaceGeneration,
    documents: Vec<DocumentDiagnostics>,
}

impl WorkspaceDiagnosticsBatch {
    #[must_use]
    pub const fn generation(&self) -> WorkspaceGeneration {
        self.generation
    }

    #[must_use]
    pub fn documents(&self) -> &[DocumentDiagnostics] {
        &self.documents
    }
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn diagnostics_for_document(&self, document_id: &DocumentId) -> DocumentDiagnostics {
        self.diagnostics_for_document_at_generation(document_id, self.generation())
    }

    #[must_use]
    pub fn diagnostics_for_document_at_generation(
        &self,
        document_id: &DocumentId,
        generation: WorkspaceGeneration,
    ) -> DocumentDiagnostics {
        if generation != self.generation() {
            return DocumentDiagnostics {
                document_id: document_id.clone(),
                generation,
                status: DiagnosticStatus::Stale,
                diagnostics: Vec::new(),
            };
        }

        let status = self.diagnostic_status(document_id);
        let diagnostics = self.document_diagnostics(document_id);
        DocumentDiagnostics {
            document_id: document_id.clone(),
            generation,
            status,
            diagnostics,
        }
    }

    #[must_use]
    pub fn diagnostics_for_open_documents(
        &self,
        open_documents: &BTreeSet<DocumentId>,
    ) -> OpenDiagnosticsBatch {
        let documents = open_documents
            .iter()
            .filter(|document_id| self.source_db().records().contains_key(*document_id))
            .map(|document_id| self.diagnostics_for_document(document_id))
            .collect();
        let pending_workspace_documents = self.pending_workspace_diagnostics(open_documents);
        OpenDiagnosticsBatch {
            generation: self.generation(),
            documents,
            pending_workspace_documents,
        }
    }

    #[must_use]
    pub fn diagnostics_for_workspace_documents(
        &self,
        open_documents: &BTreeSet<DocumentId>,
    ) -> WorkspaceDiagnosticsBatch {
        self.diagnostics_for_workspace_documents_at_generation(open_documents, self.generation())
    }

    #[must_use]
    pub fn diagnostics_for_workspace_documents_at_generation(
        &self,
        open_documents: &BTreeSet<DocumentId>,
        generation: WorkspaceGeneration,
    ) -> WorkspaceDiagnosticsBatch {
        let documents = self
            .workspace_document_ids(open_documents)
            .into_iter()
            .map(|document_id| {
                self.diagnostics_for_document_at_generation(&document_id, generation)
            })
            .collect();
        WorkspaceDiagnosticsBatch {
            generation,
            documents,
        }
    }

    fn diagnostic_status(&self, document_id: &DocumentId) -> DiagnosticStatus {
        self.project_db()
            .module_by_document()
            .get(document_id)
            .filter(|module| self.analysis_db().invalidated_modules().contains(*module))
            .map_or(DiagnosticStatus::Complete, |_| DiagnosticStatus::Partial)
    }

    fn pending_workspace_diagnostics(
        &self,
        open_documents: &BTreeSet<DocumentId>,
    ) -> Vec<DocumentId> {
        self.workspace_document_ids(open_documents)
            .into_iter()
            .filter(|document_id| {
                self.project_db()
                    .module_by_document()
                    .get(document_id)
                    .is_some_and(|module| self.analysis_db().invalidated_modules().contains(module))
            })
            .collect()
    }

    fn workspace_document_ids(&self, open_documents: &BTreeSet<DocumentId>) -> Vec<DocumentId> {
        self.project_db()
            .module_by_document()
            .keys()
            .filter(|document_id| {
                !open_documents.contains(*document_id)
                    && self.source_db().records().contains_key(*document_id)
            })
            .cloned()
            .collect()
    }

    fn document_diagnostics(&self, document_id: &DocumentId) -> Vec<ServiceDiagnostic> {
        let mut diagnostics = self
            .parse_db()
            .parse_diagnostics(document_id)
            .unwrap_or_default()
            .iter()
            .map(|diagnostic| self.convert_diagnostic(diagnostic))
            .collect::<Vec<_>>();

        if let Some(source) = self.parse_db().source_id(document_id) {
            diagnostics.extend(
                self.hir_db()
                    .graph()
                    .diagnostics()
                    .iter()
                    .filter(|diagnostic| is_hir_diagnostic(diagnostic))
                    .filter(|diagnostic| diagnostic_mentions_source(diagnostic, source))
                    .map(|diagnostic| self.convert_diagnostic(diagnostic)),
            );
        }
        diagnostics.extend(self.unused_import_diagnostics(document_id));

        if let Some(parsed) = self.parse_db().parsed_source(document_id) {
            let graph = self.hir_db().graph();
            let source_diagnostics = self
                .project_db()
                .module_by_document()
                .get(document_id)
                .and_then(|module_path| graph.module_id(module_path))
                .map_or_else(
                    || {
                        vela_analysis::diagnostics::source_diagnostics(
                            parsed,
                            self.schema_db().facts(),
                        )
                    },
                    |module| {
                        vela_analysis::diagnostics::source_diagnostics_in_module(
                            parsed,
                            graph,
                            module,
                            self.schema_db().facts(),
                        )
                    },
                );
            diagnostics.extend(
                source_diagnostics
                    .iter()
                    .map(|diagnostic| self.convert_diagnostic(diagnostic)),
            );
        }

        diagnostics.extend(self.schema_db().diagnostics().iter().map(schema_diagnostic));

        diagnostics
    }

    fn unused_import_diagnostics(&self, document_id: &DocumentId) -> Vec<ServiceDiagnostic> {
        let graph = self.hir_db().graph();
        let Some(module_path) = self.project_db().module_by_document().get(document_id) else {
            return Vec::new();
        };
        let Some(module) = graph.module_id(module_path) else {
            return Vec::new();
        };
        let Some(imports) = graph.imports(module) else {
            return Vec::new();
        };

        let used_declarations = used_declarations_in_module(graph, module);
        imports
            .iter()
            .filter_map(|import| {
                let ImportResolution::Declaration(declaration) = import.resolution?;
                if used_declarations.contains(&declaration) {
                    return None;
                }
                let binding_name = import_binding_name(import)?;
                let symbol = graph
                    .declaration(declaration)
                    .map(|declaration| source_symbol_for_declaration(graph, declaration));
                let diagnostic = Diagnostic::warning(format!("unused import `{binding_name}`"))
                    .with_code(UNUSED_IMPORT_CODE)
                    .with_span(import.span)
                    .with_label(import.span, "import is never used");
                let mut diagnostic = self.convert_diagnostic(&diagnostic);
                diagnostic.symbol = symbol;
                Some(diagnostic)
            })
            .collect()
    }

    fn convert_diagnostic(&self, diagnostic: &Diagnostic) -> ServiceDiagnostic {
        ServiceDiagnostic {
            severity: diagnostic.severity.into(),
            code: diagnostic.code.clone(),
            message: diagnostic.message.clone(),
            message_parts: DisplayParts::plain(diagnostic.message.clone()),
            symbol: None,
            range: diagnostic.span.and_then(|span| self.range_for_span(span)),
            labels: diagnostic
                .labels
                .iter()
                .filter_map(|label| {
                    let (document_id, range) = self.document_range_for_span(label.span)?;
                    Some(DiagnosticLabel {
                        document_id,
                        range,
                        message: label.message.clone(),
                        message_parts: DisplayParts::plain(label.message.clone()),
                    })
                })
                .collect(),
            candidates: diagnostic
                .candidates
                .iter()
                .map(|candidate| DiagnosticCandidate {
                    replacement: candidate.replacement.clone(),
                    replacement_parts: DisplayParts::plain(candidate.replacement.clone()),
                })
                .collect(),
            repair_hints: diagnostic
                .repairs
                .iter()
                .filter_map(|repair| {
                    let (document_id, range) = self.document_range_for_span(repair.span)?;
                    Some(DiagnosticRepairHint {
                        document_id,
                        range,
                        title: repair.title.clone(),
                        title_parts: DisplayParts::plain(repair.title.clone()),
                        replacement: repair.replacement.clone(),
                        replacement_parts: DisplayParts::plain(repair.replacement.clone()),
                    })
                })
                .collect(),
        }
    }

    fn range_for_span(&self, span: Span) -> Option<DiagnosticRange> {
        self.document_range_for_span(span).map(|(_, range)| range)
    }

    fn document_range_for_span(&self, span: Span) -> Option<(DocumentId, DiagnosticRange)> {
        let (document_id, text) = self.text_for_source(span.source)?;
        let line_index = LineIndex::new(text);
        let start = line_index.position(span.start as usize);
        let end = line_index.position(span.end as usize);
        Some((document_id, DiagnosticRange::new(start, end)))
    }

    fn text_for_source(&self, source: SourceId) -> Option<(DocumentId, &str)> {
        self.source_db()
            .records()
            .values()
            .find(|record| record.source_id() == source)
            .map(|record| (record.document_id().clone(), record.text()))
    }
}

fn schema_diagnostic(diagnostic: &crate::SchemaDiagnostic) -> ServiceDiagnostic {
    let message = diagnostic.message().to_owned();
    ServiceDiagnostic {
        severity: ServiceDiagnosticSeverity::Warning,
        code: Some("schema::unavailable".to_owned()),
        message: message.clone(),
        message_parts: DisplayParts::plain(message),
        symbol: None,
        range: None,
        labels: Vec::new(),
        candidates: Vec::new(),
        repair_hints: Vec::new(),
    }
}

fn used_declarations_in_module(graph: &ModuleGraph, module: ModuleId) -> BTreeSet<HirDeclId> {
    let mut used = BTreeSet::new();
    for declaration in graph
        .declarations()
        .filter(|declaration| declaration.module == module)
    {
        collect_body_declaration_uses(graph, declaration, &mut used);
        for_each_type_hint_in_declaration(graph, declaration, |hint| {
            if let Some(target) = type_hint_target_declaration(graph, declaration, hint) {
                used.insert(target.id);
            }
        });
    }
    used
}

fn collect_body_declaration_uses(
    graph: &ModuleGraph,
    declaration: &Declaration,
    used: &mut BTreeSet<HirDeclId>,
) {
    if let Some(bindings) = graph.bindings(declaration.id) {
        collect_binding_declaration_uses(bindings, used);
    }
    if let Some(shape) = graph.trait_shape(declaration.id) {
        for method in &shape.methods {
            if let Some(node) = method.default_body_node
                && let Some(bindings) = graph.trait_default_method_bindings(node)
            {
                collect_binding_declaration_uses(bindings, used);
            }
        }
    }
    if let Some(metadata) = graph.impl_metadata(declaration.id) {
        for method in &metadata.methods {
            if let Some(bindings) = graph.impl_method_bindings(method.node) {
                collect_binding_declaration_uses(bindings, used);
            }
        }
    }
}

fn collect_binding_declaration_uses(bindings: &BindingMap, used: &mut BTreeSet<HirDeclId>) {
    for (_, resolution) in bindings.resolutions() {
        if let BindingResolution::Declaration(declaration) = resolution {
            used.insert(*declaration);
        }
    }
    for (_, resolution) in bindings.pattern_resolutions() {
        if let BindingResolution::Declaration(declaration) = resolution {
            used.insert(*declaration);
        }
    }
}

fn for_each_type_hint_in_declaration(
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
            .or_else(|| imported_declaration_for_name(graph, owner.module, name))?
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
    module: ModuleId,
    name: &str,
) -> Option<HirDeclId> {
    graph.imports(module)?.iter().find_map(|import| {
        if import_binding_name(import)? != name {
            return None;
        }
        let ImportResolution::Declaration(declaration) = import.resolution?;
        Some(declaration)
    })
}

fn import_binding_name(import: &Import) -> Option<&str> {
    import
        .alias
        .as_deref()
        .or_else(|| import.path.last().map(String::as_str))
}

fn is_hir_diagnostic(diagnostic: &Diagnostic) -> bool {
    diagnostic
        .code
        .as_deref()
        .is_some_and(|code| code.starts_with("hir::"))
}

fn diagnostic_mentions_source(diagnostic: &Diagnostic, source: SourceId) -> bool {
    diagnostic.span.is_some_and(|span| span.source == source)
        || diagnostic
            .labels
            .iter()
            .any(|label| label.span.source == source)
}

#[cfg(test)]
mod tests;
