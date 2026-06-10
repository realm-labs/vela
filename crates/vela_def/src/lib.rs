//! Stable semantic definition identity for Vela.

use std::fmt;

const HASH_VERSION_PREFIX: &str = "vela-def-v1";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefKind {
    Function,
    Method,
    Type,
    Field,
    Variant,
    Trait,
    Module,
    Global,
}

impl DefKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Method => "method",
            Self::Type => "type",
            Self::Field => "field",
            Self::Variant => "variant",
            Self::Trait => "trait",
            Self::Module => "module",
            Self::Global => "global",
        }
    }
}

impl fmt::Display for DefKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefPath {
    pub package: String,
    pub module: Vec<String>,
    pub owner: Option<String>,
    pub name: String,
    pub kind: DefKind,
}

impl DefPath {
    #[must_use]
    pub fn new(
        kind: DefKind,
        package: impl Into<String>,
        module: impl IntoIterator<Item = impl Into<String>>,
        owner: Option<impl Into<String>>,
        name: impl Into<String>,
    ) -> Self {
        Self {
            package: package.into(),
            module: module.into_iter().map(Into::into).collect(),
            owner: owner.map(Into::into),
            name: name.into(),
            kind,
        }
    }

    #[must_use]
    pub fn function(
        package: impl Into<String>,
        module: impl IntoIterator<Item = impl Into<String>>,
        name: impl Into<String>,
    ) -> Self {
        Self::new(DefKind::Function, package, module, None::<String>, name)
    }

    #[must_use]
    pub fn method(
        package: impl Into<String>,
        module: impl IntoIterator<Item = impl Into<String>>,
        owner: impl Into<String>,
        name: impl Into<String>,
    ) -> Self {
        Self::new(DefKind::Method, package, module, Some(owner), name)
    }

    #[must_use]
    pub fn ty(
        package: impl Into<String>,
        module: impl IntoIterator<Item = impl Into<String>>,
        name: impl Into<String>,
    ) -> Self {
        Self::new(DefKind::Type, package, module, None::<String>, name)
    }

    #[must_use]
    pub fn field(
        package: impl Into<String>,
        module: impl IntoIterator<Item = impl Into<String>>,
        owner: impl Into<String>,
        name: impl Into<String>,
    ) -> Self {
        Self::new(DefKind::Field, package, module, Some(owner), name)
    }

    #[must_use]
    pub fn variant(
        package: impl Into<String>,
        module: impl IntoIterator<Item = impl Into<String>>,
        owner: impl Into<String>,
        name: impl Into<String>,
    ) -> Self {
        Self::new(DefKind::Variant, package, module, Some(owner), name)
    }

    #[must_use]
    pub fn trait_def(
        package: impl Into<String>,
        module: impl IntoIterator<Item = impl Into<String>>,
        name: impl Into<String>,
    ) -> Self {
        Self::new(DefKind::Trait, package, module, None::<String>, name)
    }

    #[must_use]
    pub fn module(
        package: impl Into<String>,
        module: impl IntoIterator<Item = impl Into<String>>,
        name: impl Into<String>,
    ) -> Self {
        Self::new(DefKind::Module, package, module, None::<String>, name)
    }

    #[must_use]
    pub fn global(
        package: impl Into<String>,
        module: impl IntoIterator<Item = impl Into<String>>,
        name: impl Into<String>,
    ) -> Self {
        Self::new(DefKind::Global, package, module, None::<String>, name)
    }

    #[must_use]
    pub fn canonical_module(&self) -> String {
        self.module.join("::")
    }

    #[must_use]
    pub fn canonical_name(&self) -> String {
        let mut parts = Vec::new();
        parts.push(self.package.as_str());
        parts.extend(self.module.iter().map(String::as_str));
        if let Some(owner) = self.owner.as_deref() {
            parts.extend(owner.split("::").filter(|part| !part.is_empty()));
        }
        parts.push(self.name.as_str());
        parts.join("::")
    }

    #[must_use]
    pub fn canonical_display(&self) -> String {
        format!("{} {}", self.kind, self.canonical_name())
    }

    #[must_use]
    pub fn canonical_hash_input(&self) -> Vec<u8> {
        let mut input = Vec::new();
        push_field(&mut input, HASH_VERSION_PREFIX, "");
        push_field(&mut input, "kind=", self.kind.as_str());
        push_field(&mut input, "package=", &self.package);
        push_field(&mut input, "module=", &self.canonical_module());
        push_field(&mut input, "owner=", self.owner.as_deref().unwrap_or(""));
        push_field(&mut input, "name=", &self.name);
        input
    }

    #[must_use]
    pub fn id(&self) -> DefId {
        DefId::from_path(self)
    }
}

impl fmt::Display for DefPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.canonical_display())
    }
}

