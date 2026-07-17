use super::{BytecodeProfileSnapshot, RuntimeImageStorage, RuntimeImpl};

impl<I> RuntimeImpl<I>
where
    I: RuntimeImageStorage,
{
    /// Returns the active generation's aggregate instruction profile.
    ///
    /// Profiling is disabled by default, so ordinary Runtimes return `None`
    /// and allocate no instruction counters.
    #[must_use]
    pub fn bytecode_profile_snapshot(&self) -> Option<BytecodeProfileSnapshot> {
        self.state.bytecode_profile_snapshot()
    }

    /// Resets the active generation's aggregate profile using relaxed atomic
    /// stores. Concurrent snapshots or execution may observe the reset in
    /// progress; no instruction count can wrap.
    pub fn reset_bytecode_profile(&self) -> bool {
        self.state.reset_bytecode_profile()
    }
}
