use std::collections::BTreeMap;

use vela_def::{FunctionId, MethodId, TypeId};

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
        owner: TypeId,
        type_name: impl Into<String>,
        method: impl Into<String>,
        method_id: MethodId,
        function_id: FunctionId,
        function: impl Into<String>,
    ) {
        let key = ScriptMethodKey {
            owner,
            method: method.into(),
        };
        let type_name = type_name.into();
        if let Some(existing) = self.methods.get(&key) {
            self.methods_by_id.remove(&ScriptMethodIdKey {
                owner,
                id: existing.id,
            });
        }
        self.methods_by_id.insert(
            ScriptMethodIdKey {
                owner,
                id: method_id,
            },
            key.clone(),
        );
        self.methods.insert(
            key,
            ScriptMethod {
                id: method_id,
                type_name,
                function_id,
                function: function.into(),
            },
        );
    }

    #[must_use]
    pub fn get(&self, owner: TypeId, method: &str) -> Option<&ScriptMethod> {
        self.methods.get(&ScriptMethodKey {
            owner,
            method: method.to_owned(),
        })
    }

    #[must_use]
    pub fn get_by_id(&self, owner: TypeId, method_id: MethodId) -> Option<&ScriptMethod> {
        let key = self.methods_by_id.get(&ScriptMethodIdKey {
            owner,
            id: method_id,
        })?;
        self.methods.get(key)
    }

    pub fn function_names(&self) -> impl Iterator<Item = &str> {
        self.methods.values().map(|method| method.function.as_str())
    }

    pub fn methods(&self) -> impl Iterator<Item = (TypeId, &str, &str, &ScriptMethod)> {
        self.methods.iter().map(|(key, method)| {
            (
                key.owner,
                method.type_name.as_str(),
                key.method.as_str(),
                method,
            )
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptMethod {
    pub id: MethodId,
    pub type_name: String,
    pub function_id: FunctionId,
    pub function: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ScriptMethodKey {
    owner: TypeId,
    method: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ScriptMethodIdKey {
    owner: TypeId,
    id: MethodId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reinserting_named_method_removes_old_id_index() {
        let mut table = ScriptMethodTable::new();
        let old_id = MethodId::new(1);
        let new_id = MethodId::new(2);

        let owner = TypeId::new(9);
        table.insert(
            owner,
            "Account",
            "apply",
            old_id,
            FunctionId::new(10),
            "Account::apply_old",
        );
        table.insert(
            owner,
            "Account",
            "apply",
            new_id,
            FunctionId::new(11),
            "Account::apply_new",
        );

        assert!(table.get_by_id(owner, old_id).is_none());
        assert_eq!(
            table.get_by_id(owner, new_id),
            Some(&ScriptMethod {
                id: new_id,
                type_name: "Account".to_owned(),
                function_id: FunctionId::new(11),
                function: "Account::apply_new".to_owned()
            })
        );
    }
}
