use vela_analysis::type_fact::TypeFact;

use crate::{LanguageServiceDatabases, QueryContext, TextRange};

use super::{CompletionContext, CompletionContextKind};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CompletionAnalysis {
    kind: CompletionAnalysisKind,
    context_kind: CompletionContextKind,
    expected_type: Option<TypeFact>,
    expected_name: Option<String>,
    visible_scope: Vec<String>,
}

impl CompletionAnalysis {
    #[must_use]
    pub const fn kind(&self) -> &CompletionAnalysisKind {
        &self.kind
    }

    #[must_use]
    pub const fn context_kind(&self) -> CompletionContextKind {
        self.context_kind
    }

    #[must_use]
    pub const fn expected_type(&self) -> Option<&TypeFact> {
        self.expected_type.as_ref()
    }

    #[must_use]
    pub fn expected_name(&self) -> Option<&str> {
        self.expected_name.as_deref()
    }

    #[must_use]
    pub fn visible_scope(&self) -> &[String] {
        &self.visible_scope
    }

    #[must_use]
    pub(super) fn from_context_only(context: &CompletionContext) -> Self {
        Self {
            kind: CompletionAnalysisKind::Path(PathCompletionCtx {
                kind: PathCompletionKind::Expression,
                type_location: None,
                qualifier: context.module_base().map(ToOwned::to_owned),
            }),
            context_kind: context.kind(),
            expected_type: None,
            expected_name: None,
            visible_scope: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CompletionAnalysisKind {
    Path(PathCompletionCtx),
    DotAccess(DotAccess),
    RecordField(RecordFieldContext),
    CallArgument(CompletionCallArgumentContext),
    Pattern(PatternContext),
    Statement(StatementContext),
    Declaration(CompletionDeclaration),
    LambdaParameter,
    MapKey,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PathCompletionCtx {
    kind: PathCompletionKind,
    type_location: Option<TypeLocation>,
    qualifier: Option<String>,
}

impl PathCompletionCtx {
    #[must_use]
    pub const fn kind(&self) -> PathCompletionKind {
        self.kind
    }

    #[must_use]
    pub const fn type_location(&self) -> Option<&TypeLocation> {
        self.type_location.as_ref()
    }

    #[must_use]
    pub fn qualifier(&self) -> Option<&str> {
        self.qualifier.as_deref()
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PathCompletionKind {
    Expression,
    Type,
    Item,
    Module,
    Pattern,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TypeLocation {
    Parameter,
    Return,
    StructField,
    BuiltinTypeArgument {
        container: String,
        argument_index: usize,
    },
    Other,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DotAccess {
    receiver_range: Option<TextRange>,
    receiver_fact: Option<TypeFact>,
}

impl DotAccess {
    #[must_use]
    pub const fn receiver_range(&self) -> Option<TextRange> {
        self.receiver_range
    }

    #[must_use]
    pub const fn receiver_fact(&self) -> Option<&TypeFact> {
        self.receiver_fact.as_ref()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RecordFieldContext {
    owner_type: Option<Vec<String>>,
}

impl RecordFieldContext {
    #[must_use]
    pub fn owner_type(&self) -> Option<&[String]> {
        self.owner_type.as_deref()
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct CompletionCallArgumentContext {
    active_parameter: usize,
}

impl CompletionCallArgumentContext {
    #[must_use]
    pub const fn active_parameter(&self) -> usize {
        self.active_parameter
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct PatternContext;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct StatementContext;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct CompletionDeclaration {
    kind: CompletionDeclarationKind,
}

impl CompletionDeclaration {
    #[must_use]
    pub const fn kind(&self) -> CompletionDeclarationKind {
        self.kind
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CompletionDeclarationKind {
    Item,
    StructField,
}

pub(super) fn completion_analysis(
    databases: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    context: &CompletionContext,
) -> CompletionAnalysis {
    let (expected_name, expected_type) = expected_call_argument(databases, query);
    CompletionAnalysis {
        kind: analysis_kind(databases, query, context),
        context_kind: context.kind(),
        expected_type,
        expected_name,
        visible_scope: query
            .local_bindings_before_cursor()
            .map(|binding| binding.name.clone())
            .collect(),
    }
}

fn analysis_kind(
    databases: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    context: &CompletionContext,
) -> CompletionAnalysisKind {
    match context.kind() {
        CompletionContextKind::Expression => CompletionAnalysisKind::Path(path_context(
            PathCompletionKind::Expression,
            query,
            context,
        )),
        CompletionContextKind::Item => CompletionAnalysisKind::Declaration(CompletionDeclaration {
            kind: CompletionDeclarationKind::Item,
        }),
        CompletionContextKind::Statement => CompletionAnalysisKind::Statement(StatementContext),
        CompletionContextKind::ModulePath => {
            CompletionAnalysisKind::Path(path_context(PathCompletionKind::Module, query, context))
        }
        CompletionContextKind::Member => CompletionAnalysisKind::DotAccess(DotAccess {
            receiver_range: context.member_receiver_range(),
            receiver_fact: context
                .member_receiver_range()
                .and_then(|range| query.type_fact_for_range(databases, range)),
        }),
        CompletionContextKind::RecordField => {
            CompletionAnalysisKind::RecordField(RecordFieldContext {
                owner_type: context
                    .record_constructor
                    .as_ref()
                    .map(|record| record.path.clone()),
            })
        }
        CompletionContextKind::StructFieldDeclaration => {
            CompletionAnalysisKind::Declaration(CompletionDeclaration {
                kind: CompletionDeclarationKind::StructField,
            })
        }
        CompletionContextKind::MapKey => CompletionAnalysisKind::MapKey,
        CompletionContextKind::Pattern => CompletionAnalysisKind::Pattern(PatternContext),
        CompletionContextKind::NamedArgument => {
            CompletionAnalysisKind::CallArgument(CompletionCallArgumentContext {
                active_parameter: query
                    .call_argument_facts()
                    .map_or(0, |call| call.active_parameter()),
            })
        }
        CompletionContextKind::LambdaParameter => CompletionAnalysisKind::LambdaParameter,
        CompletionContextKind::TypeHint => {
            CompletionAnalysisKind::Path(path_context(PathCompletionKind::Type, query, context))
        }
    }
}

fn path_context(
    kind: PathCompletionKind,
    query: &QueryContext<'_>,
    context: &CompletionContext,
) -> PathCompletionCtx {
    PathCompletionCtx {
        kind,
        type_location: (kind == PathCompletionKind::Type)
            .then(|| type_location(query.text(), context.replace_range().start))
            .flatten(),
        qualifier: context.module_base().map(ToOwned::to_owned),
    }
}

fn expected_call_argument(
    databases: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
) -> (Option<String>, Option<TypeFact>) {
    let Some(call) = query.call_argument_facts() else {
        return (None, None);
    };
    let callables = if let Some(receiver) = call.member_receiver() {
        let method = call
            .callee()
            .rsplit_once('.')
            .map_or(call.callee(), |(_, method)| method);
        query.member_callable_facts(databases, receiver, method, call.args_prefix())
    } else {
        query.callable_facts(databases, call.callee())
    };
    let Some(param) = callables
        .iter()
        .find_map(|callable| callable.params().get(call.active_parameter()))
    else {
        return (None, None);
    };
    (
        Some(param.name().to_owned()),
        Some(param.type_fact().clone()),
    )
}

fn type_location(text: &str, prefix_start: usize) -> Option<TypeLocation> {
    let before_prefix = text.get(..prefix_start)?.trim_end();
    builtin_type_argument_location(before_prefix)
        .or_else(|| type_annotation_location(before_prefix))
        .or(Some(TypeLocation::Other))
}

fn builtin_type_argument_location(before_prefix: &str) -> Option<TypeLocation> {
    let open = before_prefix.rfind('<')?;
    if before_prefix[open + 1..].contains('>') {
        return None;
    }
    let before_open = before_prefix[..open].trim_end();
    let start = before_open
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    let container = &before_open[start..];
    if !matches!(
        container,
        "Array" | "Set" | "Map" | "Iterator" | "Option" | "Result"
    ) {
        return None;
    }
    let argument_index = before_prefix[open + 1..]
        .chars()
        .filter(|ch| *ch == ',')
        .count();
    Some(TypeLocation::BuiltinTypeArgument {
        container: container.to_owned(),
        argument_index,
    })
}

fn type_annotation_location(before_prefix: &str) -> Option<TypeLocation> {
    if before_prefix.ends_with("->") {
        return Some(TypeLocation::Return);
    }
    let before_colon = before_prefix.strip_suffix(':')?.trim_end();
    if before_colon.rsplit_once('{').is_some_and(|(_, tail)| {
        tail.trim()
            .chars()
            .all(|ch| is_identifier_continue(ch) || ch.is_whitespace())
    }) {
        return Some(TypeLocation::StructField);
    }
    Some(TypeLocation::Parameter)
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}
