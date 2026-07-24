use std::collections::BTreeMap;

use crate::equality::ResumableComparison;
use crate::option_result::StdEnumVariant;
use crate::value_key::ValueKey;
use crate::{
    CallbackMethodInlineCacheTarget, StandardMethodReceiver, Value, VmError, VmErrorKind, VmResult,
};

pub(super) enum CallbackState {
    Iterator(Box<crate::iteration::ResumableIteratorMethod>),
    Sequence {
        receiver: StandardMethodReceiver,
        source: Value,
        target: CallbackMethodInlineCacheTarget,
        operation: &'static str,
        values: Vec<Value>,
        index: usize,
        host_sequence: Option<Box<crate::iteration::IteratorState>>,
        output: Vec<Value>,
        count: i64,
        total: NumericTotal,
        found: Option<Value>,
        decision: Option<bool>,
        retain: Vec<bool>,
        awaiting: Option<Value>,
    },
    Map {
        source: Value,
        target: CallbackMethodInlineCacheTarget,
        operation: &'static str,
        entries: Vec<(Value, Value)>,
        index: usize,
        host_sequence: Option<Box<crate::iteration::IteratorState>>,
        output: Vec<(Value, Value)>,
        count: i64,
        found: Option<(Value, Value)>,
        decision: Option<bool>,
        retain: Vec<bool>,
        awaiting: Option<(Value, Value)>,
    },
    GroupBy {
        operation: &'static str,
        values: Vec<Value>,
        index: usize,
        host_sequence: Option<Box<crate::iteration::IteratorState>>,
        groups: BTreeMap<ValueKey, GroupValues>,
        awaiting: Option<Value>,
    },
    MapGroupBy {
        operation: &'static str,
        entries: Vec<(Value, Value)>,
        index: usize,
        host_sequence: Option<Box<crate::iteration::IteratorState>>,
        groups: BTreeMap<ValueKey, GroupEntries>,
        awaiting: Option<(Value, Value)>,
    },
    SortBy(SortByState),
    Enum {
        receiver_kind: StandardMethodReceiver,
        target: CallbackMethodInlineCacheTarget,
        operation: &'static str,
        receiver: Value,
        variant: StdEnumVariant,
        payload: Option<Value>,
        active: bool,
        awaiting: bool,
    },
    Complete,
}

pub(super) struct GroupValues {
    pub(super) key: Value,
    pub(super) values: Vec<Value>,
}

pub(super) struct GroupEntries {
    pub(super) key: Value,
    pub(super) entries: Vec<(Value, Value)>,
}

pub(super) struct SortByState {
    pub(super) operation: &'static str,
    pub(super) values: Vec<Value>,
    pub(super) index: usize,
    pub(super) entries: Vec<SortByEntry>,
    pub(super) awaiting_callback: Option<Value>,
    pub(super) collecting: bool,
    pub(super) sort_index: usize,
    pub(super) current: usize,
    pub(super) comparison: Option<ResumableComparison>,
}

pub(super) struct SortByEntry {
    pub(super) key: Value,
    pub(super) value: Value,
}

pub(super) enum NumericTotal {
    Int(i64),
    Float(f64),
}

impl NumericTotal {
    pub(super) fn add_value(&mut self, value: &Value, operation: &'static str) -> VmResult<()> {
        match (&mut *self, value) {
            (Self::Int(total), Value::I64(value)) => {
                *total = total
                    .checked_add(*value)
                    .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
            }
            (Self::Int(total), Value::F64(value)) => {
                *self = Self::Float(*total as f64 + *value);
            }
            (Self::Float(total), Value::I64(value)) => *total += *value as f64,
            (Self::Float(total), Value::F64(value)) => *total += *value,
            _ => return Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
        }
        Ok(())
    }

    pub(super) fn into_value(self) -> Value {
        match self {
            Self::Int(value) => Value::I64(value),
            Self::Float(value) => Value::F64(value),
        }
    }
}
