use vela_analysis::{stdlib::stdlib_method_fact_with_lambda_arity, type_fact::TypeFact};

use crate::TextRange;

use super::{
    CompletionInsertFormat, CompletionItem, CompletionKind, MemberReceiver, label_segment_matches,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct LambdaParameterContext {
    pub(super) receiver: MemberReceiver,
    pub(super) method: String,
    pub(super) method_range: Option<TextRange>,
    pub(super) used_names: Vec<String>,
}

pub(super) fn lambda_parameter_completion_context(
    parameters: Option<&str>,
    shared_receiver: Option<MemberReceiver>,
    call_open: Option<usize>,
    shared_method: Option<(TextRange, &str)>,
) -> Option<LambdaParameterContext> {
    let params = parameters?;
    if !is_lambda_parameter_prefix(params) {
        return None;
    }
    let open_paren = call_open?;
    let receiver = shared_receiver?;
    let (method, method_range) = shared_method
        .filter(|(range, _)| range.end <= open_paren)
        .map(|(range, method)| (method.to_owned(), Some(range)))
        .unwrap_or_default();
    if method.is_empty() {
        return None;
    }
    Some(LambdaParameterContext {
        receiver,
        method,
        method_range,
        used_names: used_lambda_parameter_names(params),
    })
}

pub(super) fn lambda_parameter_completion_items(
    receiver_fact: &TypeFact,
    context: &LambdaParameterContext,
    prefix: &str,
) -> Vec<CompletionItem> {
    let Some(fact) =
        stdlib_method_fact_with_lambda_arity(receiver_fact, &context.method, None, None)
    else {
        return Vec::new();
    };
    let Some(lambda) = fact.lambda else {
        return Vec::new();
    };
    let used_names = context
        .used_names
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    lambda
        .params
        .iter()
        .enumerate()
        .filter_map(|(index, param)| {
            let label = lambda_parameter_name(&lambda.params, index).to_owned();
            if used_names.contains(&label.as_str()) || !label_segment_matches(&label, prefix) {
                return None;
            }
            Some(CompletionItem {
                label,
                kind: CompletionKind::Parameter,
                detail: param.display_name(),
                insert_text: None,
                insert_format: CompletionInsertFormat::PlainText,
                sort_text: None,
                metadata: Default::default(),
            })
        })
        .collect()
}

fn used_lambda_parameter_names(params: &str) -> Vec<String> {
    params
        .split(',')
        .map(str::trim)
        .filter(|param| is_identifier(param))
        .map(str::to_owned)
        .collect()
}

fn is_lambda_parameter_prefix(params: &str) -> bool {
    params
        .split(',')
        .all(|param| is_identifier_prefix(param.trim()))
}

fn is_identifier(value: &str) -> bool {
    !value.is_empty() && value.chars().all(is_identifier_continue)
}

fn is_identifier_prefix(value: &str) -> bool {
    value.chars().all(is_identifier_continue)
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn lambda_parameter_name(params: &[TypeFact], index: usize) -> &'static str {
    match (params.len(), index) {
        (1, 0) => "item",
        (2, 0) => "key",
        (2, 1) => "value",
        (_, 0) => "arg0",
        (_, 1) => "arg1",
        (_, 2) => "arg2",
        _ => "arg",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, Workspace,
        WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
        completion::{CompletionContextKind, CompletionList},
    };

    #[test]
    fn lambda_parameter_completion_suggests_stdlib_callback_item() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub fn main(scores: Array<i64>) { scores.filter(|) }";
        let databases = databases_for(document.clone(), text);
        let completions = databases.completion_items(
            &document,
            Position::new(0, text.find("|)").expect("lambda pipe") + "|".len()),
        );

        assert_eq!(
            completions.context().kind(),
            CompletionContextKind::LambdaParameter
        );
        let receiver = completions
            .context()
            .member_receiver_range()
            .expect("receiver range");
        let method = completions
            .context()
            .call_callee_range()
            .expect("method range");
        assert_eq!(&text[receiver.start..receiver.end], "scores");
        assert_eq!(&text[method.start..method.end], "filter");
        assert_completion(&completions, "item", "i64");
    }

    #[test]
    fn lambda_parameter_completion_suggests_map_key_and_value() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub fn main(scores: Map<String, i64>) { scores.filter(|) }";
        let databases = databases_for(document.clone(), text);
        let completions = databases.completion_items(
            &document,
            Position::new(0, text.find("|)").expect("lambda pipe") + "|".len()),
        );

        assert_eq!(
            completions.context().kind(),
            CompletionContextKind::LambdaParameter
        );
        assert_completion(&completions, "key", "String");
        assert_completion(&completions, "value", "i64");
    }

    #[test]
    fn lambda_parameter_completion_filters_prefix_and_used_names() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub fn main(scores: Map<String, i64>) { scores.filter(|key, va) }";
        let databases = databases_for(document.clone(), text);
        let completions = databases.completion_items(
            &document,
            Position::new(0, text.find("va)").expect("lambda prefix") + "va".len()),
        );

        assert_eq!(
            completions.context().kind(),
            CompletionContextKind::LambdaParameter
        );
        assert_no_completion(&completions, "key");
        assert_completion(&completions, "value", "i64");
    }

    fn databases_for(document: DocumentId, text: &str) -> LanguageServiceDatabases {
        let files = vec![SourceFileSnapshot::new(document, text)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);
        databases
    }

    fn assert_completion(list: &CompletionList, label: &str, detail: &str) {
        assert!(
            list.items().iter().any(|item| {
                item.label() == label
                    && item.kind() == CompletionKind::Parameter
                    && item.detail() == detail
            }),
            "{list:?}"
        );
    }

    fn assert_no_completion(list: &CompletionList, label: &str) {
        assert!(
            list.items().iter().all(|item| item.label() != label),
            "{list:?}"
        );
    }
}
