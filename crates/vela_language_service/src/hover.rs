use vela_analysis::facts::AnalysisFacts;
use vela_analysis::registry::{RegistryEffectFact, RegistryFacts};
use vela_analysis::stdlib::{
    StdlibFunctionFact, StdlibMethodFact, stdlib_function_completion_facts, stdlib_method_fact,
};
use vela_analysis::type_fact::TypeFact;
use vela_common::Span;
use vela_hir::binding::{BindingMap, BindingResolution, LocalBinding, LocalBindingKind};
use vela_hir::module_graph::{Declaration, DeclarationKind};

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

        self.schema_symbol_hover(&token.text, range)
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
        if let Some(hover) = self.schema_member_hover(&receiver_fact, token, range) {
            return Some(hover);
        }
        stdlib_method_hover(&receiver_fact, &token.text, range)
    }

    fn schema_member_hover(
        &self,
        receiver_fact: &TypeFact,
        token: &HoverToken,
        range: DiagnosticRange,
    ) -> Option<Hover> {
        let owner = fact_owner_name(receiver_fact)?;
        let schema = self.schema_db().facts();
        if let Some(fact) = schema.field_fact(&owner, &token.text) {
            let detail = schema
                .field_access_fact(&owner, &token.text)
                .map_or_else(|| fact.display_name(), |access| {
                    let permissions = permissions_detail(&access.required_permissions);
                    format!(
                        "{}; writable: {}; reflect_readable: {}; reflect_writable: {}; permissions: {permissions}",
                        fact.display_name(),
                        access.writable,
                        access.reflect_readable,
                        access.reflect_writable
                    )
                });
            return Some(Hover {
                range,
                label: format!("{owner}.{}", token.text),
                kind: HoverKind::Field,
                detail,
                docs: None,
            });
        }
        schema.method_fact(&owner, &token.text).map(|fact| Hover {
            range,
            label: format!("{owner}.{}", token.text),
            kind: HoverKind::Method,
            detail: method_detail(schema, &owner, &token.text, fact),
            docs: None,
        })
    }

    fn schema_symbol_hover(&self, name: &str, range: DiagnosticRange) -> Option<Hover> {
        let schema = self.schema_db().facts();
        if let Some(fact) = schema.type_fact(name) {
            return Some(Hover {
                range,
                label: name.to_owned(),
                kind: HoverKind::Type,
                detail: fact.display_name(),
                docs: None,
            });
        }
        if let Some(fact) = schema.trait_fact(name) {
            return Some(Hover {
                range,
                label: name.to_owned(),
                kind: HoverKind::Type,
                detail: fact.display_name(),
                docs: None,
            });
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
            .map(|function| Hover {
                range,
                label: function.name.clone(),
                kind: HoverKind::Function,
                detail: function_detail(schema, &function.name, &function.fact),
                docs: None,
            })
    }
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
        BindingResolution::Declaration(declaration) => graph
            .declaration(*declaration)
            .map(|declaration| hover_from_declaration(graph, facts, declaration, range)),
        BindingResolution::Import(name) => Some(Hover {
            range,
            label: name.clone(),
            kind: HoverKind::Unknown,
            detail: "unresolved import".to_owned(),
            docs: None,
        }),
        BindingResolution::QualifiedPath(path) => {
            let qualified = path.join("::");
            databases
                .schema_symbol_hover(&qualified, range)
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
            .cloned()
    } else {
        None
    }
}

