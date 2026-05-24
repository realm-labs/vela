use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ScriptMethodTable {
    methods: BTreeMap<ScriptMethodKey, String>,
}

impl ScriptMethodTable {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(
        &mut self,
        type_name: impl Into<String>,
        method: impl Into<String>,
        function: impl Into<String>,
    ) {
        self.methods.insert(
            ScriptMethodKey {
                type_name: type_name.into(),
                method: method.into(),
            },
            function.into(),
        );
    }

    #[must_use]
    pub fn get(&self, type_name: &str, method: &str) -> Option<&str> {
        self.methods
            .get(&ScriptMethodKey {
                type_name: type_name.to_owned(),
                method: method.to_owned(),
            })
            .map(String::as_str)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ScriptMethodKey {
    type_name: String,
    method: String,
}
