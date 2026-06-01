use vela_hir::attributes::HirAttribute;

use crate::registry::AttrMap;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ReflectedScriptAttrs {
    pub attrs: AttrMap,
    pub docs: Option<String>,
}

impl ReflectedScriptAttrs {
    pub(crate) fn from_hir(attrs: &[HirAttribute]) -> Self {
        let mut reflected = Self::default();
        for attr in attrs {
            if attr.name == "doc" {
                reflected.docs = Some(attr.string_value().to_owned());
            } else if !attr.name.is_empty() {
                reflected
                    .attrs
                    .insert(attr.name.clone(), attr.string_value());
            }
        }
        reflected
    }
}
