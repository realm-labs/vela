use vela_analysis::type_fact::TypeFact;

use crate::{LanguageServiceDatabases, QueryContext, TextRange};

use super::{CompletionContext, CompletionContextKind};

mod type_location;

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
    let (expected_name, expected_type, active_parameter) = expected_call_argument(databases, query);
    CompletionAnalysis {
        kind: analysis_kind(databases, query, context, active_parameter),
        context_kind: context.kind(),
        expected_type,
        expected_name,
        visible_scope: query.visible_scope_names(),
    }
}

fn analysis_kind(
    databases: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    context: &CompletionContext,
    active_parameter: Option<usize>,
) -> CompletionAnalysisKind {
    match context.kind() {
        CompletionContextKind::Expression if query.is_service_call() => {
            CompletionAnalysisKind::CallArgument(CompletionCallArgumentContext {
                active_parameter: active_parameter
                    .or_else(|| query.call_active_parameter_index())
                    .unwrap_or(0),
            })
        }
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
                active_parameter: active_parameter.unwrap_or_else(|| {
                    query
                        .call_argument_facts()
                        .map_or(0, |call| call.active_parameter())
                }),
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
            .then(|| type_location::type_location(query, context.replace_range().start)),
        qualifier: context.module_base().map(ToOwned::to_owned),
    }
}

fn expected_call_argument(
    databases: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
) -> (Option<String>, Option<TypeFact>, Option<usize>) {
    let callables = query.call_target_facts(databases);
    let Some((index, param)) = callables.first().and_then(|callable| {
        let index = query.call_parameter_index(callable)?;
        Some((index, callable.params().get(index)?))
    }) else {
        return (None, None, None);
    };
    (
        Some(param.name().to_owned()),
        Some(param.type_fact().clone()),
        Some(index),
    )
}
