/// Closed effect facts used by analysis, tooling schemas, and diagnostics.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RegistryEffectFact {
    pub reads_host: bool,
    pub writes_host: bool,
    pub emits_events: bool,
    pub reads_time: bool,
    pub uses_random: bool,
    pub reads_io: bool,
    pub writes_io: bool,
    pub reads_reflection: bool,
    pub writes_reflection: bool,
    pub calls_reflection: bool,
    pub spawns_tasks: bool,
}

impl RegistryEffectFact {
    #[must_use]
    pub const fn pure() -> Self {
        Self {
            reads_host: false,
            writes_host: false,
            emits_events: false,
            reads_time: false,
            uses_random: false,
            reads_io: false,
            writes_io: false,
            reads_reflection: false,
            writes_reflection: false,
            calls_reflection: false,
            spawns_tasks: false,
        }
    }

    #[must_use]
    pub const fn host_read() -> Self {
        Self {
            reads_host: true,
            ..Self::pure()
        }
    }

    #[must_use]
    pub const fn host_write() -> Self {
        Self {
            reads_host: true,
            writes_host: true,
            ..Self::pure()
        }
    }

    #[must_use]
    pub const fn event_emit() -> Self {
        Self {
            emits_events: true,
            ..Self::pure()
        }
    }

    #[must_use]
    pub fn denied_by(&self, allowed: &Self) -> Vec<&'static str> {
        self.effect_flags()
            .into_iter()
            .zip(allowed.effect_flags())
            .filter_map(|((name, required), (_, allowed))| (required && !allowed).then_some(name))
            .collect()
    }

    #[must_use]
    pub fn display_name(&self) -> String {
        let effects = self
            .effect_flags()
            .into_iter()
            .filter_map(|(name, enabled)| enabled.then_some(name))
            .collect::<Vec<_>>();
        if effects.is_empty() {
            "pure".to_owned()
        } else {
            effects.join(", ")
        }
    }

    pub fn union_with(&mut self, other: &Self) {
        self.reads_host |= other.reads_host;
        self.writes_host |= other.writes_host;
        self.emits_events |= other.emits_events;
        self.reads_time |= other.reads_time;
        self.uses_random |= other.uses_random;
        self.reads_io |= other.reads_io;
        self.writes_io |= other.writes_io;
        self.reads_reflection |= other.reads_reflection;
        self.writes_reflection |= other.writes_reflection;
        self.calls_reflection |= other.calls_reflection;
        self.spawns_tasks |= other.spawns_tasks;
    }

    fn effect_flags(&self) -> [(&'static str, bool); 11] {
        [
            ("reads_host", self.reads_host && !self.writes_host),
            ("writes_host", self.writes_host),
            ("emits_events", self.emits_events),
            ("reads_time", self.reads_time),
            ("uses_random", self.uses_random),
            ("reads_io", self.reads_io),
            ("writes_io", self.writes_io),
            ("reads_reflection", self.reads_reflection),
            ("writes_reflection", self.writes_reflection),
            ("calls_reflection", self.calls_reflection),
            ("spawns_tasks", self.spawns_tasks),
        ]
    }
}
