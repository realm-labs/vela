use vela_syntax::ast::{AstNode, SyntaxSourceFile, SyntaxTupleFieldList};

/// Tuple variant entries are parameters (`name: Type`), not bare type hints.
pub(super) fn is_tuple_field_name(tree: &SyntaxSourceFile, offset: usize) -> bool {
    tree.syntax()
        .descendants()
        .filter_map(SyntaxTupleFieldList::cast)
        .any(|list| {
            list.params().any(|param| {
                param.name_token().is_some_and(|name| {
                    let range = name.text_range();
                    usize::from(range.start()) <= offset && offset <= usize::from(range.end())
                })
            })
        })
}
