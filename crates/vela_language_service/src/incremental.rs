use std::collections::{BTreeMap, BTreeSet};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;

use vela_common::{Diagnostic, SourceId};
use vela_hir::module_graph::{ModuleGraph, ModulePath, ModuleSource, stable_source_hash};
use vela_syntax::ast::{
    EnumVariantFields, FunctionItem, ImplKind, ItemKind, Param, SourceFile, StructField, TraitItem,
    TraitMethod, TypeHint, Visibility,
};
use vela_syntax::parser::parse_source;

use crate::{DocumentId, ProjectSources, SourceVersion, WorkspaceGeneration};

#[derive(Debug, Clone)]
pub struct SourceRecord {
    document_id: DocumentId,
    source_id: SourceId,
    module_path: ModulePath,
    text: Arc<str>,
    version: SourceVersion,
    content_hash: u64,
}

impl SourceRecord {
    #[must_use]
    pub fn document_id(&self) -> &DocumentId {
        &self.document_id
    }

    #[must_use]
    pub const fn source_id(&self) -> SourceId {
        self.source_id
    }

    #[must_use]
    pub fn module_path(&self) -> &ModulePath {
        &self.module_path
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub const fn version(&self) -> SourceVersion {
        self.version
    }

    #[must_use]
    pub const fn content_hash(&self) -> u64 {
        self.content_hash
    }
}

#[derive(Debug, Clone, Default)]
pub struct SourceDb {
    records: BTreeMap<DocumentId, SourceRecord>,
}

impl SourceDb {
    #[must_use]
    pub fn records(&self) -> &BTreeMap<DocumentId, SourceRecord> {
        &self.records
    }

