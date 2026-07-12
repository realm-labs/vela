use std::fmt;
use std::sync::Arc;

macro_rules! validated_identity {
    ($name:ident, $kind:literal, $validator:ident) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(Arc<str>);

        impl $name {
            pub fn new(value: impl AsRef<str>) -> Result<Self, IdentityError> {
                let value = value.as_ref();
                if !$validator(value) {
                    return Err(IdentityError::new($kind, value));
                }
                Ok(Self(Arc::from(value)))
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

validated_identity!(PackageId, "package id", valid_package_id);
validated_identity!(PackageName, "package name", valid_identifier);
validated_identity!(PackageAlias, "package alias", valid_identifier);
validated_identity!(PackageVersion, "package version", valid_version);

impl PackageId {
    #[must_use]
    pub fn anonymous() -> Self {
        Self(Arc::from("dev.vela.anonymous"))
    }

    #[must_use]
    pub fn scratch() -> Self {
        Self(Arc::from("dev.vela.scratch"))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityError {
    kind: &'static str,
    value: String,
}

impl IdentityError {
    fn new(kind: &'static str, value: &str) -> Self {
        Self {
            kind,
            value: value.to_owned(),
        }
    }
}

impl fmt::Display for IdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid {} `{}`", self.kind, self.value)
    }
}

impl std::error::Error for IdentityError {}

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ModulePath(Vec<String>);

impl ModulePath {
    #[must_use]
    pub fn root() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn new(segments: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self(segments.into_iter().map(Into::into).collect())
    }

    #[must_use]
    pub fn from_qualified(path: &str) -> Self {
        Self::new(path.split("::").filter(|segment| !segment.is_empty()))
    }

    #[must_use]
    pub fn segments(&self) -> &[String] {
        &self.0
    }

    #[must_use]
    pub fn join(&self) -> String {
        self.0.join("::")
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ModuleKey {
    pub package: PackageId,
    pub path: ModulePath,
}

impl ModuleKey {
    #[must_use]
    pub const fn new(package: PackageId, path: ModulePath) -> Self {
        Self { package, path }
    }
}

fn valid_package_id(value: &str) -> bool {
    !value.is_empty()
        && value.split('.').all(valid_identifier)
        && value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
}

fn valid_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

fn valid_version(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_types_validate_canonical_inputs() {
        assert!(PackageId::new("com.example.tools").is_ok());
        assert!(PackageId::new("Example Tools").is_err());
        assert!(PackageAlias::new("text_utils").is_ok());
        assert!(PackageAlias::new("1text").is_err());
    }
}
