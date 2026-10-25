use std::collections::BTreeSet;

use crate::{MirLocalId, MirSourceOrigin, MirTempId, MirTypeContract};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MirEffect {
    pub may_trap: bool,
    pub may_allocate: bool,
    pub script_call: bool,
    pub dynamic_call: bool,
    pub state_read: bool,
    pub state_write: bool,
    pub reflection_read: bool,
    pub reflection_write: bool,
    pub reflection_call: bool,
    pub emits_event: bool,
    pub reads_time: bool,
    pub uses_random: bool,
    pub reads_io: bool,
    pub writes_io: bool,
    /// Capability facts for retained native calls, not borrowed-host operations.
    pub host_read: bool,
    pub host_write: bool,
}

impl MirEffect {
    pub const PURE: Self = Self {
        may_trap: false,
        may_allocate: false,
        script_call: false,
        dynamic_call: false,
        state_read: false,
        state_write: false,
        reflection_read: false,
        reflection_write: false,
        reflection_call: false,
        emits_event: false,
        reads_time: false,
        uses_random: false,
        reads_io: false,
        writes_io: false,
        host_read: false,
        host_write: false,
    };

    #[must_use]
    pub const fn may_trap() -> Self {
        Self {
            may_trap: true,
            ..Self::PURE
        }
    }

    #[must_use]
    pub const fn allocation() -> Self {
        Self {
            may_trap: true,
            may_allocate: true,
            ..Self::PURE
        }
    }

    #[must_use]
    pub const fn script_call() -> Self {
        Self {
            may_trap: true,
            may_allocate: true,
            script_call: true,
            ..Self::PURE
        }
    }

    #[must_use]
    pub const fn dynamic_call() -> Self {
        Self {
            may_trap: true,
            may_allocate: true,
            dynamic_call: true,
            ..Self::PURE
        }
    }

    #[must_use]
    pub const fn external_call() -> Self {
        Self {
            may_trap: true,
            may_allocate: true,
            ..Self::PURE
        }
    }

    #[must_use]
    pub const fn state_read() -> Self {
        Self {
            may_trap: true,
            state_read: true,
            ..Self::PURE
        }
    }

    #[must_use]
    pub const fn state_write() -> Self {
        Self {
            may_trap: true,
            state_read: true,
            state_write: true,
            ..Self::PURE
        }
    }

    #[must_use]
    pub const fn reflection_read() -> Self {
        Self {
            may_trap: true,
            may_allocate: true,
            reflection_read: true,
            ..Self::PURE
        }
    }

    #[must_use]
    pub const fn reflection_write() -> Self {
        Self {
            may_trap: true,
            may_allocate: true,
            reflection_write: true,
            ..Self::PURE
        }
    }

    #[must_use]
    pub const fn reflection_call() -> Self {
        Self {
            may_trap: true,
            may_allocate: true,
            reflection_call: true,
            ..Self::PURE
        }
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self {
            may_trap: self.may_trap || other.may_trap,
            may_allocate: self.may_allocate || other.may_allocate,
            script_call: self.script_call || other.script_call,
            dynamic_call: self.dynamic_call || other.dynamic_call,
            state_read: self.state_read || other.state_read,
            state_write: self.state_write || other.state_write,
            reflection_read: self.reflection_read || other.reflection_read,
            reflection_write: self.reflection_write || other.reflection_write,
            reflection_call: self.reflection_call || other.reflection_call,
            emits_event: self.emits_event || other.emits_event,
            reads_time: self.reads_time || other.reads_time,
            uses_random: self.uses_random || other.uses_random,
            reads_io: self.reads_io || other.reads_io,
            writes_io: self.writes_io || other.writes_io,
            host_read: self.host_read || other.host_read,
            host_write: self.host_write || other.host_write,
        }
    }

    #[must_use]
    pub const fn contains(self, required: Self) -> bool {
        (!required.may_trap || self.may_trap)
            && (!required.may_allocate || self.may_allocate)
            && (!required.script_call || self.script_call)
            && (!required.dynamic_call || self.dynamic_call)
            && (!required.state_read || self.state_read)
            && (!required.state_write || self.state_write)
            && (!required.reflection_read || self.reflection_read)
            && (!required.reflection_write || self.reflection_write)
            && (!required.reflection_call || self.reflection_call)
            && (!required.emits_event || self.emits_event)
            && (!required.reads_time || self.reads_time)
            && (!required.uses_random || self.uses_random)
            && (!required.reads_io || self.reads_io)
            && (!required.writes_io || self.writes_io)
            && (!required.host_read || self.host_read)
            && (!required.host_write || self.host_write)
    }

