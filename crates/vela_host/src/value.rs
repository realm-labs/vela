use crate::path::HostRef;

#[derive(Clone, Debug, PartialEq)]
pub enum HostValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    HostRef(HostRef),
}

pub(crate) fn add_values(lhs: &HostValue, rhs: &HostValue) -> Option<HostValue> {
    match (lhs, rhs) {
        (HostValue::Int(lhs), HostValue::Int(rhs)) => Some(HostValue::Int(lhs + rhs)),
        (HostValue::Float(lhs), HostValue::Float(rhs)) => Some(HostValue::Float(lhs + rhs)),
        _ => None,
    }
}

pub(crate) fn sub_values(lhs: &HostValue, rhs: &HostValue) -> Option<HostValue> {
    match (lhs, rhs) {
        (HostValue::Int(lhs), HostValue::Int(rhs)) => Some(HostValue::Int(lhs - rhs)),
        (HostValue::Float(lhs), HostValue::Float(rhs)) => Some(HostValue::Float(lhs - rhs)),
        _ => None,
    }
}

pub(crate) fn mul_values(lhs: &HostValue, rhs: &HostValue) -> Option<HostValue> {
    match (lhs, rhs) {
        (HostValue::Int(lhs), HostValue::Int(rhs)) => lhs.checked_mul(*rhs).map(HostValue::Int),
        (HostValue::Float(lhs), HostValue::Float(rhs)) => Some(HostValue::Float(lhs * rhs)),
        _ => None,
    }
}

pub(crate) fn div_values(lhs: &HostValue, rhs: &HostValue) -> Option<HostValue> {
    match (lhs, rhs) {
        (HostValue::Int(_), HostValue::Int(0)) => None,
        (HostValue::Int(lhs), HostValue::Int(rhs)) => lhs.checked_div(*rhs).map(HostValue::Int),
        (HostValue::Float(_), HostValue::Float(rhs)) if *rhs == 0.0 => None,
        (HostValue::Float(lhs), HostValue::Float(rhs)) => Some(HostValue::Float(lhs / rhs)),
        _ => None,
    }
}

pub(crate) fn rem_values(lhs: &HostValue, rhs: &HostValue) -> Option<HostValue> {
    match (lhs, rhs) {
        (HostValue::Int(_), HostValue::Int(0)) => None,
        (HostValue::Int(lhs), HostValue::Int(rhs)) => lhs.checked_rem(*rhs).map(HostValue::Int),
        (HostValue::Float(_), HostValue::Float(rhs)) if *rhs == 0.0 => None,
        (HostValue::Float(lhs), HostValue::Float(rhs)) => Some(HostValue::Float(lhs % rhs)),
        _ => None,
    }
}
