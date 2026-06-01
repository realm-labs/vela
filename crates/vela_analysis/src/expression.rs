mod condition_narrowing;
mod match_narrowing;
mod try_propagation;

use std::collections::BTreeMap;

use vela_syntax::ast::{
    BinaryOp, Block, ElseBranch, Expr, ExprKind, Literal, Param, Pattern, StmtKind, TypeHint,
    UnaryOp,
};

use crate::registry::RegistryFacts;
use crate::stdlib::{stdlib_function_fact, stdlib_method_fact};
use crate::type_fact::TypeFact;

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
        condition_narrowing::narrowed_by_condition(self, condition, truthy)
    }

    #[must_use]
    pub fn narrowed_by_match_pattern(
        &self,
        scrutinee: &Expr,
        pattern: &Pattern,
        facts: &RegistryFacts,
    ) -> Self {
        match_narrowing::narrowed_by_match_pattern(self, scrutinee, pattern, facts)
    }
}

pub fn type_fact_from_expr(expr: &Expr, scope: &ExprFactScope) -> TypeFact {
    type_fact_from_expr_impl(expr, scope, None)
}

pub fn type_fact_from_expr_with_registry(
    expr: &Expr,
    scope: &ExprFactScope,
    facts: &RegistryFacts,
) -> TypeFact {
    type_fact_from_expr_impl(expr, scope, Some(facts))
}