fn fact_owner_name(fact: &TypeFact) -> Option<String> {
    match fact {
        TypeFact::Host { name } | TypeFact::Record { name } | TypeFact::Enum { name, .. } => {
            Some(name.clone())
        }
        _ => None,
    }
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

fn stdlib_function_detail(function: &StdlibFunctionFact) -> String {
    TypeFact::function(function.params.clone(), function.returns.clone()).display_name()
}

fn stdlib_method_detail(method: &StdlibMethodFact) -> String {
    TypeFact::function(method.params.clone(), method.returns.clone()).display_name()
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

fn declaration_docs(
    graph: &vela_hir::module_graph::ModuleGraph,
    declaration: &Declaration,
) -> Option<String> {
    graph
        .declaration_attrs(declaration.id)
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
mod tests {
    use vela_analysis::registry::{RegistryEffectFact, RegistryFacts};

    use super::*;
    use crate::{
        SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
    };

    #[test]
    fn hover_degrades_to_any_without_schema() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub fn main(player: Player) { return player }";
        let databases = databases_for(&document, text, RegistryFacts::default());

        let hover = databases
            .hover(
                &document,
                Position::new(0, text.find("Player").expect("type hint")),
            )
            .expect("hover should degrade unknown type hints");

        assert_eq!(hover.kind(), HoverKind::Type);
        assert_eq!(hover.label(), "Player");
        assert_eq!(hover.detail(), "Any");
    }

    #[test]
    fn hover_reports_effects_and_permissions() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub fn main(player: Player) { player.grant(1) }";
        let mut schema = RegistryFacts::default();
        schema.insert_type("Player", TypeFact::host("Player"));
        schema.insert_method(
            "Player",
            "grant",
            TypeFact::function(vec![TypeFact::I64], TypeFact::BOOL),
        );
        schema.insert_method_effect("Player", "grant", RegistryEffectFact::host_write());
        schema.insert_method_access(vela_analysis::registry::RegistryMethodAccessFact {
            owner: "Player".to_owned(),
            name: "grant".to_owned(),
            public: true,
            reflect_callable: true,
            required_permissions: vec!["player.reward".to_owned()],
        });
        let databases = databases_for(&document, text, schema);

        let hover = databases
            .hover(
                &document,
                Position::new(0, text.find("grant").expect("method name")),
            )
            .expect("hover should resolve schema method");

        assert_eq!(hover.kind(), HoverKind::Method);
        assert_eq!(hover.label(), "Player.grant");
        assert!(hover.detail().contains("Function(i64) -> bool"));
        assert!(hover.detail().contains("effects: writes_host"));
        assert!(hover.detail().contains("permissions: player.reward"));
    }

    #[test]
    fn hover_reports_script_parameter_fact() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub fn main(amount: i64) -> i64 { return amount }";
        let databases = databases_for(&document, text, RegistryFacts::default());

        let hover = databases
            .hover(
                &document,
                Position::new(0, text.rfind("amount").expect("amount use")),
            )
            .expect("hover should resolve parameter use");

        assert_eq!(hover.kind(), HoverKind::Parameter);
        assert_eq!(hover.label(), "amount");
        assert_eq!(hover.detail(), "i64");
    }

    #[test]
    fn hover_reports_stdlib_function_fact() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub fn main() { math::max(1, 2) }";
        let databases = databases_for(&document, text, RegistryFacts::default());

        let hover = databases
            .hover(
                &document,
                Position::new(0, text.find("max").expect("stdlib function")),
            )
            .expect("hover should resolve stdlib function");

        assert_eq!(hover.kind(), HoverKind::Function);
        assert_eq!(hover.label(), "math::max");
        assert_eq!(
            hover.detail(),
            "Function(i64 | f64, i64 | f64) -> i64 | f64"
        );
    }

    #[test]
    fn hover_reports_stdlib_method_fact() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub fn main(scores: Array<i64>) { scores.filter(|score| score > 0) }";
        let databases = databases_for(&document, text, RegistryFacts::default());

        let hover = databases
            .hover(
                &document,
                Position::new(0, text.find("filter").expect("stdlib method")),
            )
            .expect("hover should resolve stdlib method");

        assert_eq!(hover.kind(), HoverKind::Method);
        assert_eq!(hover.label(), "Array(i64).filter");
        assert_eq!(
            hover.detail(),
            "Function(Function(i64) -> bool) -> Array(i64)"
        );
    }

    fn databases_for(
        document: &DocumentId,
        text: &str,
        schema: RegistryFacts,
    ) -> LanguageServiceDatabases {
        let files = vec![SourceFileSnapshot::new(document.clone(), text)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.set_schema_facts(schema);
        databases.update(&project);
        databases
    }
}
