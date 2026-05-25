use std::collections::BTreeMap;

use vela_syntax::{
    BinaryOp, Block, ElseBranch, Expr, ExprKind, Literal, Param, StmtKind, TypeHint, UnaryOp,
};

use crate::{TypeFact, stdlib_function_fact, stdlib_method_fact};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ExprFactScope {
    paths: BTreeMap<Vec<String>, TypeFact>,
}

impl ExprFactScope {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_path(
        mut self,
        path: impl IntoIterator<Item = impl Into<String>>,
        fact: TypeFact,
    ) -> Self {
        self.insert_path(path, fact);
        self
    }

    pub fn insert_path(
        &mut self,
        path: impl IntoIterator<Item = impl Into<String>>,
        fact: TypeFact,
    ) {
        self.paths
            .insert(path.into_iter().map(Into::into).collect(), fact);
    }

    #[must_use]
    pub fn path_fact(&self, path: &[String]) -> Option<&TypeFact> {
        self.paths.get(path)
    }

    #[must_use]
    pub fn narrowed_by_condition(&self, condition: &Expr, truthy: bool) -> Self {
        let mut narrowed = self.clone();
        if let Some((path, expects_null)) = null_check(condition, truthy)
            && let Some(fact) = self.path_fact(path)
        {
            narrowed.paths.insert(
                path.to_vec(),
                if expects_null {
                    fact.only_null()
                } else {
                    fact.without_null()
                },
            );
        }
        narrowed
    }
}

pub fn type_fact_from_expr(expr: &Expr, scope: &ExprFactScope) -> TypeFact {
    match &expr.kind {
        ExprKind::Literal(literal) => literal_fact(literal),
        ExprKind::Path(path) => scope.path_fact(path).cloned().unwrap_or(TypeFact::Unknown),
        ExprKind::SelfValue => scope
            .path_fact(&["self".to_owned()])
            .cloned()
            .unwrap_or(TypeFact::Unknown),
        ExprKind::Unary { op, expr } => unary_fact(*op, type_fact_from_expr(expr, scope)),
        ExprKind::Binary { op, left, right } => binary_fact(
            *op,
            type_fact_from_expr(left, scope),
            type_fact_from_expr(right, scope),
        ),
        ExprKind::Assign { value, .. } | ExprKind::Try(value) => type_fact_from_expr(value, scope),
        ExprKind::Field { .. } | ExprKind::Index { .. } => TypeFact::Unknown,
        ExprKind::Call { callee, args } => call_fact(callee, args, scope),
        ExprKind::Array(values) => TypeFact::array(collection_fact(
            values.iter().map(|value| type_fact_from_expr(value, scope)),
        )),
        ExprKind::Map(entries) => {
            let key = collection_fact(entries.iter().map(|entry| map_key_fact(&entry.key, scope)));
            let value = collection_fact(
                entries
                    .iter()
                    .map(|entry| type_fact_from_expr(&entry.value, scope)),
            );
            TypeFact::map(key, value)
        }
        ExprKind::Record { path, .. } => TypeFact::record(path.join(".")),
        ExprKind::Lambda { params, body } => lambda_fact(params, body, scope, None),
        ExprKind::If(if_expr) => if_expr_fact(if_expr, scope),
        ExprKind::Match(match_expr) => TypeFact::union(
            match_expr
                .arms
                .iter()
                .map(|arm| type_fact_from_expr(&arm.body, scope)),
        ),
        ExprKind::Block(block) => block_fact(block, scope),
        ExprKind::Error => TypeFact::Unknown,
    }
}

fn null_check(condition: &Expr, truthy: bool) -> Option<(&[String], bool)> {
    let ExprKind::Binary { op, left, right } = &condition.kind else {
        return None;
    };
    let equality_expects_null = match op {
        BinaryOp::Equal => truthy,
        BinaryOp::NotEqual => !truthy,
        _ => return None,
    };

    if let Some(path) = path_if_null_check(left, right) {
        return Some((path, equality_expects_null));
    }
    path_if_null_check(right, left).map(|path| (path, equality_expects_null))
}

