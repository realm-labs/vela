use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use syn::{LitStr, Result, parse::Parser};

use crate::attrs::parse_qualified_name;

use super::signature::EffectName;

#[derive(Clone, Debug)]
pub(crate) struct ExportAttrs {
    pub(crate) path: String,
    pub(crate) effects: BTreeSet<EffectName>,
    pub(crate) docs: Option<String>,
}

impl ExportAttrs {
    pub(crate) fn parse(tokens: TokenStream) -> Result<Self> {
        Self::parse_with_default(tokens, None)
    }

    pub(crate) fn parse_with_default(
        tokens: TokenStream,
        default_path: Option<String>,
    ) -> Result<Self> {
        let mut path = default_path;
        let mut explicit_path = false;
        let mut effects = BTreeSet::new();
        let mut docs = None;
        let parser = syn::meta::parser(|meta| {
            if meta.path.is_ident("path") {
                if explicit_path {
                    return Err(meta.error("duplicate export path"));
                }
                explicit_path = true;
                path = Some(parse_qualified_name(
                    meta.value()?.parse::<LitStr>()?,
                    "export path",
                )?);
                return Ok(());
            }
            if meta.path.is_ident("docs") {
                if docs.is_some() {
                    return Err(meta.error("duplicate export docs"));
                }
                docs = Some(meta.value()?.parse::<LitStr>()?.value());
                return Ok(());
            }
            if meta.path.is_ident("effects") {
                return meta.parse_nested_meta(|effect| {
                    let Some(ident) = effect.path.get_ident() else {
                        return Err(effect.error("effect must be an identifier"));
                    };
                    let effect_name = EffectName::parse(ident)?;
                    if effect_name == EffectName::Pure {
                        return Err(effect.error(
                            "effects(...) only adds effects; `pure` cannot remove an inferred host effect",
                        ));
                    }
                    if !effects.insert(effect_name) {
                        return Err(effect.error("duplicate additional effect"));
                    }
                    Ok(())
                });
            }
            Err(meta.error("unsupported export attribute"))
        });
        parser.parse2(tokens)?;
        let path = path.ok_or_else(|| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "#[vela::export] requires path = \"module::function\"",
            )
        })?;
        Ok(Self {
            path,
            effects,
            docs,
        })
    }
}
