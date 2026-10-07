//! Diagnose missing required children without discarding lossless recovery nodes.
use vela_common::{Diagnostic, SourceId, Span};

use crate::{
    SyntaxKind, SyntaxNode,
    ast::{
        AstNode, SyntaxConstItem, SyntaxFunctionItem, SyntaxImplItem, SyntaxImplMethod,
        SyntaxParam, SyntaxSourceFile, SyntaxStateItem, SyntaxStateStorage, SyntaxStructField,
        SyntaxTraitMethod,
    },
};

#[cfg(test)]
mod required_child_tests;
#[cfg(test)]
mod tests;

pub(super) fn validate(source: SourceId, tree: &SyntaxSourceFile) -> Vec<Diagnostic> {
    tree.syntax()
        .descendants()
        .flat_map(|node| {
            missing_parts(&node).into_iter().map(move |message| {
                let range = node.text_range();
                Diagnostic::error(message)
                    .with_code("E_PARSE")
                    .with_span(Span::new(source, range.start().into(), range.end().into()))
            })
        })
        .collect()
}

fn missing_parts(node: &SyntaxNode) -> Vec<&'static str> {
    let mut errors = Vec::new();
    macro_rules! item {
        ($ty:ty) => {
            <$ty>::cast(node.clone()).expect("matched syntax kind")
        };
    }
    match node.kind() {
        SyntaxKind::ConstItem => {
            let item = item!(SyntaxConstItem);
            if item.name_token().is_none() {
                errors.push("expected const name");
            }
            if item.value().is_none() {
                errors.push("expected const initializer");
            }
            if direct_token(node, SyntaxKind::Colon) && item.type_hint().is_none() {
                errors.push("expected type annotation");
            }
        }
        SyntaxKind::StateItem => {
            let item = item!(SyntaxStateItem);
            // The CST parser already diagnoses a wholly absent initializer.
            if item.storage() == SyntaxStateStorage::Vm
                && direct_token(node, SyntaxKind::Equal)
                && item.initializer().is_none()
            {
                errors.push("expected state initializer expression");
            }
        }
        SyntaxKind::FunctionItem => {
            let item = item!(SyntaxFunctionItem);
            // An unfinished parameter list already has its delimiter error;
            // diagnose the next required part only after the header is closed.
            if item.name_token().is_some()
                && item
                    .param_list()
                    .is_some_and(|params| params.r_paren_token().is_some())
                && item.body().is_none()
            {
                errors.push("expected function body");
            }
            if direct_token(node, SyntaxKind::Arrow) && item.return_type().is_none() {
                errors.push("expected return type");
            }
        }
        SyntaxKind::Param => {
            let item = item!(SyntaxParam);
            if item.name_token().is_none() {
                errors.push("expected parameter name");
            }
            if item.default_equal_token().is_some() && item.default_value().is_none() {
                errors.push("expected parameter default expression");
            }
            if direct_token(node, SyntaxKind::Colon) && item.type_hint().is_none() {
                errors.push("expected type annotation");
            }
        }
        SyntaxKind::StructField => {
            let item = item!(SyntaxStructField);
            if item.name_token().is_none() {
                errors.push("expected field name");
            }
            if direct_token(node, SyntaxKind::Equal) && item.default_value().is_none() {
                errors.push("expected field default expression");
            }
            if direct_token(node, SyntaxKind::Colon) && item.type_hint().is_none() {
                errors.push("expected type annotation");
            }
        }
        SyntaxKind::TraitMethod if item!(SyntaxTraitMethod).name_token().is_none() => {
            errors.push("expected method name");
        }
        SyntaxKind::ImplMethod if item!(SyntaxImplMethod).name_token().is_none() => {
            errors.push("expected method name");
        }
        SyntaxKind::ImplItem if item!(SyntaxImplItem).target_path_tokens().is_empty() => {
            errors.push("expected impl target");
        }
        _ => {}
    }
    errors
}

fn direct_token(node: &SyntaxNode, kind: SyntaxKind) -> bool {
    node.children_with_tokens()
        .filter_map(|element| element.into_token())
        .any(|token| token.kind() == kind)
}
