#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldAccessDef {
    pub readable: bool,
    pub writable: bool,
    pub reflect_readable: bool,
    pub reflect_writable: bool,
    required_permissions: Vec<String>,
}

impl FieldAccessDef {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn readable(mut self, readable: bool) -> Self {
        self.readable = readable;
        self
    }

    #[must_use]
    pub const fn writable(mut self, writable: bool) -> Self {
        self.writable = writable;
        self
    }

    #[must_use]
    pub const fn reflect_readable(mut self, reflect_readable: bool) -> Self {
        self.reflect_readable = reflect_readable;
        self
    }

    #[must_use]
    pub const fn reflect_writable(mut self, reflect_writable: bool) -> Self {
        self.reflect_writable = reflect_writable;
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

impl Default for FieldAccessDef {
    fn default() -> Self {
        Self {
            readable: true,
            writable: true,
            reflect_readable: false,
            reflect_writable: false,
            required_permissions: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionAccessDef {
    pub public: bool,
    pub reflect_visible: bool,
    pub reflect_callable: bool,
}

impl FunctionAccessDef {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn public(mut self, public: bool) -> Self {
        self.public = public;
        self
    }

    #[must_use]
    pub const fn reflect_visible(mut self, reflect_visible: bool) -> Self {
        self.reflect_visible = reflect_visible;
        self
    }

    #[must_use]
    pub const fn reflect_callable(mut self, reflect_callable: bool) -> Self {
        self.reflect_callable = reflect_callable;
        self
    }
}

impl Default for FunctionAccessDef {
    fn default() -> Self {
        Self {
            public: true,
            reflect_visible: true,
            reflect_callable: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAccessDef {
    pub public: bool,
    pub reflect_callable: bool,
    required_permissions: Vec<String>,
}

impl MethodAccessDef {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn public(mut self, public: bool) -> Self {
        self.public = public;
        self
    }

    #[must_use]
    pub const fn reflect_callable(mut self, reflect_callable: bool) -> Self {
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

impl Default for MethodAccessDef {
    fn default() -> Self {
        Self {
            public: true,
            reflect_callable: true,
            required_permissions: Vec::new(),
        }
    }
}
