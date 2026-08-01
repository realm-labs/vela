#[repr(u8)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Capability {
    HostRead,
    HostWrite,
    EventEmit,
    Time,
    Random,
    IoRead,
    IoWrite,
    ReflectionRead,
    ReflectionWrite,
    ReflectionCall,
    TaskSpawn,
}

impl Capability {
    pub const ALL: [Self; 11] = [
        Self::HostRead,
        Self::HostWrite,
        Self::EventEmit,
        Self::Time,
        Self::Random,
        Self::IoRead,
        Self::IoWrite,
        Self::ReflectionRead,
        Self::ReflectionWrite,
        Self::ReflectionCall,
        Self::TaskSpawn,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HostRead => "host_read",
            Self::HostWrite => "host_write",
            Self::EventEmit => "event_emit",
            Self::Time => "time",
            Self::Random => "random",
            Self::IoRead => "io_read",
            Self::IoWrite => "io_write",
            Self::ReflectionRead => "reflection_read",
            Self::ReflectionWrite => "reflection_write",
            Self::ReflectionCall => "reflection_call",
            Self::TaskSpawn => "task_spawn",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|value| value.as_str() == name)
    }

    const fn bit(self) -> u64 {
        1 << (self as u8)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
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
    pub const fn contains_all(self, required: Self) -> bool {
        self.bits & required.bits == required.bits
    }

    #[must_use]
    pub const fn difference(self, other: Self) -> Self {
        Self {
            bits: self.bits & !other.bits,
        }
    }

    #[must_use]
    pub const fn intersection(self, other: Self) -> Self {
        Self {
            bits: self.bits & other.bits,
        }
    }

    #[must_use]
    pub const fn without(mut self, capability: Capability) -> Self {
        self.bits &= !capability.bit();
        self
    }

    #[must_use]
    pub const fn from_bits(bits: u64) -> Self {
        Self { bits }
    }

    #[must_use]
    pub const fn bits(self) -> u64 {
        self.bits
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
