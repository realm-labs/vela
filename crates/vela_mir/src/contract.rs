use vela_common::{HostTypeId, PrimitiveTag, ShapeId};
use vela_def::{TypeId, VariantId};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HostTypeTarget {
    pub semantic: TypeId,
    pub runtime: HostTypeId,
}

/// Source-language callable category checked by a backend-neutral contract.
///
/// Function values and closure values have distinct runtime representations
/// and guard behavior. Positional arity is modeled separately so an erased
/// callable contract (`None`) cannot be confused with a proven zero-argument
/// callable (`Some(0)`).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MirCallableKind {
    Function,
    Closure,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirTypeContract {
    Any,
    Primitive(PrimitiveTag),
    Range,
    Array(Option<Box<Self>>),
    Map {
        key: Option<Box<Self>>,
        value: Option<Box<Self>>,
    },
    Set(Option<Box<Self>>),
    Iterator(Option<Box<Self>>),
    Tuple(Vec<Option<Self>>),
    Option(Option<Box<Self>>),
    Result {
        ok: Option<Box<Self>>,
        err: Option<Box<Self>>,
    },
    Callable {
        kind: MirCallableKind,
        positional_arity: Option<u32>,
    },
    Definition(TypeId),
    Shape {
        type_id: TypeId,
        shape: ShapeId,
    },
    Variant {
        type_id: TypeId,
        variant: VariantId,
    },
    Host(HostTypeTarget),
}