fn path_if_null_check<'a>(path_expr: &'a Expr, null_expr: &Expr) -> Option<&'a [String]> {
    let ExprKind::Path(path) = &path_expr.kind else {
        return None;
    };
    if matches!(null_expr.kind, ExprKind::Literal(Literal::Null)) {
        Some(path.as_slice())
    } else {
        None
    }
}

fn literal_fact(literal: &Literal) -> TypeFact {
    match literal {
        Literal::Null => TypeFact::Null,
        Literal::Bool(_) => TypeFact::Bool,
        Literal::Int(_) => TypeFact::Int,
        Literal::Float(_) => TypeFact::Float,
        Literal::String(_) => TypeFact::String,
    }
}

fn unary_fact(op: UnaryOp, operand: TypeFact) -> TypeFact {
    match op {
        UnaryOp::Not => TypeFact::Bool,
        UnaryOp::Negate => match operand {
            TypeFact::Int | TypeFact::Float => operand,
            _ => TypeFact::Union(vec![TypeFact::Int, TypeFact::Float]),
        },
    }
}

fn binary_fact(op: BinaryOp, left: TypeFact, right: TypeFact) -> TypeFact {
    match op {
        BinaryOp::Or
        | BinaryOp::And
        | BinaryOp::Equal
        | BinaryOp::NotEqual
        | BinaryOp::Less
        | BinaryOp::LessEqual
        | BinaryOp::Greater
        | BinaryOp::GreaterEqual => TypeFact::Bool,
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
            numeric_result([left, right])
        }
        BinaryOp::Range | BinaryOp::RangeInclusive => TypeFact::Unknown,
    }
}

fn numeric_result(values: impl IntoIterator<Item = TypeFact>) -> TypeFact {
    let values = values.into_iter().collect::<Vec<_>>();
    if values.iter().all(|value| matches!(value, TypeFact::Int)) {
        TypeFact::Int
    } else if values
        .iter()
        .all(|value| matches!(value, TypeFact::Int | TypeFact::Float))
    {
        TypeFact::Float
    } else {
        TypeFact::Union(vec![TypeFact::Int, TypeFact::Float])
    }
}

fn call_fact(callee: &Expr, args: &[vela_syntax::Argument], scope: &ExprFactScope) -> TypeFact {
    match &callee.kind {
        ExprKind::Path(path) => {
            let arg_facts = args
                .iter()
                .map(|arg| type_fact_from_expr(&arg.value, scope))
                .collect::<Vec<_>>();
            if let Some(fact) = stdlib_function_fact(&path.join("."), &arg_facts) {
                return fact.returns;
            }

            let Some((method, receiver_path)) = path.split_last() else {
                return TypeFact::Unknown;
            };
            let receiver = scope
                .path_fact(receiver_path)
                .cloned()
                .unwrap_or(TypeFact::Unknown);
            let lambda_return = args
                .first()
                .and_then(|arg| lambda_return_fact(&receiver, method, &arg.value, scope));
            stdlib_method_fact(&receiver, method, lambda_return.as_ref())
                .map_or(TypeFact::Unknown, |fact| fact.returns)
        }
        ExprKind::Field { base, name } => {
            let receiver = type_fact_from_expr(base, scope);
            let lambda_return = args
                .first()
                .and_then(|arg| lambda_return_fact(&receiver, name, &arg.value, scope));
            stdlib_method_fact(&receiver, name, lambda_return.as_ref())
                .map_or(TypeFact::Unknown, |fact| fact.returns)
        }
        _ => TypeFact::Unknown,
    }
}

fn map_key_fact(key: &Expr, scope: &ExprFactScope) -> TypeFact {
    match &key.kind {
        ExprKind::Literal(Literal::String(_))
        | ExprKind::Literal(Literal::Int(_))
        | ExprKind::Literal(Literal::Float(_))
        | ExprKind::Path(_) => TypeFact::String,
        _ => type_fact_from_expr(key, scope),
    }
}

