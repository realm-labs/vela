pub use vela_common::{Capability, CapabilitySet};

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
