use vela_common::{NumericTag, PrimitiveTag, ScalarValue, ShapeId};
use vela_def::{TypeId, VariantId};

use crate::{HostTypeTarget, MirLocalId, MirTempId};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MirImmediate {
    Unit,
    Bool(bool),
    Char(char),
    Scalar(ScalarValue),
}

/// A fully evaluated, backend-neutral constant value.
///
/// Evaluation happens before runtime MIR. Heap-backed variants are materialized
/// by an explicit `MaterializeConstant` statement at every runtime use so that
/// allocation, GC, identity, and budget behavior remain visible.
#[derive(Clone, Debug, PartialEq)]
pub enum MirEvaluatedConstant {
    Unit,
    Bool(bool),
    Char(char),
    Scalar(ScalarValue),
    String(String),
    Bytes(Vec<u8>),
    Array(Vec<Self>),
    Map(Vec<(String, Self)>),
}

impl MirEvaluatedConstant {
    #[must_use]
    pub const fn requires_allocation(&self) -> bool {
        matches!(
            self,
            Self::String(_) | Self::Bytes(_) | Self::Array(_) | Self::Map(_)
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirValueType {
    Dynamic,
    Unit,
    Primitive(PrimitiveTag),
    Range,
    Iterator,
    ScriptType { type_id: TypeId, shape: ShapeId },
    Enum(TypeId),
    Host(HostTypeTarget),
    Tuple(u32),
    Callable,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MirOperand {
    Immediate(MirImmediate),
    Local(MirLocalId),
    Temp(MirTempId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirPlace {
    Local(MirLocalId),
    Temp(MirTempId),
}

impl MirPlace {
    #[must_use]
    pub const fn local(local: MirLocalId) -> Self {
        Self::Local(local)
    }

    #[must_use]
    pub const fn temp(temp: MirTempId) -> Self {
        Self::Temp(temp)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirUnaryOp {
    NotBool,
    Negate(NumericTag),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirNumericBinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirComparisonOp {
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirBinaryOp {
    Numeric {
        operation: MirNumericBinaryOp,
        kind: NumericTag,
    },
    Compare {
        operation: MirComparisonOp,
        kind: PrimitiveTag,
    },
}

/// A non-trapping structural check used before pattern projections.
///
/// Stable checks carry definition identities. Dynamic checks also carry every
/// field name that a successful match must make safe to project. Host values
/// and host paths deliberately have no structural predicate form.
#[derive(Clone, Debug, PartialEq)]
pub enum MirPatternPredicate {
    TupleArity {
        value: MirOperand,
        arity: u32,
    },
    RecordShape {
        value: MirOperand,
        type_id: TypeId,
        shape: ShapeId,
    },
    VariantShape {
        value: MirOperand,
        type_id: TypeId,
        variant: VariantId,
    },
    DynamicRecord {
        value: MirOperand,
        type_name: String,
        required_fields: Vec<String>,
    },
    DynamicVariant {
        value: MirOperand,
        owner_name: String,
        variant_name: String,
        required_fields: Vec<String>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum MirRvalue {
    Use(MirOperand),
    Truthy { value: MirOperand },
    IsMissing { value: MirOperand },
    PatternPredicate(MirPatternPredicate),
}
