use std::collections::BTreeSet;

use vela_syntax::ast::{
    AstNode, SyntaxBlock, SyntaxConstItem, SyntaxExpression, SyntaxExpressionKind,
    SyntaxFunctionItem, SyntaxImplItem, SyntaxImplMethod, SyntaxLambdaBody, SyntaxMatchArm,
    SyntaxMatchArmBody, SyntaxParamList, SyntaxSourceFile, SyntaxStatement, SyntaxStatementKind,
    SyntaxTraitItem, SyntaxTraitMethod,
};
use vela_syntax::{SyntaxKind, TextRange as SyntaxTextRange};

use crate::{DocumentId, LanguageServiceDatabases, LineIndex, Position, SourceRecord};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub enum FoldingRangeKind {
    Imports,
    Region,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct FoldingRange {
    kind: FoldingRangeKind,
    start: Position,
    end: Position,
}

impl FoldingRange {
    #[must_use]
    pub const fn new(kind: FoldingRangeKind, start: Position, end: Position) -> Self {
        Self { kind, start, end }
    }

    #[must_use]
    pub const fn kind(self) -> FoldingRangeKind {
        self.kind
    }

    #[must_use]
    pub const fn start(self) -> Position {
        self.start
    }

    #[must_use]
    pub const fn end(self) -> Position {
        self.end
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
struct FoldingRangeKey {
    kind: FoldingRangeKind,
    start_line: usize,
    start_character: usize,
    end_line: usize,
    end_character: usize,
}

impl FoldingRangeKey {
    const fn new(kind: FoldingRangeKind, start: Position, end: Position) -> Self {
        Self {
            kind,
            start_line: start.line,
            start_character: start.character,
            end_line: end.line,
            end_character: end.character,
        }
    }

    const fn into_range(self) -> FoldingRange {
        FoldingRange::new(
            self.kind,
            Position::new(self.start_line, self.start_character),
            Position::new(self.end_line, self.end_character),
        )
    }
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn folding_ranges(&self, document_id: &DocumentId) -> Vec<FoldingRange> {
        let Some(source) = self.source_db().records().get(document_id) else {
            return Vec::new();
        };
        let Some(parsed) = self.parse_db().syntax_parse(document_id) else {
            return Vec::new();
        };

        let line_index = LineIndex::new(source.text());
        let mut ranges = BTreeSet::new();
        let tree = parsed.tree();
        collect_import_groups(&tree, source, &line_index, &mut ranges);
        for item in tree.items() {
            collect_item_ranges(&item, source, &line_index, &mut ranges);
        }
        ranges
            .into_iter()
            .map(FoldingRangeKey::into_range)
            .collect()
    }
}

fn collect_import_groups(
    tree: &SyntaxSourceFile,
    source: &SourceRecord,
    line_index: &LineIndex,
    ranges: &mut BTreeSet<FoldingRangeKey>,
) {
    let mut group_start: Option<SyntaxTextRange> = None;
    let mut group_end: Option<SyntaxTextRange> = None;

    for item in tree.items() {
        if item.syntax().kind() == SyntaxKind::UseItem {
            group_start.get_or_insert(item.syntax().text_range());
            group_end = Some(item.syntax().text_range());
        } else {
            push_import_group(
                group_start.take(),
                group_end.take(),
                source,
                line_index,
                ranges,
            );
        }
    }

    push_import_group(group_start, group_end, source, line_index, ranges);
}

fn push_import_group(
    start: Option<SyntaxTextRange>,
    end: Option<SyntaxTextRange>,
    source: &SourceRecord,
    line_index: &LineIndex,
    ranges: &mut BTreeSet<FoldingRangeKey>,
) {
    let (Some(start), Some(end)) = (start, end) else {
        return;
    };
    push_syntax_range(
        FoldingRangeKind::Imports,
        range_from_bounds(start, end),
        source,
        line_index,
        ranges,
    );
}

fn collect_item_ranges(
    item: &vela_syntax::ast::SyntaxItem,
    source: &SourceRecord,
    line_index: &LineIndex,
    ranges: &mut BTreeSet<FoldingRangeKey>,
) {
    match item.syntax().kind() {
        SyntaxKind::UseItem | SyntaxKind::GlobalItem => {}
        SyntaxKind::ConstItem => {
            if let Some(item) = SyntaxConstItem::cast(item.syntax().clone())
                && let Some(value) = item.value()
            {
                collect_expr_ranges(&value, source, line_index, ranges);
            }
        }
        SyntaxKind::FunctionItem => {
            push_syntax_range(
                FoldingRangeKind::Region,
                item.syntax().text_range(),
                source,
                line_index,
                ranges,
            );
            if let Some(function) = SyntaxFunctionItem::cast(item.syntax().clone()) {
                collect_function_ranges(&function, source, line_index, ranges);
            }
        }
        SyntaxKind::StructItem | SyntaxKind::EnumItem => {
            push_syntax_range(
                FoldingRangeKind::Region,
                item.syntax().text_range(),
                source,
                line_index,
                ranges,
            );
        }
        SyntaxKind::TraitItem => {
            push_syntax_range(
                FoldingRangeKind::Region,
                item.syntax().text_range(),
                source,
                line_index,
                ranges,
            );
            if let Some(trait_item) = SyntaxTraitItem::cast(item.syntax().clone()) {
                for method in trait_item.methods() {
                    collect_trait_method_ranges(&method, source, line_index, ranges);
                }
            }
        }
        SyntaxKind::ImplItem => {
            push_syntax_range(
                FoldingRangeKind::Region,
                item.syntax().text_range(),
                source,
                line_index,
                ranges,
            );
            if let Some(impl_item) = SyntaxImplItem::cast(item.syntax().clone()) {
                for method in impl_item.methods() {
                    collect_impl_method_ranges(&method, source, line_index, ranges);
                }
            }
        }
        _ => {}
    }
}

fn collect_function_ranges(
    function: &SyntaxFunctionItem,
    source: &SourceRecord,
    line_index: &LineIndex,
    ranges: &mut BTreeSet<FoldingRangeKey>,
) {
    if let Some(body) = function.body() {
        collect_block_ranges(&body, source, line_index, ranges);
    }
    collect_param_default_ranges(function.param_list(), source, line_index, ranges);
}

fn collect_trait_method_ranges(
    method: &SyntaxTraitMethod,
    source: &SourceRecord,
    line_index: &LineIndex,
    ranges: &mut BTreeSet<FoldingRangeKey>,
) {
    if let Some(body) = method.body() {
        collect_block_ranges(&body, source, line_index, ranges);
    }
    collect_param_default_ranges(method.param_list(), source, line_index, ranges);
}

fn collect_impl_method_ranges(
    method: &SyntaxImplMethod,
    source: &SourceRecord,
    line_index: &LineIndex,
    ranges: &mut BTreeSet<FoldingRangeKey>,
) {
    if let Some(body) = method.body() {
        collect_block_ranges(&body, source, line_index, ranges);
    }
    collect_param_default_ranges(method.param_list(), source, line_index, ranges);
}

fn collect_param_default_ranges(
    params: Option<SyntaxParamList>,
    source: &SourceRecord,
    line_index: &LineIndex,
    ranges: &mut BTreeSet<FoldingRangeKey>,
) {
    if let Some(params) = params {
        for param in params.params() {
            if let Some(default_value) = param.default_value() {
                collect_expr_ranges(&default_value, source, line_index, ranges);
            }
        }
    }
}

fn collect_block_ranges(
    block: &SyntaxBlock,
    source: &SourceRecord,
    line_index: &LineIndex,
    ranges: &mut BTreeSet<FoldingRangeKey>,
) {
    push_syntax_range(
        FoldingRangeKind::Region,
        block.syntax().text_range(),
        source,
        line_index,
        ranges,
    );
    for statement in block.statements() {
        collect_stmt_ranges(&statement, source, line_index, ranges);
    }
}

fn collect_stmt_ranges(
    statement: &SyntaxStatement,
    source: &SourceRecord,
    line_index: &LineIndex,
    ranges: &mut BTreeSet<FoldingRangeKey>,
) {
    match statement.statement_kind() {
        SyntaxStatementKind::Let => {
            if let Some(let_stmt) = statement.as_let()
                && let Some(value) = let_stmt.initializer()
            {
                collect_expr_ranges(&value, source, line_index, ranges);
            }
        }
        SyntaxStatementKind::Return => {
            if let Some(return_stmt) = statement.as_return()
                && let Some(value) = return_stmt.expression()
            {
                collect_expr_ranges(&value, source, line_index, ranges);
            }
        }
        SyntaxStatementKind::Break | SyntaxStatementKind::Continue => {}
        SyntaxStatementKind::For => {
            if let Some(for_stmt) = statement.as_for() {
                if let Some(iterable) = for_stmt.iterable() {
                    collect_expr_ranges(&iterable, source, line_index, ranges);
                }
                if let Some(body) = for_stmt.body() {
                    collect_block_ranges(&body, source, line_index, ranges);
                }
            }
        }
        SyntaxStatementKind::If => {
            if let Some(if_expr) = statement.as_if()
                && let Some(expr) = SyntaxExpression::cast(if_expr.syntax().clone())
            {
                collect_expr_ranges(&expr, source, line_index, ranges);
            }
        }
        SyntaxStatementKind::Match => {
            if let Some(match_expr) = statement.as_match()
                && let Some(expr) = SyntaxExpression::cast(match_expr.syntax().clone())
            {
                collect_expr_ranges(&expr, source, line_index, ranges);
            }
        }
        SyntaxStatementKind::Expr => {
            if let Some(expr_stmt) = statement.as_expr()
                && let Some(value) = expr_stmt.expression()
            {
                collect_expr_ranges(&value, source, line_index, ranges);
            }
        }
        SyntaxStatementKind::Block => {
            if let Some(block) = statement.as_block() {
                collect_block_ranges(&block, source, line_index, ranges);
            }
        }
    }
}

fn collect_expr_ranges(
    expr: &SyntaxExpression,
    source: &SourceRecord,
    line_index: &LineIndex,
    ranges: &mut BTreeSet<FoldingRangeKey>,
) {
    match expr.expression_kind() {
        SyntaxExpressionKind::Literal => {
            push_multiline_expression(expr, source, line_index, ranges);
            if let Some(literal) = expr.as_literal() {
                for value in literal.interpolation_expressions() {
                    collect_expr_ranges(&value, source, line_index, ranges);
                }
            }
        }
        SyntaxExpressionKind::Path => {}
        SyntaxExpressionKind::Paren => {
            if let Some(paren) = expr.as_paren()
                && let Some(value) = paren.expression()
            {
                collect_expr_ranges(&value, source, line_index, ranges);
            }
        }
        SyntaxExpressionKind::Unary => {
            if let Some(unary) = expr.as_unary()
                && let Some(value) = unary.expression()
            {
                collect_expr_ranges(&value, source, line_index, ranges);
            }
        }
        SyntaxExpressionKind::Try => {
            if let Some(try_expr) = expr.as_try()
                && let Some(value) = try_expr.expression()
            {
                collect_expr_ranges(&value, source, line_index, ranges);
            }
        }
        SyntaxExpressionKind::Binary => {
            if let Some(binary) = expr.as_binary() {
                if let Some(lhs) = binary.lhs() {
                    collect_expr_ranges(&lhs, source, line_index, ranges);
                }
                if let Some(rhs) = binary.rhs() {
                    collect_expr_ranges(&rhs, source, line_index, ranges);
                }
            }
        }
        SyntaxExpressionKind::Assign => {
            if let Some(assign) = expr.as_assign() {
                if let Some(target) = assign.target() {
                    collect_expr_ranges(&target, source, line_index, ranges);
                }
                if let Some(value) = assign.value() {
                    collect_expr_ranges(&value, source, line_index, ranges);
                }
            }
        }
        SyntaxExpressionKind::Field => {
            if let Some(field) = expr.as_field()
                && let Some(receiver) = field.receiver()
            {
                collect_expr_ranges(&receiver, source, line_index, ranges);
            }
        }
        SyntaxExpressionKind::Call => {
            if let Some(call) = expr.as_call() {
                if let Some(callee) = call.callee() {
                    collect_expr_ranges(&callee, source, line_index, ranges);
                }
                for argument in call.arguments() {
                    if let Some(value) = argument.expression() {
                        collect_expr_ranges(&value, source, line_index, ranges);
                    }
                }
            }
        }
        SyntaxExpressionKind::Index => {
            if let Some(index) = expr.as_index() {
                if let Some(receiver) = index.receiver() {
                    collect_expr_ranges(&receiver, source, line_index, ranges);
                }
                if let Some(value) = index.index() {
                    collect_expr_ranges(&value, source, line_index, ranges);
                }
            }
        }
        SyntaxExpressionKind::Array => {
            push_multiline_expression(expr, source, line_index, ranges);
            if let Some(array) = expr.as_array() {
                for item in array.expressions() {
                    collect_expr_ranges(&item, source, line_index, ranges);
                }
            }
        }
        SyntaxExpressionKind::Map => {
            push_multiline_expression(expr, source, line_index, ranges);
            if let Some(map) = expr.as_map() {
                for entry in map.entries() {
                    if let Some(key) = entry.key() {
                        collect_expr_ranges(&key, source, line_index, ranges);
                    }
                    if let Some(value) = entry.value() {
                        collect_expr_ranges(&value, source, line_index, ranges);
                    }
                }
            }
        }
        SyntaxExpressionKind::Record => {
            push_multiline_expression(expr, source, line_index, ranges);
            if let Some(record) = expr.as_record() {
                for field in record.fields() {
                    if let Some(value) = field.expression() {
                        collect_expr_ranges(&value, source, line_index, ranges);
                    }
                }
            }
        }
        SyntaxExpressionKind::Lambda => {
            push_multiline_expression(expr, source, line_index, ranges);
            if let Some(lambda) = expr.as_lambda() {
                collect_param_default_ranges(lambda.param_list(), source, line_index, ranges);
                match lambda.body() {
                    Some(SyntaxLambdaBody::Expression(value)) => {
                        collect_expr_ranges(&value, source, line_index, ranges);
                    }
                    Some(SyntaxLambdaBody::Block(block)) => {
                        collect_block_ranges(&block, source, line_index, ranges);
                    }
                    None => {}
                }
            }
        }
        SyntaxExpressionKind::If => {
            if let Some(if_expr) = expr.as_if() {
                if let Some(condition) = if_expr.condition() {
                    collect_expr_ranges(&condition, source, line_index, ranges);
                }
                if let Some(then_block) = if_expr.then_block() {
                    collect_block_ranges(&then_block, source, line_index, ranges);
                }
                if let Some(else_value) = if_expr.else_as_expression() {
                    collect_expr_ranges(&else_value, source, line_index, ranges);
                }
            }
        }
        SyntaxExpressionKind::Match => {
            push_syntax_range(
                FoldingRangeKind::Region,
                expr.syntax().text_range(),
                source,
                line_index,
                ranges,
            );
            if let Some(match_expr) = expr.as_match() {
                if let Some(scrutinee) = match_expr.scrutinee() {
                    collect_expr_ranges(&scrutinee, source, line_index, ranges);
                }
                for arm in match_expr.arms() {
                    collect_match_arm_ranges(&arm, source, line_index, ranges);
                }
            }
        }
        SyntaxExpressionKind::Block => {
            if let Some(block) = expr.as_block() {
                collect_block_ranges(&block, source, line_index, ranges);
            }
        }
    }
}

fn collect_match_arm_ranges(
    arm: &SyntaxMatchArm,
    source: &SourceRecord,
    line_index: &LineIndex,
    ranges: &mut BTreeSet<FoldingRangeKey>,
) {
    if let Some(guard) = arm.guard() {
        collect_expr_ranges(&guard, source, line_index, ranges);
    }
    match arm.body() {
        Some(SyntaxMatchArmBody::Expression(value)) => {
            push_syntax_range(
                FoldingRangeKind::Region,
                value.syntax().text_range(),
                source,
                line_index,
                ranges,
            );
            collect_expr_ranges(&value, source, line_index, ranges);
        }
        Some(SyntaxMatchArmBody::Block(block)) => {
            push_syntax_range(
                FoldingRangeKind::Region,
                block.syntax().text_range(),
                source,
                line_index,
                ranges,
            );
            collect_block_ranges(&block, source, line_index, ranges);
        }
        None => {}
    }
}

fn push_multiline_expression(
    expr: &SyntaxExpression,
    source: &SourceRecord,
    line_index: &LineIndex,
    ranges: &mut BTreeSet<FoldingRangeKey>,
) {
    if is_multiline_range(expr.syntax().text_range(), line_index) {
        push_syntax_range(
            FoldingRangeKind::Region,
            expr.syntax().text_range(),
            source,
            line_index,
            ranges,
        );
    }
}

fn push_syntax_range(
    kind: FoldingRangeKind,
    range: SyntaxTextRange,
    _source: &SourceRecord,
    line_index: &LineIndex,
    ranges: &mut BTreeSet<FoldingRangeKey>,
) {
    let start = line_index.position(text_size_to_usize(range.start()));
    let end = line_index.position(text_size_to_usize(range.end()));
    if start.line < end.line {
        ranges.insert(FoldingRangeKey::new(kind, start, end));
    }
}

fn is_multiline_range(range: SyntaxTextRange, line_index: &LineIndex) -> bool {
    line_index.position(text_size_to_usize(range.start())).line
        < line_index.position(text_size_to_usize(range.end())).line
}

fn range_from_bounds(start: SyntaxTextRange, end: SyntaxTextRange) -> SyntaxTextRange {
    SyntaxTextRange::new(start.start(), end.end())
}

fn text_size_to_usize(size: vela_syntax::TextSize) -> usize {
    u32::from(size) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
    };

    #[test]
    fn folding_ranges_cover_items_and_blocks() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "\
use game::reward::grant
use game::reward::Reward

pub struct Player {
    level: i64
}

pub fn main(player: Player) -> i64 {
    if player.level > 1 {
        return match player.level {
            1 => {
                return 1
            }
            _ => {
                return 2
            }
        }
    }
    return 0
}";
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

        let ranges = databases.folding_ranges(&document);

        assert!(
            ranges
                .iter()
                .any(|range| range.kind() == FoldingRangeKind::Imports
                    && range.start().line == 0
                    && range.end().line == 1),
            "{ranges:?}"
        );
        assert!(
            ranges
                .iter()
                .any(|range| range.kind() == FoldingRangeKind::Region
                    && range.start().line == 3
                    && range.end().line == 5),
            "{ranges:?}"
        );
        assert!(
            ranges
                .iter()
                .any(|range| range.start().line == 7 && range.end().line == 19),
            "{ranges:?}"
        );
        assert!(
            ranges
                .iter()
                .any(|range| range.start().line == 9 && range.end().line == 16),
            "{ranges:?}"
        );
        assert!(
            ranges
                .iter()
                .any(|range| range.start().line == 10 && range.end().line == 12),
            "{ranges:?}"
        );
        assert!(
            ranges
                .iter()
                .any(|range| range.start().line == 13 && range.end().line == 15),
            "{ranges:?}"
        );
    }

    #[test]
    fn folding_ranges_cover_multiline_literals_under_parser_recovery() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "\
pub fn main() -> i64 {
    let scores = [
        1,
        2
    ]
    let rewards = {
        \"gold\": 1,
        \"xp\": 2
    }
    let label = \"\"\"
daily
quest
\"\"\"
    return scores[0]
";
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

        let ranges = databases.folding_ranges(&document);

        assert!(
            ranges
                .iter()
                .any(|range| range.start().line == 1 && range.end().line == 4),
            "{ranges:?}"
        );
        assert!(
            ranges
                .iter()
                .any(|range| range.start().line == 5 && range.end().line == 8),
            "{ranges:?}"
        );
        assert!(
            ranges
                .iter()
                .any(|range| range.start().line == 9 && range.end().line == 12),
            "{ranges:?}"
        );
    }

    fn databases_for(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);
        databases
    }
}
