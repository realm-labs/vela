use std::collections::BTreeMap;

use vela_analysis::{
    facts::AnalysisFacts,
    registry::RegistryFacts,
    stdlib::{stdlib_function_completion_facts, stdlib_method_fact},
    type_fact::TypeFact,
};
use vela_common::{SourceId, Span};
use vela_hir::binding::{BindingMap, BindingResolution, LocalBinding, LocalBindingKind};
use vela_hir::module_graph::{Declaration, DeclarationKind, ModuleGraph};
use vela_hir::type_hint::{EnumVariantFieldsHint, HirTypeHint};
use vela_syntax::lexer::lex;
use vela_syntax::token::{Keyword, Symbol, Token, TokenKind};

use crate::{DocumentId, LanguageServiceDatabases, LineIndex, Position, TextRange};

mod import_paths;
mod member_receivers;
mod type_hints;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SemanticTokens {
    result_id: String,
    tokens: Vec<SemanticToken>,
}

impl SemanticTokens {
    #[must_use]
    pub fn new(tokens: Vec<SemanticToken>) -> Self {
        let result_id = semantic_tokens_result_id(&tokens);
        Self { result_id, tokens }
    }

    #[must_use]
    pub fn result_id(&self) -> &str {
        &self.result_id
    }

    #[must_use]
    pub fn tokens(&self) -> &[SemanticToken] {
        &self.tokens
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SemanticTokenDelta {
    result_id: String,
    edits: Vec<SemanticTokenEdit>,
}

impl SemanticTokenDelta {
    #[must_use]
    pub fn new(result_id: String, edits: Vec<SemanticTokenEdit>) -> Self {
        Self { result_id, edits }
    }

    #[must_use]
    pub fn result_id(&self) -> &str {
        &self.result_id
    }

    #[must_use]
    pub fn edits(&self) -> &[SemanticTokenEdit] {
        &self.edits
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SemanticTokenEdit {
    start: usize,
    delete_count: usize,
    tokens: Vec<SemanticToken>,
}

impl SemanticTokenEdit {
    #[must_use]
    pub fn new(start: usize, delete_count: usize, tokens: Vec<SemanticToken>) -> Self {
        Self {
            start,
            delete_count,
            tokens,
        }
    }

    #[must_use]
    pub const fn start(&self) -> usize {
        self.start
    }

    #[must_use]
    pub const fn delete_count(&self) -> usize {
        self.delete_count
    }

    #[must_use]
    pub fn tokens(&self) -> &[SemanticToken] {
        &self.tokens
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct SemanticToken {
    start: Position,
    length: usize,
    token_type: SemanticTokenType,
    modifiers: SemanticTokenModifiers,
}

impl SemanticToken {
    #[must_use]
    pub const fn new(
        start: Position,
        length: usize,
        token_type: SemanticTokenType,
        modifiers: SemanticTokenModifiers,
    ) -> Self {
        Self {
            start,
            length,
            token_type,
            modifiers,
        }
    }

    #[must_use]
    pub const fn start(self) -> Position {
        self.start
    }

    #[must_use]
    pub const fn length(self) -> usize {
        self.length
    }

    #[must_use]
    pub const fn token_type(self) -> SemanticTokenType {
        self.token_type
    }

    #[must_use]
    pub const fn modifiers(self) -> SemanticTokenModifiers {
        self.modifiers
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SemanticTokenType {
    Attribute,
    Bytes,
    Comment,
    EnumMember,
    Field,
    Function,
    Keyword,
    Macro,
    Method,
    Module,
    Number,
    Operator,
    Parameter,
    Property,
    String,
    Type,
    Variable,
}

impl SemanticTokenType {
    pub const LEGEND: [Self; 17] = [
        Self::Module,
        Self::Function,
        Self::Method,
        Self::Field,
        Self::Variable,
        Self::Parameter,
        Self::Type,
        Self::EnumMember,
        Self::Property,
        Self::Keyword,
        Self::Number,
        Self::String,
        Self::Bytes,
        Self::Comment,
        Self::Operator,
        Self::Attribute,
        Self::Macro,
    ];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Module => "namespace",
            Self::Function => "function",
            Self::Method => "method",
            Self::Field => "field",
            Self::Variable => "variable",
            Self::Parameter => "parameter",
            Self::Type => "type",
            Self::EnumMember => "enumMember",
            Self::Property => "property",
            Self::Keyword => "keyword",
            Self::Number => "number",
            Self::String => "string",
            Self::Bytes => "bytes",
            Self::Comment => "comment",
            Self::Operator => "operator",
            Self::Attribute => "decorator",
            Self::Macro => "macro",
        }
    }

    #[must_use]
    pub fn legend_index(self) -> u32 {
        Self::LEGEND
            .iter()
            .position(|kind| *kind == self)
            .and_then(|index| u32::try_from(index).ok())
            .expect("semantic token type should be in legend")
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct SemanticTokenModifiers(u32);

impl SemanticTokenModifiers {
    pub const NONE: Self = Self(0);
    pub const DECLARATION: Self = Self(1 << 0);
    pub const DEFINITION: Self = Self(1 << 1);
    pub const READONLY: Self = Self(1 << 2);
    pub const DEPRECATED: Self = Self(1 << 3);
    pub const BUILTIN: Self = Self(1 << 4);
    pub const HOST: Self = Self(1 << 5);
    pub const UNRESOLVED: Self = Self(1 << 6);

    pub const LEGEND: [&'static str; 7] = [
        "declaration",
        "definition",
        "readonly",
        "deprecated",
        "defaultLibrary",
        "host",
        "unresolved",
    ];

    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct SemanticTokenClassification {
    token_type: SemanticTokenType,
    modifiers: SemanticTokenModifiers,
}

impl SemanticTokenClassification {
    const fn new(token_type: SemanticTokenType, modifiers: SemanticTokenModifiers) -> Self {
        Self {
            token_type,
            modifiers,
        }
    }
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn semantic_tokens(&self, document_id: &DocumentId) -> SemanticTokens {
        let Some(source) = self.source_db().records().get(document_id) else {
            return SemanticTokens::new(Vec::new());
        };
        let line_index = LineIndex::new(source.text());
        let lexed = lex(source.source_id(), source.text());
        let member_receivers = self
            .parse_db()
            .parsed_source(document_id)
            .map(member_receivers::member_receiver_ranges)
            .unwrap_or_default();
        let classifications = self.semantic_token_classifications(
            source.source_id(),
            source.text(),
            &lexed.tokens,
            &member_receivers,
        );
        let mut semantic_tokens = Vec::new();

        for range in comment_ranges(source.text(), &lexed.tokens) {
            push_semantic_token_slices(
                source.text(),
                &line_index,
                range,
                SemanticTokenType::Comment,
                SemanticTokenModifiers::NONE,
                &mut semantic_tokens,
            );
        }

        for token in lexed.tokens {
            let Some(range) = token_range(token.span) else {
                continue;
            };
            let classification = classifications
                .get(&(range.start, range.end))
                .copied()
                .or_else(|| {
                    token_type(&token.kind).map(|token_type| {
                        SemanticTokenClassification::new(token_type, SemanticTokenModifiers::NONE)
                    })
                });
            let Some(classification) = classification else {
                continue;
            };
            push_semantic_token_slices(
                source.text(),
                &line_index,
                range,
                classification.token_type,
                classification.modifiers,
                &mut semantic_tokens,
            );
        }

        semantic_tokens.sort_by_key(|token| {
            let start = token.start();
            (start.line, start.character)
        });
        SemanticTokens::new(semantic_tokens)
    }

    #[must_use]
    pub fn semantic_token_delta(
        &self,
        document_id: &DocumentId,
        previous_result_id: &str,
    ) -> SemanticTokenDelta {
        let current = self.semantic_tokens(document_id);
        if current.result_id() == previous_result_id {
            return SemanticTokenDelta::new(current.result_id().to_owned(), Vec::new());
        }

        let previous_token_count = semantic_token_count_from_result_id(previous_result_id);
        SemanticTokenDelta::new(
            current.result_id().to_owned(),
            vec![SemanticTokenEdit::new(
                0,
                previous_token_count,
                current.tokens().to_vec(),
            )],
        )
    }

    fn semantic_token_classifications(
        &self,
        source_id: SourceId,
        text: &str,
        tokens: &[Token],
        member_receivers: &BTreeMap<(usize, usize), TextRange>,
    ) -> BTreeMap<(usize, usize), SemanticTokenClassification> {
        let mut classifications = BTreeMap::new();
        let graph = self.hir_db().graph();
        let facts = AnalysisFacts::from_module_graph(graph);
        for token in tokens {
            let TokenKind::Ident(name) = &token.kind else {
                continue;
            };
            let Some(range) = token_range(token.span) else {
                continue;
            };
            if let Some(classification) = self.semantic_classification_for_identifier(
                source_id,
                text,
                name,
                range,
                &facts,
                member_receivers,
            ) {
                classifications.insert((range.start, range.end), classification);
            }
        }
        classifications
    }

    fn semantic_classification_for_identifier(
        &self,
        source_id: SourceId,
        text: &str,
        name: &str,
        range: TextRange,
        facts: &AnalysisFacts,
        member_receivers: &BTreeMap<(usize, usize), TextRange>,
    ) -> Option<SemanticTokenClassification> {
        let span = span_for_range(source_id, range)?;
        let graph = self.hir_db().graph();
        let schema = self.schema_db().facts();

        if let Some(classification) = import_paths::classification(graph, text, name, range, span) {
            return Some(classification);
        }

        for declaration in graph.declarations() {
            if declaration.span.source != source_id || !declaration.span.contains(span.start) {
                continue;
            }
            if let Some(classification) =
                member_declaration_classification(graph, declaration, text, name, range)
            {
                return Some(classification);
            }
            if let Some(classification) =
                type_hints::classification(graph, declaration, schema, text, name, range)
            {
                return Some(classification);
            }
            if let Some(bindings) = graph.bindings(declaration.id) {
                let member_context = MemberUseContext {
                    graph,
                    bindings,
                    facts,
                    schema,
                    text,
                    member_receivers,
                };
                if let Some(classification) =
                    member_use_classification(&member_context, name, range)
                {
                    return Some(classification);
                }
                if let Some(classification) =
                    local_declaration_classification(bindings, name, range)
                {
                    return Some(classification);
                }
                if let Some(classification) =
                    resolved_identifier_classification(bindings, span, self)
                {
                    return Some(classification);
                }
                if let Some(classification) = function_call_classification(schema, text, range) {
                    return Some(classification);
                }
                if let Some(classification) = unresolved_identifier_classification(bindings, span) {
                    return Some(classification);
                }
            }
        }

        graph
            .declarations()
            .find(|declaration| {
                declaration.span.source == source_id
                    && declaration.span.contains(span.start)
                    && declaration.name == name
                    && token_text(text, range) == Some(name)
            })
            .map(declaration_classification)
    }
}

fn semantic_tokens_result_id(tokens: &[SemanticToken]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    hash_result_id_part(&mut hash, tokens.len() as u64);
    for token in tokens {
        hash_result_id_part(&mut hash, token.start().line as u64);
        hash_result_id_part(&mut hash, token.start().character as u64);
        hash_result_id_part(&mut hash, token.length() as u64);
        hash_result_id_part(&mut hash, u64::from(token.token_type().legend_index()));
        hash_result_id_part(&mut hash, u64::from(token.modifiers().bits()));
    }
    format!("v1:{}:{hash:016x}", tokens.len())
}

fn hash_result_id_part(hash: &mut u64, value: u64) {
    for byte in value.to_le_bytes() {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

fn semantic_token_count_from_result_id(result_id: &str) -> usize {
    let mut parts = result_id.split(':');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some("v1"), Some(count), Some(_hash), None) => count.parse().unwrap_or(0),
        _ => 0,
    }
}

fn member_use_classification(
    context: &MemberUseContext<'_>,
    name: &str,
    range: TextRange,
) -> Option<SemanticTokenClassification> {
    let receiver_range = *context.member_receivers.get(&(range.start, range.end))?;
    let receiver_span = span_for_range(
        context
            .graph
            .declaration(context.bindings.declaration)?
            .span
            .source,
        receiver_range,
    )?;
    let receiver = context
        .bindings
        .resolution_at_span(receiver_span)
        .and_then(|resolution| {
            type_fact_for_resolution(resolution, context.bindings, context.facts, context.schema)
        })?;
    let is_call = next_non_whitespace(context.text, range.end) == Some('(');

    if is_call
        && let Some(classification) =
            method_use_classification(context.graph, context.schema, &receiver, name)
    {
        return Some(classification);
    }

    field_use_classification(context.graph, context.schema, &receiver, name).or_else(|| {
        is_call
            .then(|| stdlib_method_fact(&receiver, name, None))
            .flatten()
            .map(|_| {
                SemanticTokenClassification::new(
                    SemanticTokenType::Method,
                    SemanticTokenModifiers::BUILTIN,
                )
            })
    })
}

struct MemberUseContext<'a> {
    graph: &'a ModuleGraph,
    bindings: &'a BindingMap,
    facts: &'a AnalysisFacts,
    schema: &'a RegistryFacts,
    text: &'a str,
    member_receivers: &'a BTreeMap<(usize, usize), TextRange>,
}

fn method_use_classification(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    receiver: &TypeFact,
    name: &str,
) -> Option<SemanticTokenClassification> {
    if let Some(modifiers) = schema_method_modifiers(schema, receiver, name) {
        return Some(SemanticTokenClassification::new(
            SemanticTokenType::Method,
            modifiers,
        ));
    }
    if stdlib_method_fact(receiver, name, None).is_some() {
        return Some(SemanticTokenClassification::new(
            SemanticTokenType::Method,
            SemanticTokenModifiers::BUILTIN,
        ));
    }
    script_method_exists(graph, receiver, name).then(|| {
        SemanticTokenClassification::new(SemanticTokenType::Method, SemanticTokenModifiers::NONE)
    })
}

fn field_use_classification(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    receiver: &TypeFact,
    name: &str,
) -> Option<SemanticTokenClassification> {
    if schema_field_exists(schema, receiver, name) {
        return Some(SemanticTokenClassification::new(
            SemanticTokenType::Property,
            host_modifier(receiver),
        ));
    }
    script_field_exists(graph, receiver, name).then(|| {
        SemanticTokenClassification::new(SemanticTokenType::Property, SemanticTokenModifiers::NONE)
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
                .or_else(|| schema_fact_for_local_hint(binding.type_hint.as_ref(), schema))
        }
        BindingResolution::Declaration(declaration) => facts.declaration(*declaration).cloned(),
        BindingResolution::Import(_) | BindingResolution::QualifiedPath(_) => None,
    }
}

fn schema_fact_for_local_hint(
    hint: Option<&HirTypeHint>,
    schema: &RegistryFacts,
) -> Option<TypeFact> {
    let hint = hint?;
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

fn schema_method_modifiers(
    schema: &RegistryFacts,
    receiver: &TypeFact,
    method: &str,
) -> Option<SemanticTokenModifiers> {
    owner_names(receiver).iter().find_map(|owner| {
        if schema.method_fact(owner, method).is_some() {
            Some(host_modifier(receiver))
        } else {
            schema
                .trait_method_fact(owner, method)
                .map(|_| SemanticTokenModifiers::HOST)
        }
    })
}

fn schema_field_exists(schema: &RegistryFacts, receiver: &TypeFact, field: &str) -> bool {
    owner_names(receiver)
        .iter()
        .any(|owner| schema.field_fact(owner, field).is_some())
}

fn script_method_exists(graph: &ModuleGraph, receiver: &TypeFact, method: &str) -> bool {
    let owner_names = owner_names(receiver);
    graph
        .declarations()
        .any(|declaration| match declaration.kind {
            DeclarationKind::Impl => {
                let Some(metadata) = graph.impl_metadata(declaration.id) else {
                    return false;
                };
                let targets = impl_target_names(graph, declaration, &metadata.target_path);
                targets.iter().any(|target| owner_names.contains(target))
                    && metadata.methods.iter().any(|entry| entry.name == method)
            }
            DeclarationKind::Trait => {
                owner_names
                    .iter()
                    .any(|owner| declaration_name_matches(graph, declaration, owner))
                    && graph
                        .trait_shape(declaration.id)
                        .is_some_and(|shape| shape.methods.iter().any(|entry| entry.name == method))
            }
            DeclarationKind::Const
            | DeclarationKind::Enum
            | DeclarationKind::Function
            | DeclarationKind::Global
            | DeclarationKind::Struct => false,
        })
}

fn script_field_exists(graph: &ModuleGraph, receiver: &TypeFact, field: &str) -> bool {
    let owner_names = owner_names(receiver);
    graph.declarations().any(|declaration| {
        if !matches!(declaration.kind, DeclarationKind::Struct) {
            return false;
        }
        owner_names
            .iter()
            .any(|owner| declaration_name_matches(graph, declaration, owner))
            && graph
                .struct_shape(declaration.id)
                .is_some_and(|shape| shape.fields.iter().any(|entry| entry.name == field))
    })
}

fn owner_names(receiver: &TypeFact) -> Vec<String> {
    let Some(owner) = receiver_owner_name(receiver) else {
        return Vec::new();
    };
    let mut names = vec![owner.clone()];
    if let Some(short) = owner.rsplit("::").next()
        && short != owner
    {
        names.push(short.to_owned());
    }
    names
}

fn receiver_owner_name(receiver: &TypeFact) -> Option<String> {
    match receiver {
        TypeFact::Host { name } | TypeFact::Record { name } | TypeFact::Trait { name } => {
            Some(name.clone())
        }
        TypeFact::Enum {
            name,
            variant: Some(variant),
        } => Some(format!("{name}::{variant}")),
        TypeFact::Enum {
            name,
            variant: None,
        } => Some(name.clone()),
        _ => None,
    }
}

fn host_modifier(receiver: &TypeFact) -> SemanticTokenModifiers {
    if matches!(receiver, TypeFact::Host { .. }) {
        SemanticTokenModifiers::HOST
    } else {
        SemanticTokenModifiers::NONE
    }
}

fn impl_target_names(
    graph: &ModuleGraph,
    declaration: &Declaration,
    target_path: &[String],
) -> Vec<String> {
    let raw = target_path.join("::");
    let mut names = vec![raw.clone()];
    if target_path.len() == 1
        && let Some(module_path) = graph.module_path(declaration.module)
    {
        let qualified = module_path
            .segments()
            .iter()
            .chain(target_path.iter())
            .cloned()
            .collect::<Vec<_>>()
            .join("::");
        if qualified != raw {
            names.push(qualified);
        }
    }
    names
}

fn declaration_name_matches(graph: &ModuleGraph, declaration: &Declaration, owner: &str) -> bool {
    declaration.name == owner || qualified_declaration_name(graph, declaration) == owner
}

fn qualified_declaration_name(graph: &ModuleGraph, declaration: &Declaration) -> String {
    graph
        .module_path(declaration.module)
        .map(|path| {
            path.segments()
                .iter()
                .chain(std::iter::once(&declaration.name))
                .cloned()
                .collect::<Vec<_>>()
                .join("::")
        })
        .unwrap_or_else(|| declaration.name.clone())
}

fn next_non_whitespace(text: &str, offset: usize) -> Option<char> {
    text.get(offset..)?.chars().find(|ch| !ch.is_whitespace())
}

fn local_declaration_classification(
    bindings: &BindingMap,
    name: &str,
    range: TextRange,
) -> Option<SemanticTokenClassification> {
    bindings
        .locals()
        .find(|binding| binding.name == name && span_contains_range(binding.span, range))
        .map(local_declaration_token_classification)
}

fn resolved_identifier_classification(
    bindings: &BindingMap,
    span: Span,
    databases: &LanguageServiceDatabases,
) -> Option<SemanticTokenClassification> {
    let resolution = bindings.resolution_at_span(span)?;
    match resolution {
        BindingResolution::Local(local) => bindings.local(*local).map(local_use_classification),
        BindingResolution::Declaration(declaration) => databases
            .hir_db()
            .graph()
            .declaration(*declaration)
            .map(declaration_use_classification),
        BindingResolution::Import(_) | BindingResolution::QualifiedPath(_) => None,
    }
}

fn unresolved_identifier_classification(
    bindings: &BindingMap,
    span: Span,
) -> Option<SemanticTokenClassification> {
    matches!(
        bindings.resolution_at_span(span)?,
        BindingResolution::Import(_) | BindingResolution::QualifiedPath(_)
    )
    .then(|| {
        SemanticTokenClassification::new(
            SemanticTokenType::Variable,
            SemanticTokenModifiers::UNRESOLVED,
        )
    })
}

fn function_call_classification(
    schema: &RegistryFacts,
    text: &str,
    range: TextRange,
) -> Option<SemanticTokenClassification> {
    let path = call_path_ending_at(text, range)?;
    let qualified = path.join("::");
    if schema.function_fact(&qualified).is_some() {
        return Some(SemanticTokenClassification::new(
            SemanticTokenType::Function,
            SemanticTokenModifiers::HOST,
        ));
    }
    stdlib_function_completion_facts()
        .iter()
        .any(|function| function.name == qualified)
        .then(|| {
            SemanticTokenClassification::new(
                SemanticTokenType::Function,
                SemanticTokenModifiers::BUILTIN,
            )
        })
}

fn call_path_ending_at(text: &str, range: TextRange) -> Option<Vec<String>> {
    (next_non_whitespace(text, range.end) == Some('(')).then_some(())?;
    let mut path = vec![token_text(text, range)?.to_owned()];
    let mut prefix = text.get(..range.start)?.trim_end();

    while let Some(before_separator) = prefix.strip_suffix("::") {
        let end = before_separator.trim_end().len();
        let start = identifier_start_before(before_separator, end)?;
        path.push(before_separator.get(start..end)?.to_owned());
        prefix = before_separator.get(..start)?.trim_end();
    }

    path.reverse();
    Some(path)
}

fn identifier_start_before(text: &str, end: usize) -> Option<usize> {
    if end == 0 {
        return None;
    }
    let prefix = text.get(..end)?;
    let start = prefix
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    (start < end).then_some(start)
}

fn local_declaration_token_classification(binding: &LocalBinding) -> SemanticTokenClassification {
    SemanticTokenClassification::new(
        local_token_type(binding.kind),
        SemanticTokenModifiers::DECLARATION,
    )
}

fn local_use_classification(binding: &LocalBinding) -> SemanticTokenClassification {
    SemanticTokenClassification::new(local_token_type(binding.kind), SemanticTokenModifiers::NONE)
}

fn local_token_type(kind: LocalBindingKind) -> SemanticTokenType {
    match kind {
        LocalBindingKind::Parameter | LocalBindingKind::LambdaParameter => {
            SemanticTokenType::Parameter
        }
        LocalBindingKind::Let | LocalBindingKind::For | LocalBindingKind::Pattern => {
            SemanticTokenType::Variable
        }
    }
}

fn declaration_classification(declaration: &Declaration) -> SemanticTokenClassification {
    SemanticTokenClassification::new(
        declaration_token_type(declaration.kind),
        SemanticTokenModifiers::DECLARATION.union(SemanticTokenModifiers::DEFINITION),
    )
}

fn declaration_use_classification(declaration: &Declaration) -> SemanticTokenClassification {
    let modifiers = if matches!(declaration.kind, DeclarationKind::Const) {
        SemanticTokenModifiers::READONLY
    } else {
        SemanticTokenModifiers::NONE
    };
    SemanticTokenClassification::new(declaration_token_type(declaration.kind), modifiers)
}

fn declaration_token_type(kind: DeclarationKind) -> SemanticTokenType {
    match kind {
        DeclarationKind::Const | DeclarationKind::Global => SemanticTokenType::Variable,
        DeclarationKind::Function => SemanticTokenType::Function,
        DeclarationKind::Struct | DeclarationKind::Enum | DeclarationKind::Trait => {
            SemanticTokenType::Type
        }
        DeclarationKind::Impl => SemanticTokenType::Type,
    }
}

fn member_declaration_classification(
    graph: &ModuleGraph,
    declaration: &Declaration,
    text: &str,
    name: &str,
    range: TextRange,
) -> Option<SemanticTokenClassification> {
    match declaration.kind {
        DeclarationKind::Struct => graph
            .struct_shape(declaration.id)?
            .fields
            .iter()
            .find(|field| member_name_matches(text, field.span, &field.name, name, range))
            .map(|_| member_declaration_token_classification(SemanticTokenType::Field)),
        DeclarationKind::Enum => graph.enum_shape(declaration.id).and_then(|shape| {
            shape.variants.iter().find_map(|variant| {
                if member_name_matches(text, variant.span, &variant.name, name, range) {
                    return Some(member_declaration_token_classification(
                        SemanticTokenType::EnumMember,
                    ));
                }

                match &variant.fields {
                    EnumVariantFieldsHint::Unit => None,
                    EnumVariantFieldsHint::Tuple(fields) => fields
                        .iter()
                        .find(|field| {
                            member_name_matches(text, field.span, &field.name, name, range)
                        })
                        .map(|_| member_declaration_token_classification(SemanticTokenType::Field)),
                    EnumVariantFieldsHint::Record(fields) => fields
                        .iter()
                        .find(|field| {
                            member_name_matches(text, field.span, &field.name, name, range)
                        })
                        .map(|_| member_declaration_token_classification(SemanticTokenType::Field)),
                }
            })
        }),
        DeclarationKind::Trait => graph.trait_shape(declaration.id).and_then(|shape| {
            shape
                .methods
                .iter()
                .find(|method| {
                    method.name == name
                        && span_contains_range(method.span, range)
                        && token_text(text, range) == Some(name)
                        && preceded_by_fn_keyword(text, range)
                })
                .map(|_| member_declaration_token_classification(SemanticTokenType::Method))
        }),
        DeclarationKind::Impl => graph.impl_metadata(declaration.id).and_then(|metadata| {
            metadata
                .methods
                .iter()
                .find(|method| {
                    method.name == name
                        && span_contains_range(declaration.span, range)
                        && token_text(text, range) == Some(name)
                        && range_ends_before_span_start(range, method.span)
                        && preceded_by_fn_keyword(text, range)
                })
                .map(|_| member_declaration_token_classification(SemanticTokenType::Method))
        }),
        DeclarationKind::Const | DeclarationKind::Global | DeclarationKind::Function => None,
    }
}

fn member_declaration_token_classification(
    token_type: SemanticTokenType,
) -> SemanticTokenClassification {
    SemanticTokenClassification::new(
        token_type,
        SemanticTokenModifiers::DECLARATION.union(SemanticTokenModifiers::DEFINITION),
    )
}

fn member_name_matches(
    text: &str,
    span: Span,
    expected: &str,
    name: &str,
    range: TextRange,
) -> bool {
    expected == name && span_contains_range(span, range) && token_text(text, range) == Some(name)
}

fn range_ends_before_span_start(range: TextRange, span: Span) -> bool {
    usize::try_from(span.start).is_ok_and(|start| range.end <= start)
}

fn preceded_by_fn_keyword(text: &str, range: TextRange) -> bool {
    let Some(prefix) = text.get(..range.start) else {
        return false;
    };
    let trimmed = prefix.trim_end();
    let Some(before_fn) = trimmed.strip_suffix("fn") else {
        return false;
    };
    before_fn
        .chars()
        .last()
        .is_none_or(|ch| !is_identifier_continue(ch))
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn span_contains_range(span: Span, range: TextRange) -> bool {
    let Ok(start) = u32::try_from(range.start) else {
        return false;
    };
    let Ok(end) = u32::try_from(range.end) else {
        return false;
    };
    span.start <= start && end <= span.end
}

fn span_for_range(source_id: SourceId, range: TextRange) -> Option<Span> {
    let start = u32::try_from(range.start).ok()?;
    let end = u32::try_from(range.end).ok()?;
    Some(Span::new(source_id, start, end))
}

fn token_text(text: &str, range: TextRange) -> Option<&str> {
    text.get(range.start..range.end)
}

fn token_type(kind: &TokenKind) -> Option<SemanticTokenType> {
    match kind {
        TokenKind::Keyword(keyword) => Some(keyword_token_type(*keyword)),
        TokenKind::Int(_) | TokenKind::Float(_) => Some(SemanticTokenType::Number),
        TokenKind::Char(_) | TokenKind::String(_) | TokenKind::InterpolatedString(_) => {
            Some(SemanticTokenType::String)
        }
        TokenKind::Bytes(_) => Some(SemanticTokenType::Bytes),
        TokenKind::Symbol(symbol) => symbol_token_type(*symbol),
        TokenKind::Ident(_) => Some(SemanticTokenType::Variable),
        TokenKind::Eof => None,
    }
}

fn keyword_token_type(keyword: Keyword) -> SemanticTokenType {
    match keyword {
        Keyword::True | Keyword::False | Keyword::Null => SemanticTokenType::Keyword,
        _ => SemanticTokenType::Keyword,
    }
}

fn symbol_token_type(symbol: Symbol) -> Option<SemanticTokenType> {
    match symbol {
        Symbol::Hash => Some(SemanticTokenType::Attribute),
        _ => Some(SemanticTokenType::Operator),
    }
}

fn comment_ranges(text: &str, tokens: &[Token]) -> Vec<TextRange> {
    let token_ranges = sorted_token_ranges(tokens);
    let mut ranges = Vec::new();
    let mut token_index = 0;
    let mut offset = 0;

    if text.starts_with("#!") {
        let end = text.find('\n').unwrap_or(text.len());
        ranges.push(TextRange::new(0, end));
        offset = end;
    }

    while offset < text.len() {
        while token_index < token_ranges.len() && token_ranges[token_index].end <= offset {
            token_index += 1;
        }

        if token_index < token_ranges.len() && token_ranges[token_index].start == offset {
            offset = token_ranges[token_index].end;
            token_index += 1;
            continue;
        }

        let Some(rest) = text.get(offset..) else {
            break;
        };
        if rest.starts_with("//") {
            let end = offset + rest.find('\n').unwrap_or(rest.len());
            ranges.push(TextRange::new(offset, end));
            offset = end;
            continue;
        }
        if rest.starts_with("/*") {
            let end = block_comment_end(text, offset);
            ranges.push(TextRange::new(offset, end));
            offset = end;
            continue;
        }

        offset += rest.chars().next().map_or(1, |ch| ch.len_utf8());
    }

    ranges
}

fn sorted_token_ranges(tokens: &[Token]) -> Vec<TextRange> {
    let mut ranges: Vec<_> = tokens
        .iter()
        .filter_map(|token| token_range(token.span))
        .collect();
    ranges.sort_by_key(|range| (range.start, range.end));
    ranges
}

fn block_comment_end(text: &str, start: usize) -> usize {
    let mut offset = start + "/*".len();
    let mut depth = 1_u32;

    while offset < text.len() {
        let Some(rest) = text.get(offset..) else {
            return text.len();
        };
        if rest.starts_with("/*") {
            depth = depth.saturating_add(1);
            offset += "/*".len();
        } else if rest.starts_with("*/") {
            depth = depth.saturating_sub(1);
            offset += "*/".len();
            if depth == 0 {
                return offset;
            }
        } else {
            offset += rest.chars().next().map_or(1, |ch| ch.len_utf8());
        }
    }

    text.len()
}

fn push_semantic_token_slices(
    text: &str,
    line_index: &LineIndex,
    range: TextRange,
    token_type: SemanticTokenType,
    modifiers: SemanticTokenModifiers,
    tokens: &mut Vec<SemanticToken>,
) {
    let Some(slice) = text.get(range.start..range.end) else {
        return;
    };
    let mut line_start = range.start;
    let mut offset = range.start;

    for part in slice.split_inclusive('\n') {
        let line_end = offset + part.trim_end_matches('\n').len();
        push_non_empty_token(
            line_index, line_start, line_end, token_type, modifiers, tokens,
        );
        offset += part.len();
        line_start = offset;
    }

    if !slice.ends_with('\n') && offset < range.end {
        push_non_empty_token(line_index, offset, range.end, token_type, modifiers, tokens);
    }
}

fn push_non_empty_token(
    line_index: &LineIndex,
    start: usize,
    end: usize,
    token_type: SemanticTokenType,
    modifiers: SemanticTokenModifiers,
    tokens: &mut Vec<SemanticToken>,
) {
    if start >= end {
        return;
    }
    let position = line_index.position(start);
    let length = end.saturating_sub(start);
    tokens.push(SemanticToken::new(position, length, token_type, modifiers));
}

fn token_range(span: vela_common::Span) -> Option<TextRange> {
    let start = usize::try_from(span.start).ok()?;
    let end = usize::try_from(span.end).ok()?;
    (start < end).then(|| TextRange::new(start, end))
}

#[cfg(test)]
mod tests;
