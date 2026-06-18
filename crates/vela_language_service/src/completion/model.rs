use vela_analysis::completion::CompletionKind as AnalysisCompletionKind;

use crate::{DisplayParts, TextRange};

use super::relevance::CompletionRelevance;
use super::{lambda_parameter::LambdaParameterContext, map_key::MapKeyContext};

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub enum CompletionKind {
    Keyword,
    Snippet,
    Binding,
    Value,
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
    pub(super) metadata: CompletionItemMetadata,
}

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct CompletionItemMetadata {
    pub(super) lookup: Option<String>,
    pub(super) edit_range: Option<TextRange>,
    pub(super) text_edit: Option<CompletionTextEdit>,
    pub(super) filter_text: Option<String>,
    pub(super) detail_parts: Option<DisplayParts>,
    pub(super) label_details: CompletionLabelDetails,
    pub(super) documentation: Option<String>,
    pub(super) relevance: CompletionRelevance,
    pub(super) deprecated: bool,
    pub(super) symbol: Option<CompletionSymbol>,
    pub(super) resolve: Option<CompletionResolvePayload>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CompletionTextEdit {
    pub(super) range: TextRange,
    pub(super) new_text: String,
}

impl CompletionTextEdit {
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }

    #[must_use]
    pub fn new_text(&self) -> &str {
        &self.new_text
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct CompletionLabelDetails {
    pub(super) detail: Option<String>,
    pub(super) description: Option<String>,
}

impl CompletionLabelDetails {
    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }

    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

pub use crate::symbol_ref::SymbolRef as CompletionSymbol;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CompletionResolvePayload {
    Documentation { symbol: CompletionSymbol },
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
    pub fn detail_parts(&self) -> DisplayParts {
        self.metadata
            .detail_parts
            .clone()
            .unwrap_or_else(|| DisplayParts::plain(self.detail.clone()))
    }

    #[must_use]
    pub fn insert_text(&self) -> Option<&str> {
        self.insert_text.as_deref()
    }

    #[must_use]
    pub fn lookup(&self) -> &str {
        self.metadata.lookup.as_deref().unwrap_or(&self.label)
    }

    #[must_use]
    pub fn filter_text(&self) -> &str {
        self.metadata
            .filter_text
            .as_deref()
            .unwrap_or_else(|| self.lookup())
    }

    #[must_use]
    pub fn edit_range(&self) -> Option<TextRange> {
        self.metadata.edit_range
    }

    #[must_use]
    pub fn text_edit(&self) -> Option<&CompletionTextEdit> {
        self.metadata.text_edit.as_ref()
    }

    #[must_use]
    pub fn label_details(&self) -> &CompletionLabelDetails {
        &self.metadata.label_details
    }

    #[must_use]
    pub fn documentation(&self) -> Option<&str> {
        self.metadata.documentation.as_deref()
    }

    #[must_use]
    pub const fn relevance(&self) -> CompletionRelevance {
        self.metadata.relevance
    }

    #[must_use]
    pub const fn deprecated(&self) -> bool {
        self.metadata.deprecated
    }

    #[must_use]
    pub fn symbol(&self) -> Option<&CompletionSymbol> {
        self.metadata.symbol.as_ref()
    }

    #[must_use]
    pub fn resolve_payload(&self) -> Option<&CompletionResolvePayload> {
        self.metadata.resolve.as_ref()
    }

    #[must_use]
    pub const fn insert_format(&self) -> CompletionInsertFormat {
        self.insert_format
    }

    #[must_use]
    pub fn sort_text(&self) -> Option<&str> {
        self.sort_text.as_deref()
    }

    #[must_use]
    pub(super) fn with_detail_parts(mut self, detail_parts: DisplayParts) -> Self {
        self.detail = detail_parts.render();
        self.metadata.detail_parts = Some(detail_parts);
        self
    }

    #[must_use]
    pub(super) fn with_symbol(mut self, symbol: CompletionSymbol) -> Self {
        self.metadata.resolve = Some(CompletionResolvePayload::Documentation {
            symbol: symbol.clone(),
        });
        self.metadata.symbol = Some(symbol);
        self
    }

    #[must_use]
    pub const fn with_deprecated(mut self, deprecated: bool) -> Self {
        self.metadata.deprecated = deprecated;
        self
    }

    #[must_use]
    pub fn with_resolve_payload(mut self, payload: CompletionResolvePayload) -> Self {
        self.metadata.resolve = Some(payload);
        self
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
    pub(super) callee_range: Option<TextRange>,
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

    #[must_use]
    pub fn member_receiver_range(&self) -> Option<TextRange> {
        self.member_receiver
            .as_ref()
            .map(|receiver| receiver.range)
            .or_else(|| {
                self.lambda_parameter
                    .as_ref()
                    .map(|context| context.receiver.range)
            })
    }

    #[must_use]
    pub fn call_callee_range(&self) -> Option<TextRange> {
        self.call_arguments
            .as_ref()
            .and_then(|context| context.callee_range)
            .or_else(|| {
                self.lambda_parameter
                    .as_ref()
                    .and_then(|context| context.method_range)
            })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_item_carries_deprecation_and_resolve_payload_metadata() {
        let item = CompletionItem {
            label: "old_value".to_owned(),
            kind: CompletionKind::Value,
            detail: "deprecated value".to_owned(),
            insert_text: None,
            insert_format: CompletionInsertFormat::PlainText,
            sort_text: None,
            metadata: CompletionItemMetadata::default(),
        }
        .with_deprecated(true)
        .with_resolve_payload(CompletionResolvePayload::Documentation {
            symbol: CompletionSymbol::Builtin("old_value".to_owned()),
        });

        assert!(item.deprecated());
        assert_eq!(
            item.resolve_payload(),
            Some(&CompletionResolvePayload::Documentation {
                symbol: CompletionSymbol::Builtin("old_value".to_owned())
            })
        );
    }
}