fn lambda_return_fact(
    receiver: &TypeFact,
    method: &str,
    expr: &Expr,
    scope: &ExprFactScope,
) -> Option<TypeFact> {
    let ExprKind::Lambda { params, body } = &expr.kind else {
        return None;
    };
    let method_fact = stdlib_method_fact(receiver, method, None);
    let TypeFact::Function { returns, .. } = lambda_fact(
        params,
        body,
        scope,
        method_fact.and_then(|fact| fact.lambda.map(|lambda| lambda.params)),
    ) else {
        return None;
    };
    Some(*returns)
}

fn lambda_fact(
    params: &[Param],
    body: &Expr,
    scope: &ExprFactScope,
    inferred_params: Option<Vec<TypeFact>>,
) -> TypeFact {
    let mut nested = scope.clone();
    let mut param_facts = Vec::new();

    for (index, param) in params.iter().enumerate() {
        let fact = param
            .type_hint
            .as_ref()
            .map(type_fact_from_syntax_hint)
            .or_else(|| {
                inferred_params
                    .as_ref()
                    .and_then(|facts| facts.get(index).cloned())
            })
            .unwrap_or(TypeFact::Unknown);
        nested.insert_path([param.name.clone()], fact.clone());
        param_facts.push(fact);
    }

    let returns = type_fact_from_expr(body, &nested);
    TypeFact::function(param_facts, returns)
}

fn block_fact(block: &Block, scope: &ExprFactScope) -> TypeFact {
    block
        .statements
        .iter()
        .rev()
        .find_map(|statement| match &statement.kind {
            StmtKind::Return(Some(expr)) | StmtKind::Expr(expr) => {
                Some(type_fact_from_expr(expr, scope))
            }
            StmtKind::Block(block) => Some(block_fact(block, scope)),
            _ => None,
        })
        .unwrap_or(TypeFact::Null)
}

fn if_expr_fact(if_expr: &vela_syntax::IfExpr, scope: &ExprFactScope) -> TypeFact {
    let then_scope = scope.narrowed_by_condition(&if_expr.condition, true);
    let else_scope = scope.narrowed_by_condition(&if_expr.condition, false);
    let mut facts = vec![block_fact(&if_expr.then_branch, &then_scope)];
    if let Some(else_branch) = &if_expr.else_branch {
        facts.push(else_branch_fact(else_branch, &else_scope));
    }
    TypeFact::union(facts)
}

fn else_branch_fact(else_branch: &ElseBranch, scope: &ExprFactScope) -> TypeFact {
    match else_branch {
        ElseBranch::If(if_expr) => if_expr_fact(if_expr, scope),
        ElseBranch::Block(block) => block_fact(block, scope),
    }
}

fn collection_fact(facts: impl IntoIterator<Item = TypeFact>) -> TypeFact {
    TypeFact::union(facts)
}

fn type_fact_from_syntax_hint(hint: &TypeHint) -> TypeFact {
    match hint.path.as_slice() {
        [name] => match name.as_str() {
            "any" => TypeFact::Any,
            "null" => TypeFact::Null,
            "bool" => TypeFact::Bool,
            "int" => TypeFact::Int,
            "float" => TypeFact::Float,
            "string" => TypeFact::String,
            "array" => TypeFact::array(TypeFact::Unknown),
            "map" => TypeFact::map(TypeFact::Unknown, TypeFact::Unknown),
            "set" => TypeFact::set(TypeFact::Unknown),
            "function" => TypeFact::function(Vec::new(), TypeFact::Unknown),
            "Option" => TypeFact::option(TypeFact::Unknown),
            "Result" => TypeFact::result(TypeFact::Unknown, TypeFact::Unknown),
            name => TypeFact::record(name),
        },
        path => TypeFact::record(path.join(".")),
    }
}

#[cfg(test)]
mod tests {
    use vela_common::SourceId;
    use vela_syntax::{ItemKind, StmtKind, parse_source};

    use super::*;

