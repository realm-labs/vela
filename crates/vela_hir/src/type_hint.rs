use vela_common::Span;
use vela_syntax::{Param, StructField, TypeHint};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirTypeHint {
    pub path: Vec<String>,
    pub span: Span,
}

impl HirTypeHint {
    #[must_use]
    pub fn from_syntax(hint: &TypeHint) -> Self {
        Self {
            path: hint.path.clone(),
            span: hint.span,
        }
    }

    #[must_use]
    pub fn display(&self) -> String {
        self.path.join(".")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamHint {
    pub name: String,
    pub type_hint: Option<HirTypeHint>,
}

impl ParamHint {
    #[must_use]
    pub fn from_syntax(param: &Param) -> Self {
        Self {
            name: param.name.clone(),
            type_hint: param.type_hint.as_ref().map(HirTypeHint::from_syntax),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionSignature {
    pub params: Vec<ParamHint>,
    pub return_type: Option<HirTypeHint>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructFieldHint {
    pub name: String,
    pub type_hint: Option<HirTypeHint>,
}

impl StructFieldHint {
    #[must_use]
    pub fn from_syntax(field: &StructField) -> Self {
        Self {
            name: field.name.clone(),
            type_hint: field.type_hint.as_ref().map(HirTypeHint::from_syntax),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructShape {
    pub fields: Vec<StructFieldHint>,
}
