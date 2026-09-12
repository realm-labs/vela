use super::{SchemaScopedResourceReturn, SchemaTypeFact, signature::SchemaCallableSignature};
use serde::{Deserialize, Serialize};
use vela_analysis::{
    registry::{
        RegistryFunctionFact, RegistryMemberFact, RegistryModuleFact, ScopedResourceReturnDef,
    },
    type_fact::TypeFact,
};
use vela_common::{SourceId, Span};

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SchemaSourceSpan {
    pub(super) source: u32,
    pub(super) start: u32,
    pub(super) end: u32,
}

impl SchemaSourceSpan {
    pub(super) fn from_span(span: Span) -> Self {
        Self {
            source: span.source.get(),
            start: span.start,
            end: span.end,
        }
    }

    pub(super) fn to_span(self) -> Option<Span> {
        (self.start <= self.end)
            .then(|| Span::new(SourceId::new(self.source), self.start, self.end))
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct SchemaNamedFact {
    pub(super) name: String,
    pub(super) fact: SchemaTypeFact,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) docs: Option<String>,
    #[serde(
        default,
        rename = "sourceSpan",
        alias = "source_span",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) source_span: Option<SchemaSourceSpan>,
}

impl SchemaNamedFact {
    pub(super) fn new(name: impl Into<String>, fact: &TypeFact, docs: Option<&str>) -> Self {
        Self {
            name: name.into(),
            fact: SchemaTypeFact::from_type_fact(fact),
            docs: docs.map(str::to_owned),
            source_span: None,
        }
    }

    pub(super) fn from_registry_module(value: RegistryModuleFact) -> Self {
        Self {
            name: value.name,
            fact: SchemaTypeFact::from_type_fact(&value.fact),
            docs: value.docs,
            source_span: value.source_span.map(SchemaSourceSpan::from_span),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct SchemaMemberFact {
    pub(super) owner: String,
    pub(super) name: String,
    pub(super) fact: SchemaTypeFact,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) signature: Option<SchemaCallableSignature>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) docs: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) scoped_resource: Option<SchemaScopedResourceReturn>,
    #[serde(
        default,
        rename = "sourceSpan",
        alias = "source_span",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) source_span: Option<SchemaSourceSpan>,
}

impl SchemaMemberFact {
    pub(super) fn with_signature(
        mut self,
        signature: Option<&vela_analysis::registry::CallableSignatureFact>,
    ) -> Self {
        self.signature = signature.map(SchemaCallableSignature::from_registry);
        self
    }

    pub(super) fn from_registry_member(
        value: RegistryMemberFact,
        docs: Option<&str>,
        scoped_resource: Option<ScopedResourceReturnDef>,
    ) -> Self {
        Self {
            owner: value.owner,
            name: value.name,
            fact: SchemaTypeFact::from_type_fact(&value.fact),
            signature: None,
            docs: docs.map(str::to_owned),
            scoped_resource: scoped_resource.map(Into::into),
            source_span: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct SchemaFunctionFact {
    pub(super) name: String,
    pub(super) fact: SchemaTypeFact,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) signature: Option<SchemaCallableSignature>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) docs: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) scoped_resource: Option<SchemaScopedResourceReturn>,
    #[serde(
        default,
        rename = "sourceSpan",
        alias = "source_span",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) source_span: Option<SchemaSourceSpan>,
}

impl SchemaFunctionFact {
    pub(super) fn with_signature(
        mut self,
        signature: Option<&vela_analysis::registry::CallableSignatureFact>,
    ) -> Self {
        self.signature = signature.map(SchemaCallableSignature::from_registry);
        self
    }

    pub(super) fn from_registry_function(
        value: RegistryFunctionFact,
        docs: Option<&str>,
        scoped_resource: Option<ScopedResourceReturnDef>,
    ) -> Self {
        Self {
            name: value.name,
            fact: SchemaTypeFact::from_type_fact(&value.fact),
            signature: None,
            docs: docs.map(str::to_owned),
            scoped_resource: scoped_resource.map(Into::into),
            source_span: None,
        }
    }
}
