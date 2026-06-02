use std::collections::{HashMap, HashSet};

use vela_common::{FieldId, HostMethodId};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompilerOptions {
    pub(super) host_fields: HashMap<String, FieldId>,
    pub(super) host_variant_fields: HashMap<String, FieldId>,
    pub(super) host_methods: HashMap<String, HostMethodId>,
    pub(super) host_methods_by_type: HashMap<(String, String), HostMethodId>,
    pub(super) host_method_params: HashMap<HostMethodId, Vec<HostMethodParam>>,
    pub(super) value_method_params: HashMap<String, Vec<ValueMethodParam>>,
    pub(super) value_method_params_by_type: HashMap<(String, String), Vec<ValueMethodParam>>,
    pub(super) host_types: HashSet<String>,
    pub(super) native_module_roots: HashSet<String>,
    pub(super) native_function_params: HashMap<String, Vec<NativeFunctionParam>>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct HostMethodParam {
    pub(super) name: String,
    pub(super) has_default: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ValueMethodParam {
    pub(super) name: String,
    pub(super) has_default: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct NativeFunctionParam {
    pub(super) name: String,
}

impl CompilerOptions {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_host_field(mut self, name: impl Into<String>, field: FieldId) -> Self {
        self.host_fields.insert(name.into(), field);
        self
    }

    #[must_use]
    pub fn with_host_variant_field(mut self, name: impl Into<String>, field: FieldId) -> Self {
        self.host_variant_fields.insert(name.into(), field);
        self
    }

    #[must_use]
    pub fn with_host_method(mut self, name: impl Into<String>, method: HostMethodId) -> Self {
        self.host_methods.insert(name.into(), method);
        self
    }

    #[must_use]
    pub fn with_host_method_params<I, S>(mut self, method: HostMethodId, params: I) -> Self
    where
        I: IntoIterator<Item = (S, bool)>,
        S: Into<String>,
    {
        self.host_method_params.insert(
            method,
            params
                .into_iter()
                .map(|(name, has_default)| HostMethodParam {
                    name: name.into(),
                    has_default,
                })
                .collect(),
        );
        self
    }

    #[must_use]
    pub fn with_value_method_params<I, S>(mut self, method: impl Into<String>, params: I) -> Self
    where
        I: IntoIterator<Item = (S, bool)>,
        S: Into<String>,
    {
        self.value_method_params.insert(
            method.into(),
            params
                .into_iter()
                .map(|(name, has_default)| ValueMethodParam {
                    name: name.into(),
                    has_default,
                })
                .collect(),
        );
        self
    }

    #[must_use]
    pub fn with_value_method_params_for_type<I, S>(
        mut self,
        type_name: impl Into<String>,
        method: impl Into<String>,
        params: I,
    ) -> Self
    where
        I: IntoIterator<Item = (S, bool)>,
        S: Into<String>,
    {
        self.value_method_params_by_type.insert(
            (type_name.into(), method.into()),
            params
                .into_iter()
                .map(|(name, has_default)| ValueMethodParam {
                    name: name.into(),
                    has_default,
                })
                .collect(),
        );
        self
    }

    #[must_use]
    pub fn with_required_value_method_params<I, S>(
        self,
        method: impl Into<String>,
        params: I,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.with_value_method_params(method, params.into_iter().map(|name| (name, false)))
    }

    #[must_use]
    pub fn with_required_value_method_params_for_type<I, S>(
        self,
        type_name: impl Into<String>,
        method: impl Into<String>,
        params: I,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.with_value_method_params_for_type(
            type_name,
            method,
            params.into_iter().map(|name| (name, false)),
        )
    }

    #[must_use]
    pub fn with_host_type(mut self, type_name: impl Into<String>) -> Self {
        self.host_types.insert(type_name.into());
        self
    }

    #[must_use]
    pub fn with_native_module_root(mut self, root: impl Into<String>) -> Self {
        self.native_module_roots.insert(root.into());
        self
    }

    #[must_use]
    pub fn with_native_function_params<I, S>(mut self, name: impl Into<String>, params: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.native_function_params.insert(
            name.into(),
            params
                .into_iter()
                .map(|name| NativeFunctionParam { name: name.into() })
                .collect(),
        );
        self
    }

    #[must_use]
    pub fn with_host_method_for_type(
        mut self,
        type_name: impl Into<String>,
        name: impl Into<String>,
        method: HostMethodId,
    ) -> Self {
        let type_name = type_name.into();
        self.host_types.insert(type_name.clone());
        self.host_methods_by_type
            .insert((type_name, name.into()), method);
        self
    }

    pub(super) fn host_method(
        &self,
        receiver_type: Option<&str>,
        name: &str,
    ) -> Option<HostMethodId> {
        receiver_type
            .and_then(|type_name| {
                self.host_methods_by_type
                    .get(&(type_name.to_owned(), name.to_owned()))
            })
            .copied()
            .or_else(|| self.host_methods.get(name).copied())
    }

    pub(super) fn is_native_module_root(&self, root: &str) -> bool {
        self.native_module_roots.contains(root)
    }

    pub(super) fn host_method_params(&self, method: HostMethodId) -> Option<&[HostMethodParam]> {
        self.host_method_params.get(&method).map(Vec::as_slice)
    }

    pub(super) fn value_method_params(&self, method: &str) -> Option<&[ValueMethodParam]> {
        self.value_method_params.get(method).map(Vec::as_slice)
    }

    pub(super) fn value_method_params_for_type(
        &self,
        type_name: &str,
        method: &str,
    ) -> Option<&[ValueMethodParam]> {
        self.value_method_params_by_type
            .get(&(type_name.to_owned(), method.to_owned()))
            .map(Vec::as_slice)
    }

    pub(super) fn native_function_params(&self, name: &str) -> Option<&[NativeFunctionParam]> {
        self.native_function_params.get(name).map(Vec::as_slice)
    }
}