    #[must_use]
    pub fn is_pure(self) -> bool {
        self == Self::PURE
    }

    #[must_use]
    pub const fn requires_safepoint(self) -> bool {
        self.may_allocate || self.script_call || self.dynamic_call
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirGuardAssumption {
    Type(MirTypeContract),
    /// A trapping tuple-destructuring boundary.
    ///
    /// Match and loop patterns use non-trapping structural predicates instead;
    /// this assumption exists so a backend can preserve the distinct runtime
    /// type/arity diagnostics of `let (..) = value` without recognizing a CFG
    /// idiom.
    TupleArity {
        arity: u32,
    },
}

/// Backend-neutral source boundary described by a runtime guard.
///
/// Parameter indices are logical signature indices. Physical backends remain
/// responsible for checking whether their encoded operand width can represent
/// the index.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MirGuardLocation {
    Parameter { index: u32 },
    Return,
    Local,
    State,
    Field,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirGuardContext {
    pub location: MirGuardLocation,
    pub debug_name: String,
}

impl MirGuardContext {
    #[must_use]
    pub fn new(location: MirGuardLocation, debug_name: impl Into<String>) -> Self {
        Self {
            location,
            debug_name: debug_name.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirGuard {
    pub kind: MirGuardKind,
    pub assumption: MirGuardAssumption,
    /// Contract guards carry the source boundary used by runtime diagnostics.
    /// Pure optimization/branch assumptions may have no user-facing context.
    pub context: Option<MirGuardContext>,
    pub origin: MirSourceOrigin,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MirGuardKind {
    Contract,
    Specialization,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MirLiveValue {
    Local(MirLocalId),
    Temp(MirTempId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirSafepoint {
    pub origin: MirSourceOrigin,
    pub live_values: BTreeSet<MirLiveValue>,
}

impl MirSafepoint {
    #[must_use]
    pub fn new(origin: MirSourceOrigin) -> Self {
        Self {
            origin,
            live_values: BTreeSet::new(),
        }
    }

    #[must_use]
    pub fn with_live_values(
        origin: MirSourceOrigin,
        live_values: impl IntoIterator<Item = MirLiveValue>,
    ) -> Self {
        Self {
            origin,
            live_values: live_values.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod native_effect_tests {
    use super::MirEffect;

    #[test]
    fn native_host_effect_facts_survive_union_and_containment() {
        let read = MirEffect {
            host_read: true,
            ..MirEffect::PURE
        };
        let write = MirEffect {
            host_write: true,
            ..MirEffect::PURE
        };
        assert!(!read.is_pure());
        assert!(!write.is_pure());
        assert!(!MirEffect::PURE.contains(read));
        assert!(!MirEffect::PURE.contains(write));
        assert!(!read.contains(write));
        assert!(!write.contains(read));
        let both = read.union(write);
        assert!(both.host_read && both.host_write);
        assert!(both.contains(read) && both.contains(write));
        assert_eq!(both.union(MirEffect::PURE), both);
        assert_eq!(both.union(read), both);
    }

    #[test]
    fn native_call_minimum_effect_requires_registered_host_facts() {
        use crate::{CompilePositionalPolicy, CompileSignature, MirCall, MirStatementKind};

        let signature_effect = MirEffect {
            host_read: true,
            host_write: true,
            ..MirEffect::PURE
        };
        let statement = MirStatementKind::Call(MirCall::NativeFunction {
            function: vela_def::FunctionId::new(0xD100),
            debug_name: "native::value_only".to_owned(),
            signature: CompileSignature {
                parameters: Vec::new(),
                positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
                return_contract: None,
                effect: signature_effect,
            },
            arguments: Vec::new(),
        });
        let required = statement.minimum_effect();
        assert!(required.contains(signature_effect));
        assert!(required.contains(MirEffect::external_call()));
        assert!(!MirEffect::external_call().contains(required));
        assert!(required.requires_safepoint());
    }
}