fn push_field(input: &mut Vec<u8>, prefix: &str, value: &str) {
    input.extend_from_slice(prefix.as_bytes());
    input.extend_from_slice(value.as_bytes());
    input.push(0);
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct DefId(u128);

impl DefId {
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u128 {
        self.0
    }

    #[must_use]
    pub fn from_path(path: &DefPath) -> Self {
        let hash = blake3::hash(&path.canonical_hash_input());
        let mut bytes = [0_u8; 16];
        bytes.copy_from_slice(&hash.as_bytes()[..16]);
        Self(u128::from_le_bytes(bytes))
    }
}

macro_rules! typed_def_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[repr(transparent)]
        pub struct $name(DefId);

        impl $name {
            #[must_use]
            pub const fn new(id: DefId) -> Self {
                Self(id)
            }

            #[must_use]
            pub const fn def_id(self) -> DefId {
                self.0
            }

            #[must_use]
            pub const fn get(self) -> u128 {
                self.0.get()
            }
        }

        impl From<DefId> for $name {
            fn from(id: DefId) -> Self {
                Self(id)
            }
        }

        impl From<$name> for DefId {
            fn from(id: $name) -> Self {
                id.0
            }
        }
    };
}

typed_def_id!(FunctionId);
typed_def_id!(MethodId);
typed_def_id!(TypeId);
typed_def_id!(FieldId);
typed_def_id!(VariantId);
typed_def_id!(TraitId);
typed_def_id!(GlobalId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_path_generates_same_id() {
        let first = DefPath::function("std", ["math"], "max");
        let second = DefPath::function("std", ["math"], "max");

        assert_eq!(first.id(), second.id());
    }

    #[test]
    fn different_kinds_generate_different_ids() {
        let function = DefPath::function("std", ["option"], "Some");
        let variant = DefPath::variant("std", std::iter::empty::<&str>(), "Option", "Some");

        assert_ne!(function.id(), variant.id());
    }

    #[test]
    fn different_owners_generate_different_ids() {
        let string_len = DefPath::method("std", std::iter::empty::<&str>(), "String", "len");
        let array_len = DefPath::method("std", std::iter::empty::<&str>(), "Array", "len");

        assert_ne!(string_len.id(), array_len.id());
    }

    #[test]
    fn canonical_path_formatting_is_stable() {
        let function = DefPath::function("std", ["math"], "max");
        let method = DefPath::method("std", std::iter::empty::<&str>(), "String", "len");
        let ty = DefPath::ty("std", std::iter::empty::<&str>(), "String");
        let variant = DefPath::variant("std", std::iter::empty::<&str>(), "Option", "Some");
        let field = DefPath::field("std", std::iter::empty::<&str>(), "Option::Some", "0");

        assert_eq!(function.to_string(), "function std::math::max");
        assert_eq!(method.to_string(), "method std::String::len");
        assert_eq!(ty.to_string(), "type std::String");
        assert_eq!(variant.to_string(), "variant std::Option::Some");
        assert_eq!(field.to_string(), "field std::Option::Some::0");
    }

    #[test]
    fn hash_input_format_is_stable() {
        let path = DefPath::function("std", ["math"], "max");
        let input = String::from_utf8_lossy(&path.canonical_hash_input()).replace('\0', "\\0\n");

        assert_eq!(
            input,
            "vela-def-v1\\0\nkind=function\\0\npackage=std\\0\nmodule=math\\0\nowner=\\0\nname=max\\0\n"
        );
    }

    #[test]
    fn blake3_128_fixture_outputs_are_stable() {
        let fixtures = [
            (
                DefPath::function("std", ["math"], "max"),
                0xc6ac_3d50_8a8f_690e_3b98_5285_b625_0f8c_u128,
            ),
            (
                DefPath::method("std", std::iter::empty::<&str>(), "String", "len"),
                0x2f1e_04cc_e9ba_9207_77d9_cb96_0411_39ce_u128,
            ),
            (
                DefPath::variant("std", std::iter::empty::<&str>(), "Option", "Some"),
                0xb0aa_d415_2d15_dd2b_2479_0cc5_378f_d2fc_u128,
            ),
            (
                DefPath::field("std", std::iter::empty::<&str>(), "Option::Some", "0"),
                0x2863_5079_24a3_8385_d163_4874_7f26_3595_u128,
            ),
        ];

        for (path, expected) in fixtures {
            assert_eq!(path.id().get(), expected, "{path}");
        }
    }

    #[test]
    fn typed_wrappers_preserve_def_id() {
        let id = DefPath::trait_def("script", ["combat"], "Scored").id();
        let trait_id = TraitId::new(id);

        assert_eq!(trait_id.def_id(), id);
        assert_eq!(DefId::from(trait_id), id);
    }
}
