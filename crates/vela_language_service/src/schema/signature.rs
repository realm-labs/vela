use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use vela_analysis::registry::{
    CallableParameterFact, CallableParameterRequirementFact, CallableSignatureFact,
};
use vela_analysis::type_fact::TypeFact;
use vela_common::CallableAsyncness;

use super::{SchemaArtifactError, SchemaArtifactFacts, SchemaSourceSpan, SchemaTypeFact};

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SchemaCallableSignature {
    asyncness: SchemaAsyncness,
    parameters: Vec<SchemaCallableParameter>,
    returns: SchemaTypeFact,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SchemaCallableParameter {
    name: String,
    type_fact: SchemaTypeFact,
    requirement: SchemaParameterRequirement,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_span: Option<SchemaSourceSpan>,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SchemaAsyncness {
    Sync,
    Async,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SchemaParameterRequirement {
    Required,
    Defaulted,
}

impl SchemaCallableSignature {
    pub(super) fn from_registry(signature: &CallableSignatureFact) -> Self {
        Self {
            asyncness: match signature.asyncness {
                CallableAsyncness::Sync => SchemaAsyncness::Sync,
                CallableAsyncness::Async => SchemaAsyncness::Async,
            },
            parameters: signature
                .parameters
                .iter()
                .map(|parameter| SchemaCallableParameter {
                    name: parameter.name.clone(),
                    type_fact: SchemaTypeFact::from_type_fact(&parameter.type_fact),
                    requirement: match parameter.requirement {
                        CallableParameterRequirementFact::Required => {
                            SchemaParameterRequirement::Required
                        }
                        CallableParameterRequirementFact::Defaulted => {
                            SchemaParameterRequirement::Defaulted
                        }
                    },
                    source_span: parameter.declaration_span.map(SchemaSourceSpan::from_span),
                })
                .collect(),
            returns: SchemaTypeFact::from_type_fact(&signature.returns),
        }
    }

    pub(super) fn to_registry(&self) -> CallableSignatureFact {
        CallableSignatureFact {
            asyncness: match self.asyncness {
                SchemaAsyncness::Sync => CallableAsyncness::Sync,
                SchemaAsyncness::Async => CallableAsyncness::Async,
            },
            parameters: self
                .parameters
                .iter()
                .map(|parameter| CallableParameterFact {
                    name: parameter.name.clone(),
                    type_fact: parameter.type_fact.to_type_fact(),
                    requirement: match parameter.requirement {
                        SchemaParameterRequirement::Required => {
                            CallableParameterRequirementFact::Required
                        }
                        SchemaParameterRequirement::Defaulted => {
                            CallableParameterRequirementFact::Defaulted
                        }
                    },
                    declaration_span: parameter.source_span.and_then(SchemaSourceSpan::to_span),
                })
                .collect(),
            returns: self.returns.to_type_fact(),
        }
    }

    fn validate(&self, fact: &SchemaTypeFact, name: &str) -> Result<(), SchemaArtifactError> {
        let signature = self.to_registry();
        let function = TypeFact::function(
            signature
                .parameters
                .iter()
                .map(|p| p.type_fact.clone())
                .collect(),
            signature.returns,
        );
        if function != fact.to_type_fact() {
            return Err(SchemaArtifactError::new(format!(
                "callable signature types disagree with fact for {name}"
            )));
        }
        let mut names = BTreeSet::new();
        for parameter in &self.parameters {
            if parameter.name.trim().is_empty() || !names.insert(&parameter.name) {
                return Err(SchemaArtifactError::new(format!(
                    "empty or duplicate callable parameter name for {name}"
                )));
            }
            if parameter
                .source_span
                .is_some_and(|span| span.to_span().is_none())
            {
                return Err(SchemaArtifactError::new(format!(
                    "invalid callable parameter source span for {name}"
                )));
            }
        }
        Ok(())
    }
}

pub(super) fn validate_signatures(facts: &SchemaArtifactFacts) -> Result<(), SchemaArtifactError> {
    for function in &facts.functions {
        if let Some(signature) = &function.signature {
            signature.validate(&function.fact, &function.name)?;
        }
    }
    for member in facts.methods.iter().chain(&facts.trait_methods) {
        if let Some(signature) = &member.signature {
            signature.validate(&member.fact, &format!("{}::{}", member.owner, member.name))?;
        }
    }
    if facts
        .fields
        .iter()
        .chain(&facts.variants)
        .any(|member| member.signature.is_some())
    {
        return Err(SchemaArtifactError::new(
            "callable signatures are only valid on functions and methods",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
