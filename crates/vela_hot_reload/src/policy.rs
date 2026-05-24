#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HotReloadPolicy {
    allow_new_functions: bool,
    allow_defaulted_parameter_additions: bool,
}

impl HotReloadPolicy {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            allow_new_functions: true,
            allow_defaulted_parameter_additions: true,
        }
    }

    #[must_use]
    pub const fn locked_down() -> Self {
        Self {
            allow_new_functions: false,
            allow_defaulted_parameter_additions: false,
        }
    }

    #[must_use]
    pub const fn with_new_functions(mut self, allow: bool) -> Self {
        self.allow_new_functions = allow;
        self
    }

    #[must_use]
    pub const fn with_defaulted_parameter_additions(mut self, allow: bool) -> Self {
        self.allow_defaulted_parameter_additions = allow;
        self
    }

    #[must_use]
    pub const fn allow_new_functions(&self) -> bool {
        self.allow_new_functions
    }

    #[must_use]
    pub const fn allow_defaulted_parameter_additions(&self) -> bool {
        self.allow_defaulted_parameter_additions
    }
}

impl Default for HotReloadPolicy {
    fn default() -> Self {
        Self::new()
    }
}
