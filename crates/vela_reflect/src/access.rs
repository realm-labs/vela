#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldAccess {
    pub readable: bool,
    pub writable: bool,
    pub reflect_readable: bool,
    pub reflect_writable: bool,
}

impl FieldAccess {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn readable(mut self, readable: bool) -> Self {
        self.readable = readable;
        self
    }

    #[must_use]
    pub fn writable(mut self, writable: bool) -> Self {
        self.writable = writable;
        self
    }

    #[must_use]
    pub fn reflect_readable(mut self, reflect_readable: bool) -> Self {
        self.reflect_readable = reflect_readable;
        self
    }

    #[must_use]
    pub fn reflect_writable(mut self, reflect_writable: bool) -> Self {
        self.reflect_writable = reflect_writable;
        self
    }
}

impl Default for FieldAccess {
    fn default() -> Self {
        Self {
            readable: true,
            writable: false,
            reflect_readable: true,
            reflect_writable: false,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FunctionEffectSet {
    pub reads_host: bool,
    pub writes_host: bool,
    pub emits_events: bool,
}

impl FunctionEffectSet {
    #[must_use]
    pub const fn pure() -> Self {
        Self {
            reads_host: false,
            writes_host: false,
            emits_events: false,
        }
    }

    #[must_use]
    pub const fn host_read() -> Self {
        Self {
            reads_host: true,
            writes_host: false,
            emits_events: false,
        }
    }

    #[must_use]
    pub const fn host_write() -> Self {
        Self {
            reads_host: true,
            writes_host: true,
            emits_events: false,
        }
    }

    #[must_use]
    pub const fn event_emit() -> Self {
        Self {
            reads_host: false,
            writes_host: false,
            emits_events: true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionAccess {
    pub public: bool,
    pub reflect_visible: bool,
    required_permissions: Vec<String>,
}

impl FunctionAccess {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn public(mut self, public: bool) -> Self {
        self.public = public;
        self
    }

    #[must_use]
    pub fn reflect_visible(mut self, reflect_visible: bool) -> Self {
        self.reflect_visible = reflect_visible;
        self
    }

    #[must_use]
    pub fn require_permission(mut self, permission: impl Into<String>) -> Self {
        self.required_permissions.push(permission.into());
        self.required_permissions.sort();
        self.required_permissions.dedup();
        self
    }

    #[must_use]
    pub fn required_permissions(&self) -> &[String] {
        &self.required_permissions
    }
}

impl Default for FunctionAccess {
    fn default() -> Self {
        Self {
            public: true,
            reflect_visible: true,
            required_permissions: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MethodEffectSet {
    pub reads_host: bool,
    pub writes_host: bool,
    pub emits_events: bool,
}

impl MethodEffectSet {
    #[must_use]
    pub const fn pure() -> Self {
        Self {
            reads_host: false,
            writes_host: false,
            emits_events: false,
        }
    }

    #[must_use]
    pub const fn host_read() -> Self {
        Self {
            reads_host: true,
            writes_host: false,
            emits_events: false,
        }
    }

    #[must_use]
    pub const fn host_write() -> Self {
        Self {
            reads_host: true,
            writes_host: true,
            emits_events: false,
        }
    }

    #[must_use]
    pub const fn event_emit() -> Self {
        Self {
            reads_host: false,
            writes_host: false,
            emits_events: true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAccess {
    pub public: bool,
    pub reflect_callable: bool,
    required_permissions: Vec<String>,
}

impl MethodAccess {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn public(mut self, public: bool) -> Self {
        self.public = public;
        self
    }

    #[must_use]
    pub fn reflect_callable(mut self, reflect_callable: bool) -> Self {
        self.reflect_callable = reflect_callable;
        self
    }

    #[must_use]
    pub fn require_permission(mut self, permission: impl Into<String>) -> Self {
        self.required_permissions.push(permission.into());
        self.required_permissions.sort();
        self.required_permissions.dedup();
        self
    }

    #[must_use]
    pub fn required_permissions(&self) -> &[String] {
        &self.required_permissions
    }
}

impl Default for MethodAccess {
    fn default() -> Self {
        Self {
            public: true,
            reflect_callable: true,
            required_permissions: Vec::new(),
        }
    }
}
