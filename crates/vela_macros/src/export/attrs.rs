use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use syn::{LitStr, Result, parse::Parser};

use crate::attrs::parse_qualified_name;

use super::signature::EffectName;

#[derive(Clone, Debug)]
pub(super) struct ExportAttrs {
    pub(super) path: String,
    pub(super) effects: BTreeSet<EffectName>,
    pub(super) docs: Option<String>,
}

impl ExportAttrs {
    pub(super) fn parse(tokens: TokenStream) -> Result<Self> {
        let mut path = None;
        let mut effects = BTreeSet::new();
        let mut docs = None;
        let parser = syn::meta::parser(|meta| {
            if meta.path.is_ident("path") {
                if path.is_some() {
                    return Err(meta.error("duplicate export path"));
                }
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
