use std::collections::BTreeMap;

use vela_def::MethodId;

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
        if let Some(existing) = self.methods.get(&key) {
            self.methods_by_id.remove(&ScriptMethodIdKey {
                type_name: key.type_name.clone(),
                id: existing.id,
            });
        }
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

    pub fn function_names(&self) -> impl Iterator<Item = &str> {
        self.methods.values().map(|method| method.function.as_str())
    }

    pub fn methods(&self) -> impl Iterator<Item = (&str, &str, &ScriptMethod)> {
        self.methods
            .iter()
            .map(|(key, method)| (key.type_name.as_str(), key.method.as_str(), method))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reinserting_named_method_removes_old_id_index() {
        let mut table = ScriptMethodTable::new();
        let old_id = MethodId::new(1);
        let new_id = MethodId::new(2);

        table.insert("Account", "apply", old_id, "Account::apply_old");
        table.insert("Account", "apply", new_id, "Account::apply_new");

        assert!(table.get_by_id("Account", old_id).is_none());
        assert_eq!(
            table.get_by_id("Account", new_id),
            Some(&ScriptMethod {
                id: new_id,
                function: "Account::apply_new".to_owned()
            })
        );
    }
}
