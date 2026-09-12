use vela_analysis::type_fact::TypeFact;
use vela_syntax::ast::{AstNode, SyntaxExpressionKind};

use crate::callable_context::CallableOrigin;
use crate::{LanguageServiceDatabases, QueryContext, TextRange};

use super::{CompletionItem, CompletionKind, call_operand};

pub(super) fn adjust_items(
    databases: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    items: &mut Vec<CompletionItem>,
) {
    if !is_sync_callback(databases, query) {
        return;
    }
    items.retain_mut(|item| {
        if item.kind() != CompletionKind::Function {
            return true;
        }
        let Some(insertion) = item.insert_text() else {
            return true;
        };
        let path = insertion.strip_suffix("($0)").unwrap_or(insertion);
        let address = query
            .cursor()
            .module_base()
            .map_or_else(|| path.to_owned(), |base| format!("{base}::{path}"));
        let targets = query.callable_facts(databases, &address);
        let Some(target) = targets.first() else {
            return false;
        };
        if matches!(target.symbol(), crate::SymbolRef::Builtin(name) if name.starts_with("task::"))
            || item.symbol() != Some(target.symbol())
            || targets.iter().any(|candidate| {
                candidate.symbol() != target.symbol()
                    || candidate.asyncness().is_async()
                    || !matches!(
                        candidate.origin(),
                        CallableOrigin::Source | CallableOrigin::Schema | CallableOrigin::Stdlib
                    )
            })
        {
            return false;
        }
        call_operand::set_path_insertion(item, query, path.to_owned());
        true
    });
}

fn is_sync_callback(databases: &LanguageServiceDatabases, query: &QueryContext<'_>) -> bool {
    let Some(call) = call_operand::argument_call(query) else {
        return false;
    };
    let Some(field) = call.callee().and_then(|callee| callee.as_field()) else {
        return false;
    };
    let Some(receiver) = field.receiver() else {
        return false;
    };
    let Some(method) = field.name_text() else {
        return false;
    };
    let range = receiver.syntax().text_range();
    let offset = query.cursor().replace_range().end;
    let open = usize::from(
        call.l_paren_token()
            .expect("argument list")
            .text_range()
            .end(),
    );
    let Some(prefix) = query.text().get(open..offset) else {
        return false;
    };
    let mut callables = query
        .member_callable_facts(
            databases,
            TextRange::new(usize::from(range.start()), usize::from(range.end())),
            &method,
            prefix,
        )
        .into_iter();
    let Some(callable) = callables.next() else {
        return false;
    };
    if callables.next().is_some() || callable.origin() != CallableOrigin::StdlibMethod {
        return false;
    }
    let ordinal = call
        .separator_tokens()
        .iter()
        .filter(|token| usize::from(token.text_range().end()) <= offset)
        .count();
    let argument = call.arguments().get(ordinal).cloned();
    // Nested calls, lambda bodies and other expressions retain ordinary call
    // insertion; only the callback value's path slot gets a function reference.
    if argument
        .as_ref()
        .and_then(|arg| arg.expression())
        .is_some_and(|expr| expr.expression_kind() != SyntaxExpressionKind::Path)
    {
        return false;
    }
    let index = if let Some(name) = argument.and_then(|arg| arg.name_text()) {
        let Some(index) = callable
            .params()
            .iter()
            .position(|param| param.name() == name)
        else {
            return false;
        };
        index
    } else {
        ordinal
    };
    callable
        .params()
        .get(index)
        .is_some_and(|param| matches!(param.type_fact(), TypeFact::Function { .. }))
}
