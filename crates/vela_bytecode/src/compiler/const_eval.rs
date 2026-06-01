use std::collections::BTreeMap;
use std::num::{ParseFloatError, ParseIntError};

use vela_syntax::{BinaryOp, Expr, ExprKind, Literal, MapEntry, UnaryOp};

use crate::Constant;

use super::{CompileError, CompileErrorKind, CompileResult};

pub(super) fn compile_literal_constant(literal: &Literal) -> CompileResult<Constant> {
    Ok(match literal {
        Literal::Null => Constant::Null,
        Literal::Bool(value) => Constant::Bool(*value),
        Literal::Int(value) => Constant::Int(parse_int(value)?),
        Literal::Float(value) => Constant::Float(parse_float(value)?),
        Literal::String(value) => Constant::String(value.clone()),
    })
}

pub(super) fn evaluate_const_expr(
    expr: &Expr,
    values_by_name: &BTreeMap<String, Constant>,
) -> CompileResult<Option<Constant>> {
    match &expr.kind {
        ExprKind::Literal(literal) => compile_literal_constant(literal).map(Some),
        ExprKind::Path(path) => {
            let [name] = path.as_slice() else {
                return Ok(None);
            };
            Ok(values_by_name.get(name).cloned())
        }
        ExprKind::Unary { op, expr } => {
            let Some(value) = evaluate_const_expr(expr, values_by_name)? else {
                return Ok(None);
            };
            Ok(evaluate_unary_const(*op, value))
        }
        ExprKind::Binary { op, left, right } => {
            let Some(left) = evaluate_const_expr(left, values_by_name)? else {
                return Ok(None);
            };
            let Some(right) = evaluate_const_expr(right, values_by_name)? else {
                return Ok(None);
            };
            Ok(evaluate_binary_const(*op, left, right))
        }
        ExprKind::Array(values) => values
            .iter()
            .map(|value| evaluate_const_expr(value, values_by_name))
            .collect::<CompileResult<Option<Vec<_>>>>()
            .map(|values| values.map(Constant::Array)),
        ExprKind::Map(entries) => entries
            .iter()
            .map(|entry| evaluate_map_entry(entry, values_by_name))
            .collect::<CompileResult<Option<Vec<_>>>>()
            .map(|entries| entries.map(Constant::Map)),
        ExprKind::Block(_)
        | ExprKind::If(_)
        | ExprKind::Match(_)
        | ExprKind::SelfValue
        | ExprKind::Assign { .. }
        | ExprKind::Field { .. }
        | ExprKind::Call { .. }
        | ExprKind::Index { .. }
        | ExprKind::Try(_)
        | ExprKind::Record { .. }
        | ExprKind::Lambda { .. }
        | ExprKind::Error => Ok(None),
    }
}

fn evaluate_map_entry(
    entry: &MapEntry,
    values_by_name: &BTreeMap<String, Constant>,
) -> CompileResult<Option<(String, Constant)>> {
    let Some(value) = evaluate_const_expr(&entry.value, values_by_name)? else {
        return Ok(None);
    };
    let Some(key) = const_map_key_name(&entry.key)? else {
        return Ok(None);
    };
    Ok(Some((key, value)))
}

fn const_map_key_name(key: &Expr) -> CompileResult<Option<String>> {
    match &key.kind {
        ExprKind::Literal(Literal::String(value))
        | ExprKind::Literal(Literal::Int(value))
        | ExprKind::Literal(Literal::Float(value)) => Ok(Some(value.clone())),
        ExprKind::Path(path) => Ok(Some(path.join("."))),
        _ => Ok(None),
    }
}

fn evaluate_unary_const(op: UnaryOp, value: Constant) -> Option<Constant> {
    match (op, value) {
        (UnaryOp::Negate, Constant::Int(value)) => value.checked_neg().map(Constant::Int),
        (UnaryOp::Negate, Constant::Float(value)) => Some(Constant::Float(-value)),
        (UnaryOp::Not, Constant::Bool(value)) => Some(Constant::Bool(!value)),
        _ => None,
    }
}

