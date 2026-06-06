#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Capability {
    HostRead,
    HostWrite,
    EventEmit,
    Time,
    Random,
    ReflectionRead,
    ReflectionWrite,
    ReflectionCall,
}

impl Capability {
    pub const ALL: [Self; 8] = [
        Self::HostRead,
        Self::HostWrite,
        Self::EventEmit,
        Self::Time,
        Self::Random,
        Self::ReflectionRead,
        Self::ReflectionWrite,
        Self::ReflectionCall,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HostRead => "host_read",
            Self::HostWrite => "host_write",
            Self::EventEmit => "event_emit",
            Self::Time => "time",
            Self::Random => "random",
            Self::ReflectionRead => "reflection_read",
            Self::ReflectionWrite => "reflection_write",
            Self::ReflectionCall => "reflection_call",
        }
    }

    const fn bit(self) -> u64 {
        1 << (self as u8)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CapabilitySet {
    bits: u64,
}

impl CapabilitySet {
    #[must_use]
    pub const fn new() -> Self {
        Self { bits: 0 }
    }

    #[must_use]
    pub const fn all() -> Self {
        let mut bits = 0;
        let mut index = 0;
        while index < Capability::ALL.len() {
            bits |= Capability::ALL[index].bit();
            index += 1;
        }
        Self { bits }
    }

    #[must_use]
    pub const fn with(mut self, capability: Capability) -> Self {
        self.bits |= capability.bit();
        self
    }

    pub fn insert(&mut self, capability: Capability) {
        self.bits |= capability.bit();
    }

    #[must_use]
    pub const fn contains(self, capability: Capability) -> bool {
        self.bits & capability.bit() != 0
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }

    pub fn iter(self) -> impl Iterator<Item = Capability> {
        Capability::ALL
            .into_iter()
            .filter(move |capability| self.contains(*capability))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionProfile {
    capabilities: CapabilitySet,
}

impl ExecutionProfile {
    #[must_use]
    pub const fn trusted() -> Self {
        Self {
            capabilities: CapabilitySet::all(),
        }
    }

    #[must_use]
    pub const fn embedded() -> Self {
        Self {
            capabilities: CapabilitySet::new()
                .with(Capability::HostRead)
                .with(Capability::HostWrite)
                .with(Capability::EventEmit)
                .with(Capability::Time)
                .with(Capability::Random),
        }
    }

    #[must_use]
    pub const fn sandboxed() -> Self {
        Self {
            capabilities: CapabilitySet::new(),
        }
    }

    #[must_use]
    pub const fn capabilities(self) -> CapabilitySet {
        self.capabilities
    }
}

impl Default for ExecutionProfile {
    fn default() -> Self {
        Self::sandboxed()
    }
}
