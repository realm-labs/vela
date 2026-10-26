use proc_macro2::Ident;
use syn::Result;

use crate::attrs::error;

pub(crate) struct TypeIdentity {
    pub(crate) name: String,
    pub(crate) module: String,
    pub(crate) stable_path: String,
    pub(crate) type_id: u128,
}

pub(crate) fn type_identity(
    ident: &Ident,
    path: Option<String>,
    module_attr: Option<String>,
    name_attr: Option<String>,
    alias: Option<String>,
) -> Result<TypeIdentity> {
    let (module, name) = match path {
        Some(path) => {
            let (module, path_name) = split_type_path(&path, ident)?;
            if let Some(module_attr) = module_attr
                && module_attr != module
            {
                return Err(error(ident.span(), "script path and module disagree"));
            }
            if let Some(name_attr) = name_attr
                && name_attr != path_name
            {
                return Err(error(ident.span(), "script path and name disagree"));
            }
            (module, path_name)
        }
        None => {
            let module = module_attr.ok_or_else(|| {
                error(
                    ident.span(),
                    "Value requires #[vela(path = \"module::Type\")] or #[vela(module = \"module\")]",
                )
            })?;
            let name = name_attr.unwrap_or_else(|| ident.to_string());
            if name.is_empty() {
                return Err(error(ident.span(), "script type name cannot be empty"));
            }
            (module, name)
        }
    };
    let current_path = format!("{module}::{name}");
    let stable_path = alias
        .map(|alias| {
            if alias.contains("::") {
                alias
            } else {
                format!("{module}::{alias}")
            }
        })
        .unwrap_or(current_path);
    // Keep existing Value identities stable while removing host-only metadata.
    let type_id = u128::from(vela_common::stable_id("host_type", "", &stable_path));
    Ok(TypeIdentity {
        name,
        module,
        stable_path,
        type_id,
    })
}

fn split_type_path(path: &str, ident: &Ident) -> Result<(String, String)> {
    let Some((module, name)) = path.rsplit_once("::") else {
        return Err(error(
            ident.span(),
            "script path must include a module and type name",
        ));
    };
    if module.is_empty() || name.is_empty() {
        return Err(error(
            ident.span(),
            "script path must include a module and type name",
        ));
    }
    Ok((module.to_owned(), name.to_owned()))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::type_identity;

    #[test]
    fn value_identity_keeps_existing_stable_aliases_without_host_ids() {
        let ident = syn::parse_quote!(Renamed);
        let identity = type_identity(
            &ident,
            Some("data::Renamed".to_owned()),
            None,
            None,
            Some("Original".to_owned()),
        )
        .expect("value identity");
        assert_eq!(identity.name, "Renamed");
        assert_eq!(identity.module, "data");
        assert_eq!(identity.stable_path, "data::Original");
        assert_eq!(
            identity.type_id,
            u128::from(vela_common::stable_id("host_type", "", "data::Original",))
        );
    }

    #[rstest]
    #[case::missing_module(
        None,
        None,
        None,
        "Value requires #[vela(path = \"module::Type\")] or #[vela(module = \"module\")]"
    )]
    #[case::inconsistent_module(
        Some("data::Record"),
        Some("other"),
        None,
        "script path and module disagree"
    )]
    #[case::inconsistent_name(
        Some("data::Record"),
        None,
        Some("Other"),
        "script path and name disagree"
    )]
    #[case::unqualified_path(
        Some("Record"),
        None,
        None,
        "script path must include a module and type name"
    )]
    fn value_identity_requires_an_explicit_module_and_consistent_names(
        #[case] path: Option<&str>,
        #[case] module: Option<&str>,
        #[case] name: Option<&str>,
        #[case] expected: &str,
    ) {
        let ident = syn::parse_quote!(Record);
        let error = match type_identity(
            &ident,
            path.map(str::to_owned),
            module.map(str::to_owned),
            name.map(str::to_owned),
            None,
        ) {
            Ok(_) => panic!("invalid Value identity must fail"),
            Err(error) => error,
        };
        assert_eq!(error.to_string(), expected);
    }

    #[test]
    fn value_identity_accepts_an_explicit_module_with_the_rust_name() {
        let ident = syn::parse_quote!(Record);
        let identity = type_identity(&ident, None, Some("data".to_owned()), None, None)
            .expect("module-local Value identity");
        assert_eq!(identity.stable_path, "data::Record");
    }
}