    #[test]
    fn infers_literal_array_map_and_record_facts() {
        let expressions = function_exprs(
            r#"
            struct Reward { count: int }
            fn main() {
                let values = [1, 2, 3];
                let rewards = {"quest": 1, boss: 2.5, 10: 3};
                let reward = Reward { count: 3 };
            }
            "#,
        );
        let scope = ExprFactScope::new();

        assert_eq!(
            type_fact_from_expr(&expressions[0], &scope),
            TypeFact::array(TypeFact::Int)
        );
        assert_eq!(
            type_fact_from_expr(&expressions[1], &scope),
            TypeFact::map(
                TypeFact::String,
                TypeFact::Union(vec![TypeFact::Int, TypeFact::Float])
            )
        );
        assert_eq!(
            type_fact_from_expr(&expressions[2], &scope),
            TypeFact::record("Reward")
        );
    }

    #[test]
    fn infers_path_and_branch_facts_from_scope() {
        let expressions = function_exprs(
            r#"
            fn main() {
                if ok { score } else { "none" };
            }
            "#,
        );
        let scope = ExprFactScope::new()
            .with_path(["ok"], TypeFact::Bool)
            .with_path(["score"], TypeFact::Int);

        assert_eq!(
            type_fact_from_expr(&expressions[0], &scope),
            TypeFact::Union(vec![TypeFact::Int, TypeFact::String])
        );
    }

    #[test]
    fn narrows_null_checked_branch_facts() {
        let expressions = function_exprs(
            r#"
            fn main() {
                if player == null { 0 } else { player };
            }
            "#,
        );
        let scope = ExprFactScope::new().with_path(
            ["player"],
            TypeFact::Union(vec![TypeFact::Null, TypeFact::host("Player")]),
        );

        assert_eq!(
            type_fact_from_expr(&expressions[0], &scope),
            TypeFact::Union(vec![TypeFact::Int, TypeFact::host("Player")])
        );
    }

    #[test]
    fn infers_stdlib_method_facts_with_lambda_parameters() {
        let expressions = function_exprs(
            r#"
            fn main() {
                rewards.map(|reward| reward);
                rewards.find(|reward| reward);
                scores.sum(|score| score);
            }
            "#,
        );
        let scope = ExprFactScope::new()
            .with_path(["rewards"], TypeFact::array(TypeFact::record("Reward")))
            .with_path(["scores"], TypeFact::array(TypeFact::Int));

        assert_eq!(
            type_fact_from_expr(&expressions[0], &scope),
            TypeFact::array(TypeFact::record("Reward"))
        );
        assert_eq!(
            type_fact_from_expr(&expressions[1], &scope),
            TypeFact::option(TypeFact::record("Reward"))
        );
        assert_eq!(type_fact_from_expr(&expressions[2], &scope), TypeFact::Int);
    }

    #[test]
    fn infers_stdlib_function_facts() {
        let expressions = function_exprs(
            r#"
            fn main() {
                option.unwrap_or(maybe, 10);
                set.from_array(names);
            }
            "#,
        );
        let scope = ExprFactScope::new()
            .with_path(["maybe"], TypeFact::option(TypeFact::Int))
            .with_path(["names"], TypeFact::array(TypeFact::String));

        assert_eq!(type_fact_from_expr(&expressions[0], &scope), TypeFact::Int);
        assert_eq!(
            type_fact_from_expr(&expressions[1], &scope),
            TypeFact::set(TypeFact::String)
        );
    }

    fn function_exprs(source: &str) -> Vec<Expr> {
        let parsed = parse_source(SourceId::new(1), source);
        assert_eq!(parsed.diagnostics, []);
        let function = parsed
            .items
            .iter()
            .find_map(|item| match &item.kind {
                ItemKind::Function(function) => Some(function),
                _ => None,
            })
            .expect("function item");

        function
            .body
            .statements
            .iter()
            .filter_map(|statement| match &statement.kind {
                StmtKind::Expr(expr) => Some(expr.clone()),
                StmtKind::Let {
                    value: Some(expr), ..
                } => Some(expr.clone()),
                _ => None,
            })
            .collect()
    }
}
