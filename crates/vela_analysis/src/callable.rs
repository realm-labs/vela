//! Backend-neutral callable signatures shared by source and registry facts.

use vela_common::{CallableAsyncness, Span};

use crate::type_fact::TypeFact;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableParameterRequirementFact {
    Required,
    Defaulted,
}

impl CallableParameterRequirementFact {
    #[must_use]
    pub const fn is_required(self) -> bool {
        matches!(self, Self::Required)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableParameterFact {
    pub name: String,
    pub type_fact: TypeFact,
    pub requirement: CallableParameterRequirementFact,
    pub declaration_span: Option<Span>,
}

impl CallableParameterFact {
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        type_fact: TypeFact,
        requirement: CallableParameterRequirementFact,
    ) -> Self {
        Self {
            name: name.into(),
            type_fact,
            requirement,
            declaration_span: None,
        }
    }

    #[must_use]
    pub const fn declared_at(mut self, span: Span) -> Self {
        self.declaration_span = Some(span);
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableSignatureFact {
    pub asyncness: CallableAsyncness,
    pub parameters: Vec<CallableParameterFact>,
    pub returns: TypeFact,
}

impl CallableSignatureFact {
    #[must_use]
    pub fn new(
        parameters: impl IntoIterator<Item = CallableParameterFact>,
        returns: TypeFact,
    ) -> Self {
        Self {
            asyncness: CallableAsyncness::Sync,
            parameters: parameters.into_iter().collect(),
            returns,
        }
    }

    #[must_use]
    pub const fn asyncness(mut self, asyncness: CallableAsyncness) -> Self {
        self.asyncness = asyncness;
        self
    }
}
