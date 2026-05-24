use std::collections::BTreeMap;

use vela_common::MethodId;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ScriptMethodTable {
    methods: BTreeMap<ScriptMethodKey, ScriptMethod>,
    methods_by_id: BTreeMap<ScriptMethodIdKey, ScriptMethodKey>,
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
        method_id: MethodId,
        function: impl Into<String>,
    ) {
        let key = ScriptMethodKey {
            type_name: type_name.into(),
            method: method.into(),
        };
        self.methods_by_id.insert(
            ScriptMethodIdKey {
                type_name: key.type_name.clone(),
                id: method_id,
            },
            key.clone(),
        );
        self.methods.insert(
            key,
            ScriptMethod {
                id: method_id,
                function: function.into(),
            },
        );
    }

    #[must_use]
    pub fn get(&self, type_name: &str, method: &str) -> Option<&ScriptMethod> {
        self.methods.get(&ScriptMethodKey {
            type_name: type_name.to_owned(),
            method: method.to_owned(),
        })
    }

    #[must_use]
    pub fn get_by_id(&self, type_name: &str, method_id: MethodId) -> Option<&ScriptMethod> {
        let key = self.methods_by_id.get(&ScriptMethodIdKey {
            type_name: type_name.to_owned(),
            id: method_id,
        })?;
        self.methods.get(key)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptMethod {
    pub id: MethodId,
    pub function: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ScriptMethodKey {
    type_name: String,
    method: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ScriptMethodIdKey {
    type_name: String,
    id: MethodId,
}
