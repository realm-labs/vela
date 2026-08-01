use vela_common::{HostTypeId, PrimitiveTag, ShapeId};
use vela_def::{TypeId, VariantId};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MirCallableKind {
    Function,
    Closure,
}

/// Runtime callable representations accepted by one source contract.
/// A `Function` contract accepts both direct functions and closures; a
/// `Closure` contract accepts closures only.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MirCallableKindSet {
    direct_function: bool,
    closure: bool,
}

impl MirCallableKindSet {
    pub const FUNCTION: Self = Self {
        direct_function: true,
        closure: true,
    };
    pub const CLOSURE: Self = Self {
        direct_function: false,
        closure: true,
    };

    #[must_use]
    pub const fn accepts(self, kind: MirCallableKind) -> bool {
        match kind {
            MirCallableKind::Function => self.direct_function,
            MirCallableKind::Closure => self.closure,
        }
    }

    #[must_use]
    pub const fn accepts_direct_function(self) -> bool {
        self.direct_function
    }

    #[must_use]
    pub const fn accepts_closure(self) -> bool {
        self.closure
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirTypeContract {
    Any,
    /// Sealed owned error payload delivered only by detached task outcomes.
    TaskError,
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
        accepted_kinds: MirCallableKindSet,
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
