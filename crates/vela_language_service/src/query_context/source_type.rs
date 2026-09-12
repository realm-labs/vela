use vela_common::Span;
use vela_hir::ids::HirDeclId;

use super::QueryContext;
use crate::{LanguageServiceDatabases, TextRange};

impl QueryContext<'_> {
    pub(crate) fn source_type_for_range(
        &self,
        databases: &LanguageServiceDatabases,
        range: TextRange,
    ) -> Option<HirDeclId> {
        let span = Span::new(
            self.source_id()?,
            u32::try_from(range.start).ok()?,
            u32::try_from(range.end).ok()?,
        );
        let expression = databases
            .hir_db()
            .graph()
            .expression_containing_span(span)?;
        databases
            .schema_analysis_facts()
            .script_type(expression)
            .map(|target| target.declaration)
    }
}