fn evaluate_binary_const(op: BinaryOp, left: Constant, right: Constant) -> Option<Constant> {
    match op {
        BinaryOp::Add => evaluate_numeric_const(left, right, i64::checked_add, |a, b| a + b),
        BinaryOp::Sub => evaluate_numeric_const(left, right, i64::checked_sub, |a, b| a - b),
        BinaryOp::Mul => evaluate_numeric_const(left, right, i64::checked_mul, |a, b| a * b),
        BinaryOp::Div => match (left, right) {
            (Constant::Int(_), Constant::Int(0)) => None,
            (Constant::Int(left), Constant::Int(right)) => {
                left.checked_div(right).map(Constant::Int)
            }
            (Constant::Float(_), Constant::Float(0.0)) => None,
            (Constant::Float(left), Constant::Float(right)) => Some(Constant::Float(left / right)),
            _ => None,
        },
        BinaryOp::Rem => match (left, right) {
            (Constant::Int(_), Constant::Int(0)) => None,
            (Constant::Int(left), Constant::Int(right)) => {
                left.checked_rem(right).map(Constant::Int)
            }
            (Constant::Float(_), Constant::Float(0.0)) => None,
            (Constant::Float(left), Constant::Float(right)) => Some(Constant::Float(left % right)),
            _ => None,
        },
        BinaryOp::Equal => Some(Constant::Bool(left == right)),
        BinaryOp::NotEqual => Some(Constant::Bool(left != right)),
        BinaryOp::Less => evaluate_numeric_compare_const(left, right, |a, b| a < b),
        BinaryOp::LessEqual => evaluate_numeric_compare_const(left, right, |a, b| a <= b),
        BinaryOp::Greater => evaluate_numeric_compare_const(left, right, |a, b| a > b),
        BinaryOp::GreaterEqual => evaluate_numeric_compare_const(left, right, |a, b| a >= b),
        BinaryOp::Range | BinaryOp::RangeInclusive | BinaryOp::Or | BinaryOp::And => None,
    }
}

fn evaluate_numeric_const(
    left: Constant,
    right: Constant,
    int_op: impl FnOnce(i64, i64) -> Option<i64>,
    float_op: impl FnOnce(f64, f64) -> f64,
) -> Option<Constant> {
    match (left, right) {
        (Constant::Int(left), Constant::Int(right)) => int_op(left, right).map(Constant::Int),
        (Constant::Float(left), Constant::Float(right)) => {
            Some(Constant::Float(float_op(left, right)))
        }
        _ => None,
    }
}

fn evaluate_numeric_compare_const(
    left: Constant,
    right: Constant,
    op: impl FnOnce(f64, f64) -> bool,
) -> Option<Constant> {
    match (left, right) {
        (Constant::Int(left), Constant::Int(right)) => {
            Some(Constant::Bool(op(left as f64, right as f64)))
        }
        (Constant::Float(left), Constant::Float(right)) => Some(Constant::Bool(op(left, right))),
        _ => None,
    }
}

fn parse_int(value: &str) -> CompileResult<i64> {
    let value_without_separators = value.replace('_', "");
    let (radix, digits) = if let Some(digits) = value_without_separators.strip_prefix("0x") {
        (16, digits)
    } else if let Some(digits) = value_without_separators.strip_prefix("0b") {
        (2, digits)
    } else {
        (10, value_without_separators.as_str())
    };
    i64::from_str_radix(digits, radix).map_err(|error: ParseIntError| {
        CompileError::new(CompileErrorKind::InvalidIntLiteral {
            literal: value.to_owned(),
            error: error.to_string(),
        })
    })
}

fn parse_float(value: &str) -> CompileResult<f64> {
    value
        .replace('_', "")
        .parse()
        .map_err(|error: ParseFloatError| {
            CompileError::new(CompileErrorKind::InvalidFloatLiteral {
                literal: value.to_owned(),
                error: error.to_string(),
            })
        })
}