    fn replace(&mut self, records: BTreeMap<DocumentId, SourceRecord>) {
        self.records = records;
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ParseSummary {
    imports: BTreeSet<ModulePath>,
    declaration_fingerprint: u64,
    import_fingerprint: u64,
}

#[derive(Debug, Clone)]
struct ParseRecord {
    source: SourceId,
    module_path: ModulePath,
    content_hash: u64,
    parsed: SourceFile,
    summary: ParseSummary,
}

#[derive(Debug, Clone, Default)]
pub struct ParseDb {
    records: BTreeMap<DocumentId, ParseRecord>,
    parse_count: usize,
}

impl ParseDb {
    #[must_use]
    pub fn parse_count(&self) -> usize {
        self.parse_count
    }

    #[must_use]
    pub fn source_id(&self, document_id: &DocumentId) -> Option<SourceId> {
        self.records.get(document_id).map(|record| record.source)
    }

    #[must_use]
    pub fn parse_diagnostics(&self, document_id: &DocumentId) -> Option<&[Diagnostic]> {
        self.records
            .get(document_id)
            .map(|record| record.parsed.diagnostics.as_slice())
    }

    #[must_use]
    pub fn parsed_source(&self, document_id: &DocumentId) -> Option<&SourceFile> {
        self.records.get(document_id).map(|record| &record.parsed)
    }

    #[must_use]
    pub fn parsed_document_count(&self) -> usize {
        self.records.len()
    }

    fn update_from_sources(&mut self, sources: &BTreeMap<DocumentId, SourceRecord>) -> ParseUpdate {
        let previous = self.records.clone();
        let source_ids = sources.keys().cloned().collect::<BTreeSet<_>>();
        self.records
            .retain(|document_id, _| source_ids.contains(document_id));

        let mut changed_modules = BTreeSet::new();
        let mut declaration_changed_modules = BTreeSet::new();
        let mut import_changed_modules = BTreeSet::new();
        let mut reparsed_documents = BTreeSet::new();

        for (document_id, source) in sources {
            let previous_record = previous.get(document_id);
            let record = if previous_record
                .is_some_and(|record| record.content_hash == source.content_hash)
            {
                previous_record.cloned().expect("checked previous record")
            } else {
                reparsed_documents.insert(document_id.clone());
                changed_modules.insert(source.module_path.clone());
                self.parse_count = self.parse_count.saturating_add(1);
                let parsed = parse_source(source.source_id, source.text());
                let summary = summarize_source(&parsed);
                ParseRecord {
                    source: source.source_id,
                    module_path: source.module_path.clone(),
                    content_hash: source.content_hash,
                    parsed,
                    summary,
                }
            };

            if let Some(previous_record) = previous_record {
                if previous_record.summary.declaration_fingerprint
                    != record.summary.declaration_fingerprint
                    || previous_record.module_path != record.module_path
                {
                    declaration_changed_modules.insert(record.module_path.clone());
                }
                if previous_record.summary.import_fingerprint != record.summary.import_fingerprint
                    || previous_record.module_path != record.module_path
                {
                    import_changed_modules.insert(record.module_path.clone());
                }
            } else {
                declaration_changed_modules.insert(record.module_path.clone());
                import_changed_modules.insert(record.module_path.clone());
            }

            self.records.insert(document_id.clone(), record);
        }

        for (document_id, previous_record) in previous {
            if !sources.contains_key(&document_id) {
                changed_modules.insert(previous_record.module_path.clone());
                declaration_changed_modules.insert(previous_record.module_path.clone());
                import_changed_modules.insert(previous_record.module_path);
            }
        }

        ParseUpdate {
            changed_modules,
            declaration_changed_modules,
            import_changed_modules,
            reparsed_documents,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct ParseUpdate {
    changed_modules: BTreeSet<ModulePath>,
    declaration_changed_modules: BTreeSet<ModulePath>,
    import_changed_modules: BTreeSet<ModulePath>,
    reparsed_documents: BTreeSet<DocumentId>,
}

#[derive(Debug, Clone, Default)]
pub struct ProjectDb {
    module_by_document: BTreeMap<DocumentId, ModulePath>,
    document_by_module: BTreeMap<ModulePath, DocumentId>,
    imports_by_module: BTreeMap<ModulePath, BTreeSet<ModulePath>>,
    reverse_dependencies: BTreeMap<ModulePath, BTreeSet<ModulePath>>,
}

impl ProjectDb {
    #[must_use]
    pub fn module_by_document(&self) -> &BTreeMap<DocumentId, ModulePath> {
        &self.module_by_document
    }

    #[must_use]
    pub fn reverse_dependencies(&self) -> &BTreeMap<ModulePath, BTreeSet<ModulePath>> {
        &self.reverse_dependencies
    }

    fn rebuild(&mut self, parse_db: &ParseDb) {
        self.module_by_document.clear();
        self.document_by_module.clear();
        self.imports_by_module.clear();
        self.reverse_dependencies.clear();

        for (document_id, record) in &parse_db.records {
            self.module_by_document
                .insert(document_id.clone(), record.module_path.clone());
            self.document_by_module
                .insert(record.module_path.clone(), document_id.clone());
            self.imports_by_module
                .insert(record.module_path.clone(), record.summary.imports.clone());
        }

        for (module, imports) in &self.imports_by_module {
            for imported in imports {
                self.reverse_dependencies
                    .entry(imported.clone())
                    .or_default()
                    .insert(module.clone());
            }
        }
    }

    fn transitive_dependents(&self, roots: &BTreeSet<ModulePath>) -> BTreeSet<ModulePath> {
        let mut impacted = roots.clone();
        let mut pending = roots.iter().cloned().collect::<Vec<_>>();
        while let Some(module) = pending.pop() {
            if let Some(dependents) = self.reverse_dependencies.get(&module) {
                for dependent in dependents {
                    if impacted.insert(dependent.clone()) {
                        pending.push(dependent.clone());
                    }
                }
            }
        }
        impacted
    }
}

#[derive(Debug, Clone, Default)]
pub struct HirDb {
    graph: ModuleGraph,
    rebuild_count: usize,
}

impl HirDb {
    #[must_use]
    pub const fn rebuild_count(&self) -> usize {
        self.rebuild_count
    }

    #[must_use]
    pub const fn graph(&self) -> &ModuleGraph {
        &self.graph
    }

    fn rebuild(&mut self, sources: &[ModuleSource]) {
        let mut graph = ModuleGraph::new();
        for source in sources {
            graph.add_source(source.clone());
        }
        graph.resolve_imports();
        self.graph = graph;
        self.rebuild_count = self.rebuild_count.saturating_add(1);
    }
}

#[derive(Debug, Clone, Default)]
pub struct AnalysisDb {
    invalidated_modules: BTreeSet<ModulePath>,
    generation: WorkspaceGeneration,
}

impl AnalysisDb {
    #[must_use]
    pub fn invalidated_modules(&self) -> &BTreeSet<ModulePath> {
        &self.invalidated_modules
    }

    #[must_use]
    pub const fn generation(&self) -> WorkspaceGeneration {
        self.generation
    }

    fn invalidate(&mut self, generation: WorkspaceGeneration, modules: BTreeSet<ModulePath>) {
        self.generation = generation;
        self.invalidated_modules = modules;
    }
}

#[derive(Debug, Clone)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

#[derive(Debug, Clone)]
pub struct CancellationHandle {
    cancelled: Arc<AtomicBool>,
}

impl CancellationHandle {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn token(&self) -> CancellationToken {
        CancellationToken {
            cancelled: Arc::clone(&self.cancelled),
        }
    }
}

fn cancellation_pair() -> (CancellationToken, CancellationHandle) {
    let cancelled = Arc::new(AtomicBool::new(false));
    (
        CancellationToken {
            cancelled: Arc::clone(&cancelled),
        },
        CancellationHandle { cancelled },
    )
}

#[derive(Debug, Clone)]
pub struct GenerationToken {
    generation: WorkspaceGeneration,
    cancellation: Option<CancellationToken>,
}

impl GenerationToken {
    #[must_use]
    pub const fn generation(&self) -> WorkspaceGeneration {
        self.generation
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancellation
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
    }
}

#[derive(Debug, Clone)]
pub struct BackgroundResult<T> {
    generation: WorkspaceGeneration,
    cancellation: Option<CancellationToken>,
    value: T,
}

impl<T> BackgroundResult<T> {
    #[must_use]
    pub fn new(token: GenerationToken, value: T) -> Self {
        Self {
            generation: token.generation,
            cancellation: token.cancellation,
            value,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum WorkPriority {
    Open,
    Workspace,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ScheduledModule {
    module: ModulePath,
    priority: WorkPriority,
}

impl ScheduledModule {
    #[must_use]
    pub fn new(module: ModulePath, priority: WorkPriority) -> Self {
        Self { module, priority }
    }

    #[must_use]
    pub fn module(&self) -> &ModulePath {
        &self.module
    }

    #[must_use]
    pub const fn priority(&self) -> WorkPriority {
        self.priority
    }
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct IndexingMetrics {
    source_count: usize,
    total_bytes: usize,
    total_lines: usize,
    parsed_document_count: usize,
    reparsed_document_count: usize,
    hir_rebuild_count: usize,
    elapsed_micros: u128,
}

impl IndexingMetrics {
    #[must_use]
    pub const fn source_count(self) -> usize {
        self.source_count
    }

    #[must_use]
    pub const fn total_bytes(self) -> usize {
        self.total_bytes
    }

    #[must_use]
    pub const fn total_lines(self) -> usize {
        self.total_lines
    }

    #[must_use]
    pub const fn parsed_document_count(self) -> usize {
        self.parsed_document_count
    }

    #[must_use]
    pub const fn reparsed_document_count(self) -> usize {
        self.reparsed_document_count
    }

    #[must_use]
    pub const fn hir_rebuild_count(self) -> usize {
        self.hir_rebuild_count
    }

    #[must_use]
    pub const fn elapsed_micros(self) -> u128 {
        self.elapsed_micros
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct InvalidationReport {
    generation: WorkspaceGeneration,
    reparsed_documents: BTreeSet<DocumentId>,
    changed_modules: BTreeSet<ModulePath>,
    declaration_changed_modules: BTreeSet<ModulePath>,
    import_changed_modules: BTreeSet<ModulePath>,
    hir_invalidated_modules: BTreeSet<ModulePath>,
    analysis_invalidated_modules: BTreeSet<ModulePath>,
    scheduled_modules: Vec<ScheduledModule>,
    metrics: IndexingMetrics,
}

impl InvalidationReport {
    #[must_use]
    pub const fn generation(&self) -> WorkspaceGeneration {
        self.generation
    }

    #[must_use]
    pub fn reparsed_documents(&self) -> &BTreeSet<DocumentId> {
        &self.reparsed_documents
    }

    #[must_use]
    pub fn changed_modules(&self) -> &BTreeSet<ModulePath> {
        &self.changed_modules
    }

    #[must_use]
    pub fn declaration_changed_modules(&self) -> &BTreeSet<ModulePath> {
        &self.declaration_changed_modules
    }

    #[must_use]
    pub fn import_changed_modules(&self) -> &BTreeSet<ModulePath> {
        &self.import_changed_modules
    }

    #[must_use]
    pub fn hir_invalidated_modules(&self) -> &BTreeSet<ModulePath> {
        &self.hir_invalidated_modules
    }

    #[must_use]
    pub fn analysis_invalidated_modules(&self) -> &BTreeSet<ModulePath> {
        &self.analysis_invalidated_modules
    }

    #[must_use]
    pub fn scheduled_modules(&self) -> &[ScheduledModule] {
        &self.scheduled_modules
    }

    #[must_use]
    pub const fn metrics(&self) -> IndexingMetrics {
        self.metrics
    }
}

#[derive(Debug, Clone, Default)]
pub struct LanguageServiceDatabases {
    source_db: SourceDb,
    project_db: ProjectDb,
    parse_db: ParseDb,
    hir_db: HirDb,
    analysis_db: AnalysisDb,
    generation: WorkspaceGeneration,
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn generation(&self) -> WorkspaceGeneration {
        self.generation
    }

    #[must_use]
    pub const fn source_db(&self) -> &SourceDb {
        &self.source_db
    }

    #[must_use]
    pub const fn project_db(&self) -> &ProjectDb {
        &self.project_db
    }

    #[must_use]
    pub const fn parse_db(&self) -> &ParseDb {
        &self.parse_db
    }

    #[must_use]
    pub const fn hir_db(&self) -> &HirDb {
        &self.hir_db
    }

    #[must_use]
    pub const fn analysis_db(&self) -> &AnalysisDb {
        &self.analysis_db
    }

    #[must_use]
    pub const fn begin_background_request(&self) -> GenerationToken {
        GenerationToken {
            generation: self.generation,
            cancellation: None,
        }
    }

    #[must_use]
    pub fn begin_cancellable_background_request(&self) -> (GenerationToken, CancellationHandle) {
        let (cancellation, handle) = cancellation_pair();
        (
            GenerationToken {
                generation: self.generation,
                cancellation: Some(cancellation),
            },
            handle,
        )
    }

    pub fn accept_background_result<T>(&self, result: BackgroundResult<T>) -> Option<T> {
        (result.generation == self.generation
            && !result
                .cancellation
                .as_ref()
                .is_some_and(CancellationToken::is_cancelled))
        .then_some(result.value)
    }

    pub fn update(&mut self, project: &ProjectSources) -> InvalidationReport {
        self.update_with_open_documents(project, &BTreeSet::new())
    }

    pub fn update_with_open_documents(
        &mut self,
        project: &ProjectSources,
        open_documents: &BTreeSet<DocumentId>,
    ) -> InvalidationReport {
        let started = Instant::now();
        self.generation = WorkspaceGeneration::new(self.generation.get().saturating_add(1));
        let sources = source_records(project);
        self.source_db.replace(sources);
        let previous_parse_count = self.parse_db.parse_count();
        let previous_hir_rebuild_count = self.hir_db.rebuild_count();
        let parse_update = self.parse_db.update_from_sources(self.source_db.records());
        self.project_db.rebuild(&self.parse_db);

        let dependency_roots = parse_update
            .declaration_changed_modules
            .union(&parse_update.import_changed_modules)
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut hir_invalidated_modules = parse_update.changed_modules.clone();
        hir_invalidated_modules.extend(self.project_db.transitive_dependents(&dependency_roots));
        let analysis_invalidated_modules = hir_invalidated_modules.clone();
        self.analysis_db
            .invalidate(self.generation, analysis_invalidated_modules.clone());

        if !dependency_roots.is_empty() {
            self.hir_db.rebuild(project.sources());
        }
        let scheduled_modules = schedule_modules(&hir_invalidated_modules, project, open_documents);
        let metrics = indexing_metrics(
            self.source_db.records(),
            self.parse_db.parsed_document_count(),
            self.parse_db
                .parse_count()
                .saturating_sub(previous_parse_count),
            self.hir_db
                .rebuild_count()
                .saturating_sub(previous_hir_rebuild_count),
            started.elapsed().as_micros(),
        );

        InvalidationReport {
            generation: self.generation,
            reparsed_documents: parse_update.reparsed_documents,
            changed_modules: parse_update.changed_modules,
            declaration_changed_modules: parse_update.declaration_changed_modules,
            import_changed_modules: parse_update.import_changed_modules,
            hir_invalidated_modules,
            analysis_invalidated_modules,
            scheduled_modules,
            metrics,
        }
    }
}

fn schedule_modules(
    invalidated_modules: &BTreeSet<ModulePath>,
    project: &ProjectSources,
    open_documents: &BTreeSet<DocumentId>,
) -> Vec<ScheduledModule> {
    let open_modules = open_documents
        .iter()
        .filter_map(|document| project.document_modules().get(document))
        .cloned()
        .collect::<BTreeSet<_>>();
    invalidated_modules
        .iter()
        .filter(|module| open_modules.contains(*module))
        .cloned()
        .map(|module| ScheduledModule::new(module, WorkPriority::Open))
        .chain(
            invalidated_modules
                .iter()
                .filter(|module| !open_modules.contains(*module))
                .cloned()
                .map(|module| ScheduledModule::new(module, WorkPriority::Workspace)),
        )
        .collect()
}

fn indexing_metrics(
    sources: &BTreeMap<DocumentId, SourceRecord>,
    parsed_document_count: usize,
    reparsed_document_count: usize,
    hir_rebuild_count: usize,
    elapsed_micros: u128,
) -> IndexingMetrics {
    let total_bytes = sources.values().map(|source| source.text.len()).sum();
    let total_lines = sources
        .values()
        .map(|source| source.text.lines().count().max(1))
        .sum();
    IndexingMetrics {
        source_count: sources.len(),
        total_bytes,
        total_lines,
        parsed_document_count,
        reparsed_document_count,
        hir_rebuild_count,
        elapsed_micros,
    }
}

fn source_records(project: &ProjectSources) -> BTreeMap<DocumentId, SourceRecord> {
    let source_by_module = project
        .sources()
        .iter()
        .map(|source| (source.path.clone(), source))
        .collect::<BTreeMap<_, _>>();
    project
        .document_modules()
        .iter()
        .filter_map(|(document_id, module_path)| {
            let source = source_by_module.get(module_path)?;
            let text = Arc::<str>::from(source.text.as_str());
            Some((
                document_id.clone(),
                SourceRecord {
                    document_id: document_id.clone(),
                    source_id: source.id,
                    module_path: module_path.clone(),
                    content_hash: stable_source_hash(&text),
                    version: SourceVersion::INITIAL,
                    text,
                },
            ))
        })
        .collect()
}

fn summarize_source(parsed: &SourceFile) -> ParseSummary {
    let mut declarations = Vec::new();
    let mut imports = BTreeSet::new();
    let mut import_fingerprint_parts = Vec::new();

    for item in &parsed.items {
        match &item.kind {
            ItemKind::Use(use_item) => {
                if let Some((_, module_segments)) = use_item.path.split_last() {
                    imports.insert(ModulePath::new(module_segments.iter().cloned()));
                }
                import_fingerprint_parts.push(format!(
                    "use:{} as {}",
                    use_item.path.join("::"),
                    use_item.alias.as_deref().unwrap_or("")
                ));
            }
            ItemKind::Const(inner) => declarations.push(format!(
                "{} const {}:{}",
                visibility(&item.visibility),
                inner.name,
                optional_hint(&inner.type_hint)
            )),
            ItemKind::Global(inner) => declarations.push(format!(
                "{} global {}:{}",
                visibility(&item.visibility),
                inner.name,
                hint_signature(&inner.type_hint)
            )),
            ItemKind::Function(function) => declarations.push(format!(
                "{} {}",
                visibility(&item.visibility),
                function_signature(function)
            )),
            ItemKind::Struct(inner) => declarations.push(format!(
                "{} struct {} {}",
                visibility(&item.visibility),
                inner.name,
                fields_signature(&inner.fields)
            )),
            ItemKind::Enum(inner) => declarations.push(format!(
                "{} enum {} {}",
                visibility(&item.visibility),
                inner.name,
                inner
                    .variants
                    .iter()
                    .map(|variant| {
                        let fields = match &variant.fields {
                            EnumVariantFields::Unit => String::new(),
                            EnumVariantFields::Tuple(params) => params_signature(params),
                            EnumVariantFields::Record(fields) => fields_signature(fields),
                        };
                        format!("{}({fields})", variant.name)
                    })
                    .collect::<Vec<_>>()
                    .join("|")
            )),
            ItemKind::Trait(inner) => declarations.push(format!(
                "{} {}",
                visibility(&item.visibility),
                trait_signature(inner)
            )),
            ItemKind::Impl(inner) => declarations.push(format!(
                "{} {}",
                visibility(&item.visibility),
                impl_signature(inner)
            )),
        }
    }
    declarations.sort();
    import_fingerprint_parts.sort();
    ParseSummary {
        imports,
        declaration_fingerprint: stable_source_hash(&declarations.join("\n")),
        import_fingerprint: stable_source_hash(&import_fingerprint_parts.join("\n")),
    }
}

fn visibility(visibility: &Visibility) -> &'static str {
    match visibility {
        Visibility::Private => "private",
        Visibility::Public => "public",
    }
}

fn function_signature(function: &FunctionItem) -> String {
    format!(
        "fn {}({}) -> {}",
        function.name,
        params_signature(&function.params),
        optional_hint(&function.return_type)
    )
}

fn trait_signature(item: &TraitItem) -> String {
    format!(
        "trait {} {}",
        item.name,
        item.methods
            .iter()
            .map(trait_method_signature)
            .collect::<Vec<_>>()
            .join("|")
    )
}

fn trait_method_signature(method: &TraitMethod) -> String {
    format!(
        "{}({}) -> {} default:{}",
        method.name,
        params_signature(&method.params),
        optional_hint(&method.return_type),
        method.has_default
    )
}

fn impl_signature(item: &vela_syntax::ast::ImplItem) -> String {
    let owner = item.target_path.join("::");
    let kind = match &item.kind {
        ImplKind::Inherent => "impl".to_owned(),
        ImplKind::Trait { trait_path } => format!("impl {}", trait_path.join("::")),
    };
    format!(
        "{kind} for {owner} {}",
        item.methods
            .iter()
            .map(|method| function_signature(&method.function))
            .collect::<Vec<_>>()
            .join("|")
    )
}

fn params_signature(params: &[Param]) -> String {
    params
        .iter()
        .map(|param| {
            format!(
                "{}:{}={}",
                param.name,
                optional_hint(&param.type_hint),
                param.default_value.is_some()
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn fields_signature(fields: &[StructField]) -> String {
    fields
        .iter()
        .map(|field| {
            format!(
                "{}:{}={}",
                field.name,
                optional_hint(&field.type_hint),
                field.default_value.is_some()
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn optional_hint(hint: &Option<TypeHint>) -> String {
    hint.as_ref().map_or_else(String::new, hint_signature)
}

fn hint_signature(hint: &TypeHint) -> String {
    if hint.args.is_empty() {
        hint.path.join("::")
    } else {
        format!(
            "{}<{}>",
            hint.path.join("::"),
            hint.args
                .iter()
                .map(hint_signature)
                .collect::<Vec<_>>()
                .join(",")
        )
    }
}

#[cfg(test)]
mod tests;
