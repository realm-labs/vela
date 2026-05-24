use std::collections::{BTreeMap, btree_map::Entry};

use vela_common::{Diagnostic, SourceId, Span};
use vela_syntax::{FunctionItem, ItemKind, SourceFile, Visibility, parse_source};

use crate::binding::{BindingMap, FunctionBindingInput, bind_function};
use crate::{HirDeclId, HirNodeId, ModuleId};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ModulePath(Vec<String>);

impl ModulePath {
    #[must_use]
    pub fn new(segments: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self(segments.into_iter().map(Into::into).collect())
    }

    #[must_use]
    pub fn from_dotted(path: &str) -> Self {
        Self::new(path.split('.').filter(|segment| !segment.is_empty()))
    }

    #[must_use]
    pub fn segments(&self) -> &[String] {
        &self.0
    }

    #[must_use]
    pub fn join(&self) -> String {
        self.0.join(".")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleSource {
    pub id: SourceId,
    pub path: ModulePath,
    pub text: String,
}

impl ModuleSource {
    #[must_use]
    pub fn new(id: SourceId, path: ModulePath, text: impl Into<String>) -> Self {
        Self {
            id,
            path,
            text: text.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Declaration {
    pub id: HirDeclId,
    pub node: HirNodeId,
    pub module: ModuleId,
    pub name: String,
    pub kind: DeclarationKind,
    pub visibility: Visibility,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationKind {
    Function,
    Struct,
    Enum,
    Trait,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Import {
    pub module: ModuleId,
    pub path: Vec<String>,
    pub span: Span,
    pub resolution: Option<ImportResolution>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportResolution {
    Declaration(HirDeclId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedImport {
    pub path: Vec<String>,
    pub resolution: ImportResolution,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeclarationIndex {
    by_name: BTreeMap<String, HirDeclId>,
}

impl DeclarationIndex {
    #[must_use]
    pub fn get(&self, name: &str) -> Option<HirDeclId> {
        self.by_name.get(name).copied()
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.by_name.keys().map(String::as_str)
    }

    fn insert(&mut self, name: String, id: HirDeclId) -> Option<HirDeclId> {
        match self.by_name.entry(name) {
            Entry::Vacant(entry) => {
                entry.insert(id);
                None
            }
            Entry::Occupied(entry) => Some(*entry.get()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HirModule {
    id: ModuleId,
    path: ModulePath,
    source: SourceId,
    declarations: DeclarationIndex,
    imports: Vec<Import>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ModuleGraph {
    modules: Vec<HirModule>,
    module_by_path: BTreeMap<ModulePath, ModuleId>,
    declarations: BTreeMap<HirDeclId, Declaration>,
    bindings: BTreeMap<HirDeclId, BindingMap>,
    diagnostics: Vec<Diagnostic>,
    next_node_id: u32,
    next_decl_id: u32,
    next_expr_id: u32,
    next_local_id: u32,
}

impl ModuleGraph {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_source(&mut self, source: ModuleSource) -> ModuleId {
        let parsed = parse_source(source.id, &source.text);
        self.add_parsed_source(source.id, source.path, parsed)
    }

    pub fn add_parsed_source(
        &mut self,
        source: SourceId,
        path: ModulePath,
        parsed: SourceFile,
    ) -> ModuleId {
        let module = self.next_module_id();
        let module_span = self.module_span(source, &parsed);

        if let Some(existing) = self.module_by_path.get(&path).copied() {
            self.diagnostics.push(
                Diagnostic::error(format!("duplicate module `{}`", path.join()))
                    .with_code("hir::duplicate_module")
                    .with_label(
                        module_span,
                        format!("module `{}` is declared more than once", path.join()),
                    ),
            );
            self.diagnostics.extend(parsed.diagnostics);
            return existing;
        }
        self.module_by_path.insert(path.clone(), module);
        self.diagnostics.extend(parsed.diagnostics);

        let mut hir_module = HirModule {
            id: module,
            path,
            source,
            declarations: DeclarationIndex::default(),
            imports: Vec::new(),
        };

        let mut function_declarations = Vec::new();

        for item in &parsed.items {
            match &item.kind {
                ItemKind::Use(use_item) => {
                    hir_module.imports.push(Import {
                        module,
                        path: use_item.path.clone(),
                        span: item.span,
                        resolution: None,
                    });
                }
                ItemKind::Function(function) => {
                    let declaration = self.insert_declaration(
                        &mut hir_module,
                        function.name.clone(),
                        DeclarationKind::Function,
                        item.visibility.clone(),
                        item.span,
                    );
                    function_declarations.push((declaration, function.clone()));
                }
                ItemKind::Struct(record) => {
                    self.insert_declaration(
                        &mut hir_module,
                        record.name.clone(),
                        DeclarationKind::Struct,
                        item.visibility.clone(),
                        item.span,
                    );
                }
                ItemKind::Enum(enumeration) => {
                    self.insert_declaration(
                        &mut hir_module,
                        enumeration.name.clone(),
                        DeclarationKind::Enum,
                        item.visibility.clone(),
                        item.span,
                    );
                }
                ItemKind::Trait(trait_item) => {
                    self.insert_declaration(
                        &mut hir_module,
                        trait_item.name.clone(),
                        DeclarationKind::Trait,
                        item.visibility.clone(),
                        item.span,
                    );
                }
            }
        }

        for (declaration, function) in function_declarations {
            self.bind_function_body(&hir_module, declaration, &function);
        }

        self.modules.push(hir_module);
        module
    }

    pub fn resolve_imports(&mut self) {
        for module_index in 0..self.modules.len() {
            let import_count = self.modules[module_index].imports.len();
            for import_index in 0..import_count {
                let import_path = self.modules[module_index].imports[import_index]
                    .path
                    .clone();
                let span = self.modules[module_index].imports[import_index].span;
                let resolution = self.resolve_import_path(&import_path, span);
                self.modules[module_index].imports[import_index].resolution = resolution;
            }
        }
    }

    #[must_use]
    pub fn module(&self, module: ModuleId) -> Option<&DeclarationIndex> {
        self.modules
            .get(usize::try_from(module.get()).ok()?)
            .map(|module| &module.declarations)
    }

    #[must_use]
    pub fn module_path(&self, module: ModuleId) -> Option<&ModulePath> {
        self.modules
            .get(usize::try_from(module.get()).ok()?)
            .map(|module| &module.path)
    }

    #[must_use]
    pub fn declaration(&self, declaration: HirDeclId) -> Option<&Declaration> {
        self.declarations.get(&declaration)
    }

    #[must_use]
    pub fn bindings(&self, declaration: HirDeclId) -> Option<&BindingMap> {
        self.bindings.get(&declaration)
    }

    #[must_use]
    pub fn imports(&self, module: ModuleId) -> Option<&[Import]> {
        self.modules
            .get(usize::try_from(module.get()).ok()?)
            .map(|module| module.imports.as_slice())
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    fn insert_declaration(
        &mut self,
        module: &mut HirModule,
        name: String,
        kind: DeclarationKind,
        visibility: Visibility,
        span: Span,
    ) -> HirDeclId {
        let id = self.next_decl_id();
        let node = self.next_node_id();
        let declaration = Declaration {
            id,
            node,
            module: module.id,
            name: name.clone(),
            kind,
            visibility,
            span,
        };

        if let Some(previous_id) = module.declarations.insert(name.clone(), id)
            && let Some(previous) = self.declarations.get(&previous_id)
        {
            self.diagnostics.push(
                Diagnostic::error(format!("duplicate declaration `{name}`"))
                    .with_code("hir::duplicate_declaration")
                    .with_span(span)
                    .with_label(previous.span, "previous declaration is here")
                    .with_label(span, "duplicate declaration is here"),
            );
        }

        self.declarations.insert(id, declaration);
        id
    }

    fn bind_function_body(
        &mut self,
        module: &HirModule,
        declaration: HirDeclId,
        function: &FunctionItem,
    ) {
        let module_declarations = module
            .declarations
            .names()
            .filter_map(|name| {
                module
                    .declarations
                    .get(name)
                    .map(|declaration| (name.to_owned(), declaration))
            })
            .collect::<Vec<_>>();
        let imports = module
            .imports
            .iter()
            .filter_map(|import| import.path.last().cloned())
            .collect::<Vec<_>>();

        let (bindings, diagnostics) = bind_function(FunctionBindingInput {
            declaration,
            params: &function.params,
            body: &function.body,
            module_declarations,
            imports,
            next_expr_id: &mut self.next_expr_id,
            next_local_id: &mut self.next_local_id,
        });
        self.bindings.insert(declaration, bindings);
        self.diagnostics.extend(diagnostics);
    }

    fn resolve_import_path(&mut self, path: &[String], span: Span) -> Option<ImportResolution> {
        let Some((name, module_segments)) = path.split_last() else {
            self.diagnostics.push(
                Diagnostic::error("empty import path")
                    .with_code("hir::empty_import")
                    .with_span(span),
            );
            return None;
        };
        let module_path = ModulePath::new(module_segments.iter().cloned());
        let Some(module_id) = self.module_by_path.get(&module_path).copied() else {
            self.diagnostics.push(
                Diagnostic::error(format!("unresolved module `{}`", module_path.join()))
                    .with_code("hir::unresolved_module")
                    .with_span(span)
                    .with_label(span, self.module_candidate_label(&module_path)),
            );
            return None;
        };

        let declaration = self
            .module(module_id)
            .and_then(|declarations| declarations.get(name));
        match declaration {
            Some(declaration) => Some(ImportResolution::Declaration(declaration)),
            None => {
                self.diagnostics.push(
                    Diagnostic::error(format!(
                        "unresolved import `{}` in module `{}`",
                        name,
                        module_path.join()
                    ))
                    .with_code("hir::unresolved_import")
                    .with_span(span)
                    .with_label(span, self.declaration_candidate_label(module_id, name)),
                );
                None
            }
        }
    }

    fn declaration_candidate_label(&self, module: ModuleId, name: &str) -> String {
        let Some(declarations) = self.module(module) else {
            return "no declarations are available in this module".to_owned();
        };
        if let Some(candidate) = closest_name(name, declarations.names()) {
            format!("did you mean `{candidate}`?")
        } else {
            "no similar declarations found".to_owned()
        }
    }

    fn module_candidate_label(&self, path: &ModulePath) -> String {
        let wanted = path.join();
        let candidates = self
            .module_by_path
            .keys()
            .map(ModulePath::join)
            .collect::<Vec<_>>();
        if let Some(candidate) = closest_name(&wanted, candidates.iter().map(String::as_str)) {
            format!("did you mean module `{candidate}`?")
        } else {
            "no similar modules found".to_owned()
        }
    }

    fn next_module_id(&self) -> ModuleId {
        ModuleId::new(u32::try_from(self.modules.len()).unwrap_or(u32::MAX))
    }

    fn next_node_id(&mut self) -> HirNodeId {
        let id = HirNodeId::new(self.next_node_id);
        self.next_node_id = self.next_node_id.saturating_add(1);
        id
    }

    fn next_decl_id(&mut self) -> HirDeclId {
        let id = HirDeclId::new(self.next_decl_id);
        self.next_decl_id = self.next_decl_id.saturating_add(1);
        id
    }

    fn module_span(&self, source: SourceId, parsed: &SourceFile) -> Span {
        parsed
            .items
            .first()
            .map_or_else(|| Span::new(source, 0, 0), |item| item.span)
    }
}

fn closest_name(
    wanted: &str,
    candidates: impl IntoIterator<Item = impl AsRef<str>>,
) -> Option<String> {
    candidates
        .into_iter()
        .map(|candidate| candidate.as_ref().to_owned())
        .min_by_key(|candidate| candidate_distance(wanted, candidate))
        .filter(|candidate| candidate_distance(wanted, candidate) <= 3)
}

fn candidate_distance(wanted: &str, candidate: &str) -> usize {
    if wanted.contains(candidate) || candidate.contains(wanted) {
        return 0;
    }
    levenshtein(wanted, candidate)
}

fn levenshtein(lhs: &str, rhs: &str) -> usize {
    let mut previous = (0..=rhs.chars().count()).collect::<Vec<_>>();
    let mut current = vec![0; previous.len()];

    for (lhs_index, lhs_char) in lhs.chars().enumerate() {
        current[0] = lhs_index + 1;
        for (rhs_index, rhs_char) in rhs.chars().enumerate() {
            let cost = usize::from(lhs_char != rhs_char);
            current[rhs_index + 1] = (previous[rhs_index + 1] + 1)
                .min(current[rhs_index] + 1)
                .min(previous[rhs_index] + cost);
        }
        std::mem::swap(&mut previous, &mut current);
    }

    previous[rhs.chars().count()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BindingResolution, LocalBindingKind};

    fn source(id: u32, module: &str, text: &str) -> ModuleSource {
        ModuleSource::new(SourceId::new(id), ModulePath::from_dotted(module), text)
    }

    #[test]
    fn indexes_top_level_declarations_with_stable_ids() {
        let mut graph = ModuleGraph::new();
        let module = graph.add_source(source(
            1,
            "game.reward",
            r#"
pub fn grant(player) { return player; }
struct Reward { item_id, count }
enum QuestProgress { None, Active }
trait Damageable { fn damage(self, amount); }
"#,
        ));

        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
        let declarations = graph.module(module).expect("module declarations");
        let grant = declarations.get("grant").expect("grant declaration");
        let reward = declarations.get("Reward").expect("Reward declaration");

        assert_ne!(grant, reward);
        assert_eq!(grant.get(), 0);
        assert_eq!(reward.get(), 1);
        assert_eq!(
            graph.declaration(grant).map(|decl| decl.kind),
            Some(DeclarationKind::Function)
        );
        assert_eq!(
            graph.declaration(reward).map(|decl| decl.kind),
            Some(DeclarationKind::Struct)
        );
    }

    #[test]
    fn resolves_imports_across_modules() {
        let mut graph = ModuleGraph::new();
        let _reward = graph.add_source(source(1, "game.reward", "pub fn grant() { return 1; }"));
        let main = graph.add_source(source(
            2,
            "game.main",
            r#"
use game.reward.grant
fn main() { return grant(); }
"#,
        ));

        graph.resolve_imports();

        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
        let imports = graph.imports(main).expect("imports");
        let Some(ImportResolution::Declaration(declaration)) =
            imports.first().and_then(|import| import.resolution)
        else {
            panic!("expected resolved declaration import");
        };
        assert_eq!(
            graph
                .declaration(declaration)
                .map(|decl| decl.name.as_str()),
            Some("grant")
        );
    }

    #[test]
    fn duplicate_declarations_report_both_spans() {
        let mut graph = ModuleGraph::new();
        graph.add_source(source(
            1,
            "game.player",
            r#"
fn level() { return 1; }
struct level { value }
"#,
        ));

        let duplicate = graph
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code.as_deref() == Some("hir::duplicate_declaration"))
            .expect("duplicate declaration diagnostic");

        assert_eq!(duplicate.labels.len(), 2);
        assert!(duplicate.labels[0].message.contains("previous"));
        assert!(duplicate.labels[1].message.contains("duplicate"));
    }

    #[test]
    fn unresolved_imports_include_candidate_hints() {
        let mut graph = ModuleGraph::new();
        graph.add_source(source(1, "game.reward", "pub fn grant() { return 1; }"));
        graph.add_source(source(2, "game.main", "use game.reward.grant_reward"));

        graph.resolve_imports();

        let unresolved = graph
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code.as_deref() == Some("hir::unresolved_import"))
            .expect("unresolved import diagnostic");

        assert_eq!(unresolved.labels.len(), 1);
        assert!(unresolved.labels[0].message.contains("grant"));
    }

    #[test]
    fn function_bindings_resolve_params_and_locals_with_expression_ids() {
        let mut graph = ModuleGraph::new();
        let module = graph.add_source(source(
            1,
            "game.player",
            r#"
fn main(player) {
    let next = player.level;
    return next;
}
"#,
        ));
        let main = graph
            .module(module)
            .and_then(|module| module.get("main"))
            .expect("main declaration");

        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
        let bindings = graph.bindings(main).expect("main bindings");
        let [player] = bindings.locals_named("player") else {
            panic!("expected one player binding");
        };
        let [next] = bindings.locals_named("next") else {
            panic!("expected one next binding");
        };

        assert_eq!(
            bindings.local(*player).map(|local| local.kind),
            Some(LocalBindingKind::Parameter)
        );
        assert_eq!(
            bindings.local(*next).map(|local| local.kind),
            Some(LocalBindingKind::Let)
        );
        assert!(bindings.expression_count() >= 2);
        assert!(
            bindings
                .resolutions()
                .any(|(_, resolution)| resolution == &BindingResolution::Local(*player))
        );
        assert!(
            bindings
                .resolutions()
                .any(|(_, resolution)| resolution == &BindingResolution::Local(*next))
        );
    }

    #[test]
    fn binding_unresolved_names_report_candidate_hints() {
        let mut graph = ModuleGraph::new();
        graph.add_source(source(
            1,
            "game.player",
            r#"
fn main(player) {
    return plaeyr;
}
"#,
        ));

        let unresolved = graph
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code.as_deref() == Some("hir::unresolved_name"))
            .expect("unresolved name diagnostic");

        assert_eq!(unresolved.labels.len(), 1);
        assert!(unresolved.labels[0].message.contains("player"));
    }

    #[test]
    fn binding_tracks_nested_for_and_lambda_scopes() {
        let mut graph = ModuleGraph::new();
        let module = graph.add_source(source(
            1,
            "game.reward",
            r#"
fn main(rewards) {
    for reward in rewards {
        let mapper = |reward| reward.count;
    }
    return rewards;
}
"#,
        ));
        let main = graph
            .module(module)
            .and_then(|module| module.get("main"))
            .expect("main declaration");

        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
        let bindings = graph.bindings(main).expect("main bindings");
        let reward_bindings = bindings.locals_named("reward");

        assert_eq!(reward_bindings.len(), 2);
        assert_eq!(
            bindings.local(reward_bindings[0]).map(|local| local.kind),
            Some(LocalBindingKind::For)
        );
        assert_eq!(
            bindings.local(reward_bindings[1]).map(|local| local.kind),
            Some(LocalBindingKind::LambdaParameter)
        );
    }

    #[test]
    fn function_bindings_resolve_imported_names() {
        let mut graph = ModuleGraph::new();
        graph.add_source(source(1, "game.reward", "pub fn grant() { return 1; }"));
        let module = graph.add_source(source(
            2,
            "game.main",
            r#"
use game.reward.grant
fn main() { return grant; }
"#,
        ));
        let main = graph
            .module(module)
            .and_then(|module| module.get("main"))
            .expect("main declaration");

        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
        let bindings = graph.bindings(main).expect("main bindings");
        assert!(bindings.resolutions().any(|(_, resolution)| {
            resolution == &BindingResolution::Import("grant".to_owned())
        }));
    }

    #[test]
    fn binding_treats_bare_map_keys_as_keys_not_name_reads() {
        let mut graph = ModuleGraph::new();
        graph.add_source(source(
            1,
            "game.reward",
            r#"
fn main() {
    return { exp: 15 };
}
"#,
        ));

        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    }
}
