use vela_common::Span;

use crate::{DocumentId, TextRange};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SymbolRef {
    Source(String),
    Schema(String),
    Builtin(String),
    Local(LocalSymbolRef),
}

impl SymbolRef {
    #[must_use]
    pub fn local(name: impl Into<String>) -> Self {
        Self::Local(LocalSymbolRef::new(name))
    }

    #[must_use]
    pub fn local_at(name: impl Into<String>, document_id: DocumentId, range: TextRange) -> Self {
        Self::Local(LocalSymbolRef::with_location(name, document_id, range))
    }

    #[must_use]
    pub fn local_from_span(
        name: impl Into<String>,
        document_id: DocumentId,
        source_text: &str,
        span: Span,
    ) -> Self {
        let name = name.into();
        let Some(span_range) = span_text_range(span) else {
            return Self::local(name);
        };
        let Some(name_range) = name_range_in_text(source_text, span_range, &name) else {
            return Self::local(name);
        };
        Self::local_at(name, document_id, name_range)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LocalSymbolRef {
    name: String,
    document_id: Option<DocumentId>,
    range: Option<TextRange>,
}

impl LocalSymbolRef {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            document_id: None,
            range: None,
        }
    }

    #[must_use]
    pub fn with_location(
        name: impl Into<String>,
        document_id: DocumentId,
        range: TextRange,
    ) -> Self {
        Self {
            name: name.into(),
            document_id: Some(document_id),
            range: Some(range),
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn document_id(&self) -> Option<&DocumentId> {
        self.document_id.as_ref()
    }

    #[must_use]
    pub const fn range(&self) -> Option<TextRange> {
        self.range
    }
}

fn span_text_range(span: Span) -> Option<TextRange> {
    let start = usize::try_from(span.start).ok()?;
    let end = usize::try_from(span.end).ok()?;
    Some(TextRange::new(start, end))
}

fn name_range_in_text(text: &str, range: TextRange, name: &str) -> Option<TextRange> {
    let slice = text.get(range.start..range.end)?;
    let relative = slice.find(name)?;
    let start = range.start + relative;
    Some(TextRange::new(start, start + name.len()))
}
