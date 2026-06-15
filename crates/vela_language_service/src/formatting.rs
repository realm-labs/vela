use crate::{DiagnosticRange, DocumentId, LanguageServiceDatabases, LineIndex, Position, TextEdit};

impl LanguageServiceDatabases {
    #[must_use]
    pub fn document_formatting(&self, document_id: &DocumentId) -> Vec<TextEdit> {
        let Some(source) = self.source_db().records().get(document_id) else {
            return Vec::new();
        };
        let formatted = format_document(source.text());
        if formatted == source.text() {
            return Vec::new();
        }

        vec![TextEdit::new(
            DiagnosticRange::new(
                Position::new(0, 0),
                LineIndex::new(source.text()).position(source.text().len()),
            ),
            formatted,
        )]
    }
}

fn format_document(source: &str) -> String {
    if source.is_empty() {
        return String::new();
    }

    let mut formatted = String::with_capacity(source.len().saturating_add(1));
    let mut saw_trailing_newline = false;
    for line in source.split_inclusive('\n') {
        saw_trailing_newline = line.ends_with('\n');
        let line = line.strip_suffix('\n').unwrap_or(line);
        let (body, ending) = line
            .strip_suffix('\r')
            .map_or((line, ""), |body| (body, "\r"));
        formatted.push_str(body.trim_end_matches([' ', '\t']));
        formatted.push_str(ending);
        if saw_trailing_newline {
            formatted.push('\n');
        }
    }

    if !saw_trailing_newline {
        formatted.push('\n');
    }
    formatted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
    };

    fn format_source(source: &str) -> Vec<TextEdit> {
        let document_id = DocumentId::from("file:///workspace/scripts/main.vela");
        let config = WorkspaceConfig::workspace([WorkspaceRoot::new("/workspace/scripts")]);
        let files = vec![SourceFileSnapshot::new(document_id.clone(), source)];
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);

        databases.document_formatting(&document_id)
    }

    fn apply_edits(source: &str, edits: &[TextEdit]) -> String {
        if edits.is_empty() {
            return source.to_owned();
        }
        assert_eq!(edits.len(), 1);
        edits[0].new_text().to_owned()
    }

    #[test]
    fn formatting_preserves_comments() {
        let source = "// keep this comment   \npub fn main() { // inline\t\n    return 1   \n}";
        let edits = format_source(source);
        let formatted = apply_edits(source, &edits);

        assert_eq!(
            formatted,
            "// keep this comment\npub fn main() { // inline\n    return 1\n}\n"
        );
    }

    #[test]
    fn formatting_is_idempotent() {
        let source = "pub fn main() {\n    return 1\n}\n";
        let edits = format_source(source);

        assert!(edits.is_empty());
    }

    #[test]
    fn formatting_handles_malformed_source_without_panic() {
        let source = "pub fn main( {   ";
        let edits = format_source(source);
        let formatted = apply_edits(source, &edits);

        assert_eq!(formatted, "pub fn main( {\n");
    }
}