fn type_fact_from_expr_impl(
    expr: &Expr,
    scope: &ExprFactScope,
    facts: Option<&RegistryFacts>,
) -> TypeFact {
    match &expr.kind {
        ExprKind::Literal(literal) => literal_fact(literal),
        ExprKind::Path(path) => scope
            .path_fact(path)
            .cloned()
            .or_else(|| path_field_fact(path, scope, facts))
            .unwrap_or(TypeFact::Unknown),
        ExprKind::SelfValue => scope
            .path_fact(&["self".to_owned()])
            .cloned()
            .unwrap_or(TypeFact::Unknown),
        ExprKind::Unary { op, expr } => {
            unary_fact(*op, type_fact_from_expr_impl(expr, scope, facts))
        }
        ExprKind::Binary { op, left, right } => binary_fact(
            *op,
            type_fact_from_expr_impl(left, scope, facts),
            type_fact_from_expr_impl(right, scope, facts),
        ),
        ExprKind::Assign { value, .. } => type_fact_from_expr_impl(value, scope, facts),
        ExprKind::Try(value) => {
            try_propagation::try_fact(type_fact_from_expr_impl(value, scope, facts))
        }
        ExprKind::Field { base, name } => field_access_fact(base, name, scope, facts),
        ExprKind::Index { base, index } => index_fact(
            type_fact_from_expr_impl(base, scope, facts),
            type_fact_from_expr_impl(index, scope, facts),
        ),
        ExprKind::Call { callee, args } => call_fact(callee, args, scope, facts),
        ExprKind::Array(values) => TypeFact::array(collection_fact(
            values
                .iter()
                .map(|value| type_fact_from_expr_impl(value, scope, facts)),
        )),
        ExprKind::Map(entries) => {
            let key = collection_fact(
                entries
                    .iter()
                    .map(|entry| map_key_fact(&entry.key, scope, facts)),
            );
            let value = collection_fact(
                entries
                    .iter()
                    .map(|entry| type_fact_from_expr_impl(&entry.value, scope, facts)),
            );
            TypeFact::map(key, value)
        }
        ExprKind::Record { path, .. } => TypeFact::record(path.join(".")),
        ExprKind::Lambda { params, body } => lambda_fact(params, body, scope, facts, None),
        ExprKind::If(if_expr) => if_expr_fact(if_expr, scope, facts),
        ExprKind::Match(match_expr) => TypeFact::union(match_expr.arms.iter().map(|arm| {
            let arm_scope = facts.map_or_else(
                || scope.clone(),
                |facts| scope.narrowed_by_match_pattern(&match_expr.scrutinee, &arm.pattern, facts),
            );
            type_fact_from_expr_impl(&arm.body, &arm_scope, facts)
        })),
        ExprKind::Block(block) => block_fact(block, scope, facts),
        ExprKind::Error => TypeFact::Unknown,
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
        BinaryOp::Range | BinaryOp::RangeInclusive => TypeFact::Range,
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

fn call_fact(
    callee: &Expr,
    args: &[vela_syntax::ast::Argument],
    scope: &ExprFactScope,
    facts: Option<&RegistryFacts>,
) -> TypeFact {
    match &callee.kind {
        ExprKind::Path(path) => {
            let arg_facts = args
                .iter()
                .map(|arg| type_fact_from_expr_impl(&arg.value, scope, facts))
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
                .and_then(|arg| lambda_return_fact(&receiver, method, &arg.value, scope, facts));
            stdlib_method_fact(&receiver, method, lambda_return.as_ref())
                .map_or(TypeFact::Unknown, |fact| fact.returns)
        }
        ExprKind::Field { base, name } => {
            let receiver = type_fact_from_expr_impl(base, scope, facts);
            let lambda_return = args
                .first()
                .and_then(|arg| lambda_return_fact(&receiver, name, &arg.value, scope, facts));
            stdlib_method_fact(&receiver, name, lambda_return.as_ref())
                .map_or(TypeFact::Unknown, |fact| fact.returns)
        }
        _ => TypeFact::Unknown,
    }
}

fn path_field_fact(
    path: &[String],
    scope: &ExprFactScope,
    facts: Option<&RegistryFacts>,
) -> Option<TypeFact> {
    let facts = facts?;
    let (field, receiver_path) = path.split_last()?;
    if receiver_path.is_empty() {
        return None;
    }
    let receiver = scope.path_fact(receiver_path)?;
    registry_field_fact(receiver, field, facts)
}

fn field_access_fact(
    base: &Expr,
    field: &str,
    scope: &ExprFactScope,
    facts: Option<&RegistryFacts>,
) -> TypeFact {
    let receiver = type_fact_from_expr_impl(base, scope, facts);
    facts
        .and_then(|facts| registry_field_fact(&receiver, field, facts))
        .unwrap_or(TypeFact::Unknown)
}

fn registry_field_fact(
    receiver: &TypeFact,
    field: &str,
    facts: &RegistryFacts,
) -> Option<TypeFact> {
    match receiver {
        TypeFact::Host { name } | TypeFact::Record { name } => {
            facts.field_fact(name, field).cloned()
        }
        TypeFact::Enum {
            name,
            variant: Some(variant),
        } => facts
            .field_fact(&format!("{name}.{variant}"), field)
            .cloned(),
        _ => None,
    }
}

fn index_fact(base: TypeFact, index: TypeFact) -> TypeFact {
    match base {
        TypeFact::Array { element } if accepts_int_index(&index) => *element,
        TypeFact::Map { key, value } if accepts_map_key(&index, &key) => *value,
        TypeFact::Union(facts) => TypeFact::union(
            facts
                .into_iter()
                .map(|fact| index_fact(fact, index.clone()))
                .filter(|fact| !matches!(fact, TypeFact::Unknown)),
        ),
        _ => TypeFact::Unknown,
    }
}

fn accepts_int_index(index: &TypeFact) -> bool {
    match index {
        TypeFact::Int | TypeFact::Any | TypeFact::Unknown => true,
        TypeFact::Union(facts) => facts.iter().any(accepts_int_index),
        _ => false,
    }
}

fn accepts_map_key(index: &TypeFact, key: &TypeFact) -> bool {
    match (index, key) {
        (TypeFact::Any | TypeFact::Unknown, _) | (_, TypeFact::Any | TypeFact::Unknown) => true,
        (TypeFact::Union(facts), key) => facts.iter().any(|fact| accepts_map_key(fact, key)),
        (index, TypeFact::Union(facts)) => facts.iter().any(|fact| accepts_map_key(index, fact)),
        _ => key == index,
    }
}

fn map_key_fact(key: &Expr, scope: &ExprFactScope, facts: Option<&RegistryFacts>) -> TypeFact {
    match &key.kind {
        ExprKind::Literal(Literal::String(_))
        | ExprKind::Literal(Literal::Int(_))
        | ExprKind::Literal(Literal::Float(_))
        | ExprKind::Path(_) => TypeFact::String,
        _ => type_fact_from_expr_impl(key, scope, facts),
    }
}

fn lambda_return_fact(
    receiver: &TypeFact,
    method: &str,
    expr: &Expr,
    scope: &ExprFactScope,
    facts: Option<&RegistryFacts>,
) -> Option<TypeFact> {
    let ExprKind::Lambda { params, body } = &expr.kind else {
        return None;
    };
    let method_fact = stdlib_method_fact(receiver, method, None);
    let declared_param_count = params.len();
    let inferred_params = method_fact
        .and_then(|fact| fact.lambda.map(|lambda| lambda.params))
        .map(|params| {
            inferred_lambda_params_for_call(receiver, method, declared_param_count, params)
        });
    let TypeFact::Function { returns, .. } =
        lambda_fact(params, body, scope, facts, inferred_params)
    else {
        return None;
    };
    Some(*returns)
}

fn inferred_lambda_params_for_call(
    receiver: &TypeFact,
    method: &str,
    declared_param_count: usize,
    default_params: Vec<TypeFact>,
) -> Vec<TypeFact> {
    if !matches!(
        method,
        "map_values" | "filter" | "find" | "any" | "all" | "count"
    ) {
        return default_params;
    }
    let TypeFact::Map { value, .. } = receiver else {
        return default_params;
    };
    match declared_param_count {
        0 => Vec::new(),
        1 => vec![(**value).clone()],
        _ => default_params,
    }
}

fn lambda_fact(
    params: &[Param],
    body: &Expr,
    scope: &ExprFactScope,
    facts: Option<&RegistryFacts>,
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

    let returns = type_fact_from_expr_impl(body, &nested, facts);
    TypeFact::function(param_facts, returns)
}

fn block_fact(block: &Block, scope: &ExprFactScope, facts: Option<&RegistryFacts>) -> TypeFact {
    block
        .statements
        .iter()
        .rev()
        .find_map(|statement| match &statement.kind {
            StmtKind::Return(Some(expr)) | StmtKind::Expr(expr) => {
                Some(type_fact_from_expr_impl(expr, scope, facts))
            }
            StmtKind::Block(block) => Some(block_fact(block, scope, facts)),
            _ => None,
        })
        .unwrap_or(TypeFact::Null)
}

fn if_expr_fact(
    if_expr: &vela_syntax::ast::IfExpr,
    scope: &ExprFactScope,
    facts: Option<&RegistryFacts>,
) -> TypeFact {
    let then_scope = scope.narrowed_by_condition(&if_expr.condition, true);
    let else_scope = scope.narrowed_by_condition(&if_expr.condition, false);
    let mut branch_facts = vec![block_fact(&if_expr.then_branch, &then_scope, facts)];
    branch_facts.push(
        if_expr
            .else_branch
            .as_ref()
            .map_or(TypeFact::Null, |else_branch| {
                else_branch_fact(else_branch, &else_scope, facts)
            }),
    );
    TypeFact::union(branch_facts)
}

fn else_branch_fact(
    else_branch: &ElseBranch,
    scope: &ExprFactScope,
    facts: Option<&RegistryFacts>,
) -> TypeFact {
    match else_branch {
        ElseBranch::If(if_expr) => if_expr_fact(if_expr, scope, facts),
        ElseBranch::Block(block) => block_fact(block, scope, facts),
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
mod tests;
