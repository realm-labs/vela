use super::{
    CompletionInsertFormat, CompletionItem, CompletionKind, relevance::completion_sort_text,
};

pub(super) fn item_keyword_completions(prefix: &str) -> Vec<CompletionItem> {
    [
        (
            "fn",
            "function declaration",
            "fn ${1:name}(${2:params}) {\n    $0\n}",
        ),
        (
            "struct",
            "struct declaration",
            "struct ${1:Name} {\n    $0\n}",
        ),
        ("enum", "enum declaration", "enum ${1:Name} {\n    $0\n}"),
        ("trait", "trait declaration", "trait ${1:Name} {\n    $0\n}"),
        (
            "impl",
            "implementation block",
            "impl ${1:Type} {\n    $0\n}",
        ),
        ("use", "import declaration", "use $0"),
        ("const", "constant declaration", "const ${1:NAME} = $0"),
        (
            "global",
            "host-bound global declaration",
            "global ${1:name}: ${2:Type}",
        ),
        ("pub", "public visibility", "pub $0"),
    ]
    .into_iter()
    .filter(|(label, _, _)| label.starts_with(prefix))
    .map(|(label, detail, insert_text)| CompletionItem {
        label: label.to_owned(),
        kind: CompletionKind::Keyword,
        detail: detail.to_owned(),
        insert_text: Some(insert_text.to_owned()),
        insert_format: CompletionInsertFormat::Snippet,
        sort_text: Some(completion_sort_text(CompletionKind::Keyword, label, "")),
        metadata: Default::default(),
    })
    .collect()
}
