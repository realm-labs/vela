use std::collections::BTreeSet;

use crate::{MirLocalId, MirSourceOrigin, MirTempId, MirTypeContract};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MirEffect {
    pub may_trap: bool,
    pub may_allocate: bool,
    pub script_call: bool,
    pub dynamic_call: bool,
    pub global_read: bool,
    pub host_read: bool,
    pub host_write: bool,
    pub host_call: bool,
    pub reflection_read: bool,
    pub reflection_write: bool,
    pub reflection_call: bool,
    pub emits_event: bool,
    pub reads_time: bool,
    pub uses_random: bool,
    pub reads_io: bool,
    pub writes_io: bool,
}

impl MirEffect {
    pub const PURE: Self = Self {
        may_trap: false,
        may_allocate: false,
        script_call: false,
        dynamic_call: false,
        global_read: false,
        host_read: false,
        host_write: false,
        host_call: false,
        reflection_read: false,
        reflection_write: false,
        reflection_call: false,
        emits_event: false,
        reads_time: false,
        uses_random: false,
        reads_io: false,
        writes_io: false,
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
    pub const fn global_read() -> Self {
        Self {
            may_trap: true,
            global_read: true,
            ..Self::PURE
        }
    }

    #[must_use]
    pub const fn host_read() -> Self {
        Self {
            may_trap: true,
            may_allocate: true,
            host_read: true,
            ..Self::PURE
        }
    }

    #[must_use]
    pub const fn host_write() -> Self {
        Self {
            may_trap: true,
            host_read: true,
            host_write: true,
            ..Self::PURE
        }
    }

    #[must_use]
    pub const fn host_call() -> Self {
        Self {
            may_trap: true,
            may_allocate: true,
            host_call: true,
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
            global_read: self.global_read || other.global_read,
            host_read: self.host_read || other.host_read,
            host_write: self.host_write || other.host_write,
            host_call: self.host_call || other.host_call,
            reflection_read: self.reflection_read || other.reflection_read,
            reflection_write: self.reflection_write || other.reflection_write,
            reflection_call: self.reflection_call || other.reflection_call,
            emits_event: self.emits_event || other.emits_event,
            reads_time: self.reads_time || other.reads_time,
            uses_random: self.uses_random || other.uses_random,
            reads_io: self.reads_io || other.reads_io,
            writes_io: self.writes_io || other.writes_io,
        }
    }

    #[must_use]
    pub const fn contains(self, required: Self) -> bool {
        (!required.may_trap || self.may_trap)
            && (!required.may_allocate || self.may_allocate)
            && (!required.script_call || self.script_call)
            && (!required.dynamic_call || self.dynamic_call)
            && (!required.global_read || self.global_read)
            && (!required.host_read || self.host_read)
            && (!required.host_write || self.host_write)
            && (!required.host_call || self.host_call)
            && (!required.reflection_read || self.reflection_read)
            && (!required.reflection_write || self.reflection_write)
            && (!required.reflection_call || self.reflection_call)
            && (!required.emits_event || self.emits_event)
            && (!required.reads_time || self.reads_time)
            && (!required.uses_random || self.uses_random)
            && (!required.reads_io || self.reads_io)
            && (!required.writes_io || self.writes_io)
    }

    #[must_use]
    pub fn is_pure(self) -> bool {
        self == Self::PURE
    }

    #[must_use]
    pub const fn requires_safepoint(self) -> bool {
        self.may_allocate || self.script_call || self.dynamic_call || self.host_call
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirGuardAssumption {
    Type(MirTypeContract),
    CallableArity { positional: u32, named: Vec<String> },
    TruthyBoolean,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirGuard {
    pub assumption: MirGuardAssumption,
    pub origin: MirSourceOrigin,
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
