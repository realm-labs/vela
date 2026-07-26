use std::collections::{BTreeMap, BTreeSet};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;

use vela_analysis::facts::AnalysisFacts;
use vela_analysis::registry::RegistryFacts;
use vela_common::{Diagnostic, SourceId};
use vela_hir::module_graph::{ModuleGraph, ModuleSource, stable_source_hash};
use vela_package::{ModuleKey, ModulePath, PackageAlias, PackageId};
use vela_syntax::Parse as SyntaxParse;
use vela_syntax::ast::SyntaxSourceFile;
use vela_syntax::parse::parse_source_with_id;

use crate::analysis_cache::AnalysisFactsCache;
use crate::{
    CompletionResolvePayload, DocumentId, ProjectSources, SchemaArtifact, SchemaSourceLocations,
    SourceVersion, SymbolRef, WorkspaceGeneration,
};

mod parse_summary;

use parse_summary::{ParseSummary, summarize_source};

#[derive(Debug, Clone)]
pub struct SourceRecord {
    document_id: DocumentId,
    source_id: SourceId,
    module_key: ModuleKey,
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
    pub fn module_key(&self) -> &ModuleKey {
        &self.module_key
    }

    #[must_use]
    pub fn module_path(&self) -> &ModulePath {
        &self.module_key.path
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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ModuleFingerprint {
    declaration: u64,
    import: u64,
}

impl ModuleFingerprint {
    #[must_use]
    pub const fn declaration(self) -> u64 {
        self.declaration
    }

    #[must_use]
    pub const fn import(self) -> u64 {
        self.import
    }
}

#[derive(Debug, Clone)]
struct ParseRecord {
    source: SourceId,
    module_key: ModuleKey,
    content_hash: u64,
    syntax: SyntaxParse<SyntaxSourceFile>,
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
            .map(|record| record.syntax.diagnostics())
    }

    #[must_use]
    pub fn syntax_parse(&self, document_id: &DocumentId) -> Option<&SyntaxParse<SyntaxSourceFile>> {
        self.records.get(document_id).map(|record| &record.syntax)
    }

    #[must_use]
    pub fn parsed_document_count(&self) -> usize {
        self.records.len()
    }

    /// Indexes the trees this database already holds so HIR lowering can reuse
    /// them instead of parsing every workspace source again.
    fn syntax_by_source(&self) -> BTreeMap<SourceId, &SyntaxParse<SyntaxSourceFile>> {
        self.records
            .values()
            .map(|record| (record.source, &record.syntax))
            .collect()
    }

    #[must_use]
    pub fn module_fingerprint(&self, module_key: &ModuleKey) -> Option<ModuleFingerprint> {
        self.records
            .values()
            .find(|record| &record.module_key == module_key)
            .map(|record| ModuleFingerprint {
                declaration: record.summary.declaration_fingerprint,
                import: record.summary.import_fingerprint,
            })
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
            let record = if let Some(previous_record) = previous_record
                && previous_record.content_hash == source.content_hash
                && previous_record.source == source.source_id
            {
                let mut record = previous_record.clone();
                record.module_key.clone_from(&source.module_key);
                record
            } else {
                reparsed_documents.insert(document_id.clone());
                changed_modules.insert(source.module_key.clone());
                self.parse_count = self.parse_count.saturating_add(1);
                let syntax = parse_source_with_id(source.source_id, source.text());
                let summary = summarize_source(&syntax);
                ParseRecord {
                    source: source.source_id,
                    module_key: source.module_key.clone(),
                    content_hash: source.content_hash,
                    syntax,
                    summary,
                }
            };

            if let Some(previous_record) = previous_record {
                let module_key_changed = previous_record.module_key != record.module_key;
                if module_key_changed {
                    changed_modules.insert(previous_record.module_key.clone());
                    changed_modules.insert(record.module_key.clone());
                }
                if previous_record.summary.declaration_fingerprint
                    != record.summary.declaration_fingerprint
                    || module_key_changed
                {
                    if module_key_changed {
                        declaration_changed_modules.insert(previous_record.module_key.clone());
                    }
                    declaration_changed_modules.insert(record.module_key.clone());
                }
                if previous_record.summary.import_fingerprint != record.summary.import_fingerprint
                    || module_key_changed
                {
                    if module_key_changed {
                        import_changed_modules.insert(previous_record.module_key.clone());
                    }
                    import_changed_modules.insert(record.module_key.clone());
                }
            } else {
                declaration_changed_modules.insert(record.module_key.clone());
                import_changed_modules.insert(record.module_key.clone());
            }

            self.records.insert(document_id.clone(), record);
        }

        for (document_id, previous_record) in previous {
            if !sources.contains_key(&document_id) {
                changed_modules.insert(previous_record.module_key.clone());
                declaration_changed_modules.insert(previous_record.module_key.clone());
                import_changed_modules.insert(previous_record.module_key);
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
    changed_modules: BTreeSet<ModuleKey>,
    declaration_changed_modules: BTreeSet<ModuleKey>,
    import_changed_modules: BTreeSet<ModuleKey>,
    reparsed_documents: BTreeSet<DocumentId>,
}

#[derive(Debug, Clone, Default)]
pub struct ProjectDb {
    module_by_document: BTreeMap<DocumentId, ModuleKey>,
    document_by_module: BTreeMap<ModuleKey, DocumentId>,
    imports_by_module: BTreeMap<ModuleKey, BTreeSet<ModuleKey>>,
    reverse_dependencies: BTreeMap<ModuleKey, BTreeSet<ModuleKey>>,
    rebuild_count: usize,
}

impl ProjectDb {
    #[must_use]
    pub fn module_by_document(&self) -> &BTreeMap<DocumentId, ModuleKey> {
        &self.module_by_document
    }

    #[must_use]
    pub fn reverse_dependencies(&self) -> &BTreeMap<ModuleKey, BTreeSet<ModuleKey>> {
        &self.reverse_dependencies
    }

    #[must_use]
    pub const fn rebuild_count(&self) -> usize {
        self.rebuild_count
    }

    fn rebuild(
        &mut self,
        parse_db: &ParseDb,
        dependencies: &BTreeMap<PackageId, BTreeMap<PackageAlias, PackageId>>,
    ) {
        self.module_by_document.clear();
        self.document_by_module.clear();
        self.imports_by_module.clear();
        self.reverse_dependencies.clear();
        self.rebuild_count = self.rebuild_count.saturating_add(1);

        for (document_id, record) in &parse_db.records {
            self.module_by_document
                .insert(document_id.clone(), record.module_key.clone());
            self.document_by_module
                .insert(record.module_key.clone(), document_id.clone());
            self.imports_by_module.insert(
                record.module_key.clone(),
                record
                    .summary
                    .imports
                    .iter()
                    .map(|path| resolve_import_module(&record.module_key, path, dependencies))
                    .collect(),
            );
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

    fn transitive_dependents(&self, roots: &BTreeSet<ModuleKey>) -> BTreeSet<ModuleKey> {
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

    fn rebuild(
        &mut self,
        sources: &[ModuleSource],
        dependencies: BTreeMap<PackageId, BTreeMap<PackageAlias, PackageId>>,
        parses: &BTreeMap<SourceId, &SyntaxParse<SyntaxSourceFile>>,
    ) {
        let mut graph = ModuleGraph::with_package_dependencies(dependencies);
        for source in sources {
            match parses.get(&source.id) {
                Some(parsed) => graph.add_parsed_source(source.clone(), parsed),
                None => graph.add_source(source.clone()),
            };
        }
        graph.resolve_imports();
        self.graph = graph;
        self.rebuild_count = self.rebuild_count.saturating_add(1);
    }
}

#[derive(Debug, Clone, Default)]
pub struct AnalysisDb {
    invalidated_modules: BTreeSet<ModuleKey>,
    generation: WorkspaceGeneration,
}

impl AnalysisDb {
    #[must_use]
    pub fn invalidated_modules(&self) -> &BTreeSet<ModuleKey> {
        &self.invalidated_modules
    }

    #[must_use]
    pub const fn generation(&self) -> WorkspaceGeneration {
        self.generation
    }

    fn invalidate(&mut self, generation: WorkspaceGeneration, modules: BTreeSet<ModuleKey>) {
        self.generation = generation;
        self.invalidated_modules = modules;
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SchemaDiagnostic {
    message: String,
}

impl SchemaDiagnostic {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, Clone, Default)]
pub struct SchemaDb {
    facts: RegistryFacts,
    service_set: Option<crate::SchemaServiceSetFact>,
    source_locations: SchemaSourceLocations,
    diagnostics: Vec<SchemaDiagnostic>,
}

impl SchemaDb {
    #[must_use]
    pub const fn facts(&self) -> &RegistryFacts {
        &self.facts
    }

    #[must_use]
    pub const fn source_locations(&self) -> &SchemaSourceLocations {
        &self.source_locations
    }

    #[must_use]
    pub const fn service_set(&self) -> Option<&crate::SchemaServiceSetFact> {
        self.service_set.as_ref()
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[SchemaDiagnostic] {
        &self.diagnostics
    }

    pub fn clear(&mut self) {
        self.facts = RegistryFacts::default();
        self.service_set = None;
        self.source_locations = SchemaSourceLocations::default();
        self.diagnostics.clear();
    }

    pub fn set_facts(&mut self, facts: RegistryFacts) {
        self.facts = facts;
        self.service_set = None;
        self.source_locations = SchemaSourceLocations::default();
        self.diagnostics.clear();
    }

    pub fn set_artifact(&mut self, artifact: SchemaArtifact) {
        self.source_locations = artifact.source_locations();
        self.service_set = artifact.service_set().cloned();
        self.facts = artifact.to_registry_facts();
        self.diagnostics.clear();
    }

    pub fn set_missing(&mut self, schema_path: impl Into<String>) {
        self.facts = RegistryFacts::default();
        self.service_set = None;
        self.source_locations = SchemaSourceLocations::default();
        self.diagnostics = vec![SchemaDiagnostic::new(format!(
            "host schema `{}` is unavailable; host facts degrade to Any",
            schema_path.into()
        ))];
    }

    pub fn set_invalid(&mut self, schema_path: impl Into<String>, message: impl Into<String>) {
        self.facts = RegistryFacts::default();
        self.service_set = None;
        self.source_locations = SchemaSourceLocations::default();
        self.diagnostics = vec![SchemaDiagnostic::new(format!(
            "host schema `{}` is invalid: {}; host facts degrade to Any",
            schema_path.into(),
            message.into()
        ))];
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
    module: ModuleKey,
    priority: WorkPriority,
}

impl ScheduledModule {
    #[must_use]
    pub fn new(module: ModuleKey, priority: WorkPriority) -> Self {
        Self { module, priority }
    }

    #[must_use]
    pub fn module(&self) -> &ModuleKey {
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
    project_rebuild_count: usize,
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
    pub const fn project_rebuild_count(self) -> usize {
        self.project_rebuild_count
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
    changed_modules: BTreeSet<ModuleKey>,
    declaration_changed_modules: BTreeSet<ModuleKey>,
    import_changed_modules: BTreeSet<ModuleKey>,
    hir_invalidated_modules: BTreeSet<ModuleKey>,
    analysis_invalidated_modules: BTreeSet<ModuleKey>,
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
    pub fn changed_modules(&self) -> &BTreeSet<ModuleKey> {
        &self.changed_modules
    }

    #[must_use]
    pub fn declaration_changed_modules(&self) -> &BTreeSet<ModuleKey> {
        &self.declaration_changed_modules
    }

    #[must_use]
    pub fn import_changed_modules(&self) -> &BTreeSet<ModuleKey> {
        &self.import_changed_modules
    }

    #[must_use]
    pub fn hir_invalidated_modules(&self) -> &BTreeSet<ModuleKey> {
        &self.hir_invalidated_modules
    }

    #[must_use]
    pub fn analysis_invalidated_modules(&self) -> &BTreeSet<ModuleKey> {
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
    schema_db: SchemaDb,
    analysis_db: AnalysisDb,
    analysis_cache: AnalysisFactsCache,
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
    pub const fn schema_db(&self) -> &SchemaDb {
        &self.schema_db
    }

    #[must_use]
    pub const fn analysis_db(&self) -> &AnalysisDb {
        &self.analysis_db
    }

    /// Whole-workspace analysis facts resolved without the host schema,
    /// memoized until the module graph changes.
    pub(crate) fn graph_analysis_facts(&self) -> &AnalysisFacts {
        self.analysis_cache.graph_only(self.hir_db.graph())
    }

    /// Whole-workspace analysis facts resolved against the host schema,
    /// memoized until the module graph or the schema changes.
    pub(crate) fn schema_analysis_facts(&self) -> &AnalysisFacts {
        self.analysis_cache
            .with_schema(self.hir_db.graph(), self.schema_db.facts())
    }

    /// Number of whole-workspace fact builds this database has paid for. Tests
    /// assert on it to prove requests reuse one build per generation.
    #[must_use]
    pub fn analysis_facts_build_count(&self) -> usize {
        self.analysis_cache.build_count()
    }

    pub fn set_schema_facts(&mut self, facts: RegistryFacts) {
        self.schema_db.set_facts(facts);
        self.analysis_cache.invalidate_schema();
    }

    pub fn load_schema_artifact_json(&mut self, schema_path: &str, source: &str) {
        match SchemaArtifact::from_json(source) {
            Ok(artifact) => self.schema_db.set_artifact(artifact),
            Err(error) => self.schema_db.set_invalid(schema_path, error.message()),
        }
        self.analysis_cache.invalidate_schema();
    }

    pub fn clear_schema(&mut self) {
        self.schema_db.clear();
        self.analysis_cache.invalidate_schema();
    }

    pub fn mark_schema_missing(&mut self, schema_path: impl Into<String>) {
        self.schema_db.set_missing(schema_path);
        self.analysis_cache.invalidate_schema();
    }

    #[must_use]
    pub fn completion_documentation(&self, payload: &CompletionResolvePayload) -> Option<String> {
        match payload {
            CompletionResolvePayload::Documentation {
                symbol: SymbolRef::Schema(name),
            } => schema_completion_documentation(self.schema_db.facts(), name).map(str::to_owned),
            CompletionResolvePayload::Documentation { .. } => None,
        }
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

    pub fn invalidate_project_config(&mut self) {
        self.generation = WorkspaceGeneration::new(self.generation.get().saturating_add(1));
        self.reset_project_config_caches();
        self.analysis_db
            .invalidate(self.generation, BTreeSet::new());
    }

    pub fn update_after_project_config_change_with_open_documents(
        &mut self,
        project: &ProjectSources,
        open_documents: &BTreeSet<DocumentId>,
    ) -> InvalidationReport {
        self.reset_project_config_caches();
        self.update_with_open_documents(project, open_documents)
    }

    fn reset_project_config_caches(&mut self) {
        self.source_db = SourceDb::default();
        self.project_db = ProjectDb::default();
        self.parse_db = ParseDb::default();
        self.hir_db = HirDb::default();
        self.analysis_cache.invalidate_graph();
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
        let previous_project_rebuild_count = self.project_db.rebuild_count();
        let previous_hir_rebuild_count = self.hir_db.rebuild_count();
        let parse_update = self.parse_db.update_from_sources(self.source_db.records());
        let project_index_invalidated = !parse_update.declaration_changed_modules.is_empty()
            || !parse_update.import_changed_modules.is_empty();
        if project_index_invalidated {
            self.project_db
                .rebuild(&self.parse_db, project.package_dependencies());
        }

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

        if !hir_invalidated_modules.is_empty() {
            self.hir_db.rebuild(
                project.sources(),
                project.package_dependencies().clone(),
                &self.parse_db.syntax_by_source(),
            );
            self.analysis_cache.invalidate_graph();
        }
        let scheduled_modules = schedule_modules(&hir_invalidated_modules, project, open_documents);
        let metrics = indexing_metrics(
            self.source_db.records(),
            self.parse_db.parsed_document_count(),
            self.parse_db
                .parse_count()
                .saturating_sub(previous_parse_count),
            self.project_db
                .rebuild_count()
                .saturating_sub(previous_project_rebuild_count),
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
    invalidated_modules: &BTreeSet<ModuleKey>,
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
    project_rebuild_count: usize,
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
        project_rebuild_count,
        hir_rebuild_count,
        elapsed_micros,
    }
}

fn schema_completion_documentation<'a>(schema: &'a RegistryFacts, name: &str) -> Option<&'a str> {
    if let Some((owner, variant)) = name.rsplit_once("::")
        && let Some(docs) = schema.variant_docs(owner, variant)
    {
        return Some(docs);
    }
    if let Some((owner, member)) = name.rsplit_once('.') {
        return schema
            .field_docs(owner, member)
            .or_else(|| schema.method_docs(owner, member))
            .or_else(|| schema.trait_method_docs(owner, member));
    }
    schema
        .type_docs(name)
        .or_else(|| schema.trait_docs(name))
        .or_else(|| schema.function_docs(name))
}

fn source_records(project: &ProjectSources) -> BTreeMap<DocumentId, SourceRecord> {
    let source_by_module = project
        .sources()
        .iter()
        .map(|source| {
            (
                ModuleKey::new(source.package.clone(), source.path.clone()),
                source,
            )
        })
        .collect::<BTreeMap<_, _>>();
    project
        .document_modules()
        .iter()
        .filter_map(|(document_id, module_key)| {
            let source = source_by_module.get(module_key)?;
            let text = Arc::<str>::from(source.text.as_str());
            Some((
                document_id.clone(),
                SourceRecord {
                    document_id: document_id.clone(),
                    source_id: source.id,
                    module_key: module_key.clone(),
                    content_hash: stable_source_hash(&text),
                    version: project
                        .document_versions()
                        .get(document_id)
                        .copied()
                        .unwrap_or(SourceVersion::INITIAL),
                    text,
                },
            ))
        })
        .collect()
}

fn resolve_import_module(
    current: &ModuleKey,
    path: &ModulePath,
    dependencies: &BTreeMap<PackageId, BTreeMap<PackageAlias, PackageId>>,
) -> ModuleKey {
    let Some((first, rest)) = path.segments().split_first() else {
        return ModuleKey::new(current.package.clone(), ModulePath::root());
    };
    if first == "crate" {
        return ModuleKey::new(
            current.package.clone(),
            ModulePath::new(rest.iter().cloned()),
        );
    }
    let dependency = PackageAlias::new(first)
        .ok()
        .and_then(|alias| dependencies.get(&current.package)?.get(&alias))
        .cloned();
    dependency.map_or_else(
        || ModuleKey::new(current.package.clone(), path.clone()),
        |package| ModuleKey::new(package, ModulePath::new(rest.iter().cloned())),
    )
}

#[cfg(test)]
mod tests;
