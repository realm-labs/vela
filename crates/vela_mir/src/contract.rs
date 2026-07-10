use vela_common::{HostTypeId, PrimitiveTag, ShapeId};
use vela_def::{TypeId, VariantId};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HostTypeTarget {
    pub semantic: TypeId,
    pub runtime: HostTypeId,
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
