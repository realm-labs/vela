use vela_analysis::completion::CompletionKind as AnalysisCompletionKind;

use crate::TextRange;

use super::{lambda_parameter::LambdaParameterContext, map_key::MapKeyContext};

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub enum CompletionKind {
    Keyword,
    Binding,
    Const,
    Field,
    Method,
    Module,
    Variant,
    Function,
    Type,
    Trait,
    Parameter,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CompletionInsertFormat {
    PlainText,
    Snippet,
}

impl From<AnalysisCompletionKind> for CompletionKind {
    fn from(value: AnalysisCompletionKind) -> Self {
        match value {
            AnalysisCompletionKind::Binding => Self::Binding,
            AnalysisCompletionKind::Const => Self::Const,
            AnalysisCompletionKind::Field => Self::Field,
            AnalysisCompletionKind::Method => Self::Method,
            AnalysisCompletionKind::Module => Self::Module,
            AnalysisCompletionKind::Variant => Self::Variant,
            AnalysisCompletionKind::Function => Self::Function,
            AnalysisCompletionKind::Type => Self::Type,
            AnalysisCompletionKind::Trait => Self::Trait,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CompletionContextKind {
    Item,
    Statement,
    Expression,
    Global,
    ModulePath,
    Member,
    RecordField,
    MapKey,
    Pattern,
    NamedArgument,
    LambdaParameter,
    TypeHint,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CompletionItem {
    pub(super) label: String,
    pub(super) kind: CompletionKind,
    pub(super) detail: String,
    pub(super) insert_text: Option<String>,
    pub(super) insert_format: CompletionInsertFormat,
    pub(super) sort_text: Option<String>,
}

impl CompletionItem {
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn kind(&self) -> CompletionKind {
        self.kind
    }

    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }

    #[must_use]
    pub fn insert_text(&self) -> Option<&str> {
        self.insert_text.as_deref()
    }

    #[must_use]
    pub const fn insert_format(&self) -> CompletionInsertFormat {
        self.insert_format
    }

    #[must_use]
    pub fn sort_text(&self) -> Option<&str> {
        self.sort_text.as_deref()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CompletionContext {
    pub(super) kind: CompletionContextKind,
    pub(super) prefix: String,
    pub(super) replace_range: TextRange,
    pub(super) module_base: Option<String>,
    pub(super) member_receiver: Option<MemberReceiver>,
    pub(super) record_constructor: Option<RecordConstructor>,
    pub(super) map_key: Option<MapKeyContext>,
    pub(super) call_arguments: Option<CallArgumentContext>,
    pub(super) lambda_parameter: Option<LambdaParameterContext>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct MemberReceiver {
    pub(super) range: TextRange,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct RecordConstructor {
    pub(super) path: Vec<String>,
    pub(super) field_names: Vec<String>,
    pub(super) current_module: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct CallArgumentContext {
    pub(super) callee: String,
    pub(super) used_names: Vec<String>,
}

impl CompletionContext {
    #[must_use]
    pub const fn kind(&self) -> CompletionContextKind {
        self.kind
    }

    #[must_use]
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    #[must_use]
    pub const fn replace_range(&self) -> TextRange {
        self.replace_range
    }

    #[must_use]
    pub fn module_base(&self) -> Option<&str> {
        self.module_base.as_deref()
    }

    pub(super) fn global(prefix_start: usize, prefix: &str) -> Self {
        Self {
            kind: CompletionContextKind::Global,
            prefix: prefix.to_owned(),
            replace_range: TextRange::new(prefix_start, prefix_start + prefix.len()),
            module_base: None,
            member_receiver: None,
            record_constructor: None,
            map_key: None,
            call_arguments: None,
            lambda_parameter: None,
        }
    }

    pub(super) fn item(prefix_start: usize, prefix: &str) -> Self {
        Self {
            kind: CompletionContextKind::Item,
            prefix: prefix.to_owned(),
            replace_range: TextRange::new(prefix_start, prefix_start + prefix.len()),
            module_base: None,
            member_receiver: None,
            record_constructor: None,
            map_key: None,
            call_arguments: None,
            lambda_parameter: None,
        }
    }

    pub(super) fn expression(prefix_start: usize, prefix: &str) -> Self {
        Self {
            kind: CompletionContextKind::Expression,
            prefix: prefix.to_owned(),
            replace_range: TextRange::new(prefix_start, prefix_start + prefix.len()),
            module_base: None,
            member_receiver: None,
            record_constructor: None,
            map_key: None,
            call_arguments: None,
            lambda_parameter: None,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CompletionList {
    pub(super) context: CompletionContext,
    pub(super) items: Vec<CompletionItem>,
}

impl CompletionList {
    #[must_use]
    pub fn context(&self) -> &CompletionContext {
        &self.context
    }

    #[must_use]
    pub fn items(&self) -> &[CompletionItem] {
        &self.items
    }
}
