use crate::{CursorContextKind, ModulePathRole, QueryContext, TextRange};

use super::{
    CompletionContext, CompletionContextKind,
    lambda_parameter::lambda_parameter_completion_context, map_key::map_key_at,
    model::MemberReceiver, named_argument::named_argument_completion_context,
    record_field::record_constructor_at,
};

pub(super) fn completion_context(query: &QueryContext<'_>) -> CompletionContext {
    let text = query.text();
    let cursor = query.cursor();
    let offset = cursor.replace_range().end;
    let prefix_start = cursor.replace_range().start;
    let prefix = cursor.prefix();

    let shared_lambda_receiver = query
        .member_receiver_range()
        .map(|range| MemberReceiver { range });
    let shared_lambda_method = query.lambda_method_range().zip(query.lambda_method_text());
    if let Some(lambda_parameter) = lambda_parameter_completion_context(
        query.lambda_parameters_text(),
        shared_lambda_receiver,
        query.call_open_offset(),
        shared_lambda_method,
    ) {
        return CompletionContext {
            kind: CompletionContextKind::LambdaParameter,
            prefix: prefix.to_owned(),
            replace_range: TextRange::new(prefix_start, offset),
            module_base: None,
            member_receiver: None,
            record_constructor: None,
            map_key: None,
            call_arguments: None,
            lambda_parameter: Some(lambda_parameter),
        };
    }

    if cursor.kind() == CursorContextKind::Type {
        return CompletionContext {
            kind: CompletionContextKind::TypeHint,
            prefix: prefix.to_owned(),
            replace_range: TextRange::new(prefix_start, offset),
            module_base: None,
            member_receiver: None,
            record_constructor: None,
            map_key: None,
            call_arguments: None,
            lambda_parameter: None,
        };
    }

    if cursor.kind() == CursorContextKind::RecordExpressionField
        && let Some(mut record_constructor) = query
            .parsed_source()
            .and_then(|source| record_constructor_at(source, offset))
    {
        record_constructor.current_module = query
            .module_path()
            .map(|module| module.segments().to_vec())
            .unwrap_or_default();
        return CompletionContext {
            kind: CompletionContextKind::RecordField,
            prefix: prefix.to_owned(),
            replace_range: TextRange::new(prefix_start, offset),
            module_base: None,
            member_receiver: None,
            record_constructor: Some(record_constructor),
            map_key: None,
            call_arguments: None,
            lambda_parameter: None,
        };
    }

    if cursor.kind() == CursorContextKind::MapKey
        && let Some(mut map_key) = query
            .parsed_source()
            .and_then(|source| map_key_at(source, offset))
    {
        map_key.current_module = query
            .module_path()
            .map(|module| module.segments().to_vec())
            .unwrap_or_default();
        return CompletionContext {
            kind: CompletionContextKind::MapKey,
            prefix: prefix.to_owned(),
            replace_range: TextRange::new(prefix_start, offset),
            module_base: None,
            member_receiver: None,
            record_constructor: None,
            map_key: Some(map_key),
            call_arguments: None,
            lambda_parameter: None,
        };
    }

    if cursor.kind() == CursorContextKind::Pattern {
        return CompletionContext {
            kind: CompletionContextKind::Pattern,
            prefix: prefix.to_owned(),
            replace_range: TextRange::new(prefix_start, offset),
            module_base: None,
            member_receiver: None,
            record_constructor: None,
            map_key: None,
            call_arguments: None,
            lambda_parameter: None,
        };
    }

    if cursor.kind() == CursorContextKind::MemberAccess {
        let member_receiver = query
            .member_receiver_range()
            .map(|range| MemberReceiver { range });
        return CompletionContext {
            kind: CompletionContextKind::Member,
            prefix: prefix.to_owned(),
            replace_range: TextRange::new(prefix_start, offset),
            module_base: None,
            member_receiver,
            record_constructor: None,
            map_key: None,
            call_arguments: None,
            lambda_parameter: None,
        };
    }

    if let Some(module_base) = cursor.module_base() {
        if cursor.module_path_role() == ModulePathRole::Type {
            return CompletionContext {
                kind: CompletionContextKind::TypeHint,
                prefix: prefix.to_owned(),
                replace_range: TextRange::new(prefix_start, offset),
                module_base: Some(module_base.to_owned()),
                member_receiver: None,
                record_constructor: None,
                map_key: None,
                call_arguments: None,
                lambda_parameter: None,
            };
        }
        return CompletionContext {
            kind: CompletionContextKind::ModulePath,
            prefix: prefix.to_owned(),
            replace_range: TextRange::new(prefix_start, offset),
            module_base: Some(module_base.to_owned()),
            member_receiver: None,
            record_constructor: None,
            map_key: None,
            call_arguments: None,
            lambda_parameter: None,
        };
    }

    if cursor.kind() == CursorContextKind::CallArgument
        && let Some(call_arguments) = named_argument_completion_context(
            text,
            query.cursor().replace_range().end,
            query.call_open_offset(),
            query_call_callee(query),
        )
    {
        return CompletionContext {
            kind: CompletionContextKind::NamedArgument,
            prefix: prefix.to_owned(),
            replace_range: TextRange::new(prefix_start, offset),
            module_base: None,
            member_receiver: None,
            record_constructor: None,
            map_key: None,
            call_arguments: Some(call_arguments),
            lambda_parameter: None,
        };
    }

    if cursor.kind() == CursorContextKind::Item {
        return CompletionContext::item(prefix_start, prefix);
    }

    if cursor.kind() == CursorContextKind::Statement {
        return CompletionContext {
            kind: CompletionContextKind::Statement,
            prefix: prefix.to_owned(),
            replace_range: TextRange::new(prefix_start, offset),
            module_base: None,
            member_receiver: None,
            record_constructor: None,
            map_key: None,
            call_arguments: None,
            lambda_parameter: None,
        };
    }

    if cursor.kind() == CursorContextKind::Expression {
        return CompletionContext::expression(prefix_start, prefix);
    }

    CompletionContext::expression(prefix_start, prefix)
}

fn query_call_callee<'a>(query: &'a QueryContext<'_>) -> Option<(TextRange, &'a str)> {
    Some((query.call_callee_range()?, query.call_callee_text()?))
}
