use serde::{Deserialize, Serialize};
use vela_analysis::registry::{
    RegistryEffectFact, RegistryFacts, RegistryFieldAccessFact, RegistryFunctionFact,
    RegistryIndexCapabilityFact, RegistryMemberFact, RegistryMethodAccessFact,
};
use vela_analysis::type_fact::TypeFact;
use vela_common::PrimitiveTag;

pub const SCHEMA_ARTIFACT_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SchemaArtifactError {
    message: String,
}

impl SchemaArtifactError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaArtifact {
    format_version: u32,
    #[serde(default)]
    schema_version: Option<String>,
    #[serde(default)]
    schema_hash: Option<String>,
    #[serde(default)]
    facts: SchemaArtifactFacts,
}

impl SchemaArtifact {
    #[must_use]
    pub fn new(facts: SchemaArtifactFacts) -> Self {
        Self {
            format_version: SCHEMA_ARTIFACT_FORMAT_VERSION,
            schema_version: None,
            schema_hash: None,
            facts,
        }
    }

    #[must_use]
    pub fn from_registry_facts(facts: &RegistryFacts) -> Self {
        Self::new(SchemaArtifactFacts::from_registry_facts(facts))
    }

    pub fn from_json(source: &str) -> Result<Self, SchemaArtifactError> {
        let artifact = serde_json::from_str::<Self>(source).map_err(|error| {
            SchemaArtifactError::new(format!("invalid schema artifact: {error}"))
        })?;
        artifact.validate()?;
        Ok(artifact)
    }

    pub fn to_json(&self) -> Result<String, SchemaArtifactError> {
        serde_json::to_string_pretty(self).map_err(|error| {
            SchemaArtifactError::new(format!("failed to encode schema artifact: {error}"))
        })
    }

    pub fn to_registry_facts(&self) -> RegistryFacts {
        self.facts.to_registry_facts()
    }

    fn validate(&self) -> Result<(), SchemaArtifactError> {
        if self.format_version != SCHEMA_ARTIFACT_FORMAT_VERSION {
            return Err(SchemaArtifactError::new(format!(
                "unsupported schema artifact format version {}; expected {}",
                self.format_version, SCHEMA_ARTIFACT_FORMAT_VERSION
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaArtifactFacts {
    #[serde(default)]
    types: Vec<SchemaNamedFact>,
    #[serde(default)]
    traits: Vec<SchemaNamedFact>,
    #[serde(default)]
    fields: Vec<SchemaMemberFact>,
    #[serde(default)]
    field_access: Vec<SchemaFieldAccessFact>,
    #[serde(default)]
    variants: Vec<SchemaMemberFact>,
    #[serde(default)]
    methods: Vec<SchemaMemberFact>,
    #[serde(default)]
    method_effects: Vec<SchemaMemberEffectFact>,
    #[serde(default)]
    method_access: Vec<SchemaMethodAccessFact>,
    #[serde(default)]
    trait_methods: Vec<SchemaMemberFact>,
    #[serde(default)]
    trait_method_effects: Vec<SchemaMemberEffectFact>,
    #[serde(default)]
    functions: Vec<SchemaFunctionFact>,
    #[serde(default)]
    function_effects: Vec<SchemaFunctionEffectFact>,
    #[serde(default)]
    index_capabilities: Vec<SchemaIndexCapabilityFact>,
}

impl SchemaArtifactFacts {
    #[must_use]
    pub fn from_registry_facts(facts: &RegistryFacts) -> Self {
        Self {
            types: facts
                .types()
                .map(|(name, fact)| SchemaNamedFact::new(name, fact))
                .collect(),
            traits: facts
                .traits()
                .map(|(name, fact)| SchemaNamedFact::new(name, fact))
                .collect(),
            fields: facts.fields().map(SchemaMemberFact::from).collect(),
            field_access: facts
                .field_accesses()
                .map(SchemaFieldAccessFact::from)
                .collect(),
            variants: facts.variants().map(SchemaMemberFact::from).collect(),
            methods: facts.methods().map(SchemaMemberFact::from).collect(),
            method_effects: facts
                .method_effects()
                .map(SchemaMemberEffectFact::from)
                .collect(),
            method_access: facts
                .method_accesses()
                .map(SchemaMethodAccessFact::from)
                .collect(),
            trait_methods: facts.trait_methods().map(SchemaMemberFact::from).collect(),
            trait_method_effects: facts
                .trait_method_effects()
                .map(SchemaMemberEffectFact::from)
                .collect(),
            functions: facts.functions().map(SchemaFunctionFact::from).collect(),
            function_effects: facts
                .function_effects()
                .map(|(name, effect)| SchemaFunctionEffectFact::new(name, effect))
                .collect(),
            index_capabilities: facts
                .index_capabilities()
                .map(SchemaIndexCapabilityFact::from)
                .collect(),
        }
    }

    fn to_registry_facts(&self) -> RegistryFacts {
        let mut facts = RegistryFacts::default();
        for entry in &self.types {
            facts.insert_type(entry.name.clone(), entry.fact.to_type_fact());
        }
        for entry in &self.traits {
            facts.insert_trait(entry.name.clone(), entry.fact.to_type_fact());
        }
        for entry in &self.fields {
            facts.insert_field(
                entry.owner.clone(),
                entry.name.clone(),
                entry.fact.to_type_fact(),
            );
        }
        for access in &self.field_access {
            facts.insert_field_access(access.to_registry_fact());
        }
        for entry in &self.variants {
            facts.insert_variant(
                entry.owner.clone(),
                entry.name.clone(),
                entry.fact.to_type_fact(),
            );
        }
        for entry in &self.methods {
            facts.insert_method(
                entry.owner.clone(),
                entry.name.clone(),
                entry.fact.to_type_fact(),
            );
        }
        for effect in &self.method_effects {
            facts.insert_method_effect(
                effect.owner.clone(),
                effect.name.clone(),
                effect.effect.to_registry_fact(),
            );
        }
        for access in &self.method_access {
            facts.insert_method_access(access.to_registry_fact());
        }
        for entry in &self.trait_methods {
            facts.insert_trait_method(
                entry.owner.clone(),
                entry.name.clone(),
                entry.fact.to_type_fact(),
            );
        }
        for effect in &self.trait_method_effects {
            facts.insert_trait_method_effect(
                effect.owner.clone(),
                effect.name.clone(),
                effect.effect.to_registry_fact(),
            );
        }
        for entry in &self.functions {
            facts.insert_function(entry.name.clone(), entry.fact.to_type_fact());
        }
        for effect in &self.function_effects {
            facts.insert_function_effect(effect.name.clone(), effect.effect.to_registry_fact());
        }
        for capability in &self.index_capabilities {
            facts.insert_index_capability(capability.to_registry_fact());
        }
        facts
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
struct SchemaNamedFact {
    name: String,
    fact: SchemaTypeFact,
}

impl SchemaNamedFact {
    fn new(name: impl Into<String>, fact: &TypeFact) -> Self {
        Self {
            name: name.into(),
            fact: SchemaTypeFact::from_type_fact(fact),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
struct SchemaMemberFact {
    owner: String,
    name: String,
    fact: SchemaTypeFact,
}

impl From<RegistryMemberFact> for SchemaMemberFact {
    fn from(value: RegistryMemberFact) -> Self {
        Self {
            owner: value.owner,
            name: value.name,
            fact: SchemaTypeFact::from_type_fact(&value.fact),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
struct SchemaFunctionFact {
    name: String,
    fact: SchemaTypeFact,
}

impl From<RegistryFunctionFact> for SchemaFunctionFact {
    fn from(value: RegistryFunctionFact) -> Self {
        Self {
            name: value.name,
            fact: SchemaTypeFact::from_type_fact(&value.fact),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
struct SchemaFieldAccessFact {
    owner: String,
    name: String,
    readable: bool,
    writable: bool,
    reflect_readable: bool,
    reflect_writable: bool,
    #[serde(default)]
    required_permissions: Vec<String>,
}

impl SchemaFieldAccessFact {
    fn to_registry_fact(&self) -> RegistryFieldAccessFact {
        RegistryFieldAccessFact {
            owner: self.owner.clone(),
            name: self.name.clone(),
            readable: self.readable,
            writable: self.writable,
            reflect_readable: self.reflect_readable,
            reflect_writable: self.reflect_writable,
            required_permissions: self.required_permissions.clone(),
        }
    }
}

impl From<RegistryFieldAccessFact> for SchemaFieldAccessFact {
    fn from(value: RegistryFieldAccessFact) -> Self {
        Self {
            owner: value.owner,
            name: value.name,
            readable: value.readable,
            writable: value.writable,
            reflect_readable: value.reflect_readable,
            reflect_writable: value.reflect_writable,
            required_permissions: value.required_permissions,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
struct SchemaMethodAccessFact {
    owner: String,
    name: String,
    public: bool,
    reflect_callable: bool,
    #[serde(default)]
    required_permissions: Vec<String>,
}

impl SchemaMethodAccessFact {
    fn to_registry_fact(&self) -> RegistryMethodAccessFact {
        RegistryMethodAccessFact {
            owner: self.owner.clone(),
            name: self.name.clone(),
            public: self.public,
            reflect_callable: self.reflect_callable,
            required_permissions: self.required_permissions.clone(),
        }
    }
}

impl From<RegistryMethodAccessFact> for SchemaMethodAccessFact {
    fn from(value: RegistryMethodAccessFact) -> Self {
        Self {
            owner: value.owner,
            name: value.name,
            public: value.public,
            reflect_callable: value.reflect_callable,
            required_permissions: value.required_permissions,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
struct SchemaIndexCapabilityFact {
    owner: String,
    readable: bool,
    writable: bool,
    addable: bool,
    removable: bool,
    key: SchemaTypeFact,
    value: SchemaTypeFact,
}

impl SchemaIndexCapabilityFact {
    fn to_registry_fact(&self) -> RegistryIndexCapabilityFact {
        RegistryIndexCapabilityFact {
            owner: self.owner.clone(),
            readable: self.readable,
            writable: self.writable,
            addable: self.addable,
            removable: self.removable,
            key: self.key.to_type_fact(),
            value: self.value.to_type_fact(),
        }
    }
}

impl From<RegistryIndexCapabilityFact> for SchemaIndexCapabilityFact {
    fn from(value: RegistryIndexCapabilityFact) -> Self {
        Self {
            owner: value.owner,
            readable: value.readable,
            writable: value.writable,
            addable: value.addable,
            removable: value.removable,
            key: SchemaTypeFact::from_type_fact(&value.key),
            value: SchemaTypeFact::from_type_fact(&value.value),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
struct SchemaMemberEffectFact {
    owner: String,
    name: String,
    effect: SchemaEffectFact,
}

impl From<(RegistryMemberFact, RegistryEffectFact)> for SchemaMemberEffectFact {
    fn from((member, effect): (RegistryMemberFact, RegistryEffectFact)) -> Self {
        Self {
            owner: member.owner,
            name: member.name,
            effect: SchemaEffectFact::from(effect),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
struct SchemaFunctionEffectFact {
    name: String,
    effect: SchemaEffectFact,
}

impl SchemaFunctionEffectFact {
    fn new(name: impl Into<String>, effect: &RegistryEffectFact) -> Self {
        Self {
            name: name.into(),
            effect: SchemaEffectFact::from(effect.clone()),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SchemaEffectFact {
    reads_host: bool,
    writes_host: bool,
    emits_events: bool,
    reads_time: bool,
    uses_random: bool,
    reads_io: bool,
    writes_io: bool,
    reads_reflection: bool,
    writes_reflection: bool,
    calls_reflection: bool,
}

impl SchemaEffectFact {
    fn to_registry_fact(&self) -> RegistryEffectFact {
        RegistryEffectFact {
            reads_host: self.reads_host,
            writes_host: self.writes_host,
            emits_events: self.emits_events,
            reads_time: self.reads_time,
            uses_random: self.uses_random,
            reads_io: self.reads_io,
            writes_io: self.writes_io,
            reads_reflection: self.reads_reflection,
            writes_reflection: self.writes_reflection,
            calls_reflection: self.calls_reflection,
        }
    }
}

impl From<RegistryEffectFact> for SchemaEffectFact {
    fn from(value: RegistryEffectFact) -> Self {
        Self {
            reads_host: value.reads_host,
            writes_host: value.writes_host,
            emits_events: value.emits_events,
            reads_time: value.reads_time,
            uses_random: value.uses_random,
            reads_io: value.reads_io,
            writes_io: value.writes_io,
            reads_reflection: value.reads_reflection,
            writes_reflection: value.writes_reflection,
            calls_reflection: value.calls_reflection,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum SchemaTypeFact {
    Unknown,
    Never,
    Any,
    Primitive {
        name: String,
    },
    Range,
    Array {
        element: Box<SchemaTypeFact>,
    },
    Map {
        key: Box<SchemaTypeFact>,
        value: Box<SchemaTypeFact>,
    },
    Set {
        element: Box<SchemaTypeFact>,
    },
    Iterator {
        item: Box<SchemaTypeFact>,
    },
    Option {
        some: Box<SchemaTypeFact>,
    },
    OptionSome {
        some: Box<SchemaTypeFact>,
    },
    OptionNone,
    Result {
        ok: Box<SchemaTypeFact>,
        err: Box<SchemaTypeFact>,
    },
    ResultOk {
        ok: Box<SchemaTypeFact>,
    },
    ResultErr {
        err: Box<SchemaTypeFact>,
    },
    Function {
        params: Vec<SchemaTypeFact>,
        returns: Box<SchemaTypeFact>,
    },
    Record {
        name: String,
    },
    Enum {
        name: String,
        variant: Option<String>,
    },
    Host {
        name: String,
    },
    Trait {
        name: String,
    },
    Module {
        name: String,
    },
    Union {
        facts: Vec<SchemaTypeFact>,
    },
}

impl SchemaTypeFact {
    fn from_type_fact(fact: &TypeFact) -> Self {
        match fact {
            TypeFact::Unknown => Self::Unknown,
            TypeFact::Never => Self::Never,
            TypeFact::Any => Self::Any,
            TypeFact::Primitive(tag) => Self::Primitive {
                name: tag.name().to_owned(),
            },
            TypeFact::Range => Self::Range,
            TypeFact::Array { element } => Self::Array {
                element: Box::new(Self::from_type_fact(element)),
            },
            TypeFact::Map { key, value } => Self::Map {
                key: Box::new(Self::from_type_fact(key)),
                value: Box::new(Self::from_type_fact(value)),
            },
            TypeFact::Set { element } => Self::Set {
                element: Box::new(Self::from_type_fact(element)),
            },
            TypeFact::Iterator { item } => Self::Iterator {
                item: Box::new(Self::from_type_fact(item)),
            },
            TypeFact::Option { some } => Self::Option {
                some: Box::new(Self::from_type_fact(some)),
            },
            TypeFact::OptionSome { some } => Self::OptionSome {
                some: Box::new(Self::from_type_fact(some)),
            },
            TypeFact::OptionNone => Self::OptionNone,
            TypeFact::Result { ok, err } => Self::Result {
                ok: Box::new(Self::from_type_fact(ok)),
                err: Box::new(Self::from_type_fact(err)),
            },
            TypeFact::ResultOk { ok } => Self::ResultOk {
                ok: Box::new(Self::from_type_fact(ok)),
            },
            TypeFact::ResultErr { err } => Self::ResultErr {
                err: Box::new(Self::from_type_fact(err)),
            },
            TypeFact::Function { params, returns } => Self::Function {
                params: params.iter().map(Self::from_type_fact).collect(),
                returns: Box::new(Self::from_type_fact(returns)),
            },
            TypeFact::Record { name } => Self::Record { name: name.clone() },
            TypeFact::Enum { name, variant } => Self::Enum {
                name: name.clone(),
                variant: variant.clone(),
            },
            TypeFact::Host { name } => Self::Host { name: name.clone() },
            TypeFact::Trait { name } => Self::Trait { name: name.clone() },
            TypeFact::Module { name } => Self::Module { name: name.clone() },
            TypeFact::Union(facts) => Self::Union {
                facts: facts.iter().map(Self::from_type_fact).collect(),
            },
        }
    }

    fn to_type_fact(&self) -> TypeFact {
        match self {
            Self::Unknown => TypeFact::Unknown,
            Self::Never => TypeFact::Never,
            Self::Any => TypeFact::Any,
            Self::Primitive { name } => {
                PrimitiveTag::from_name(name).map_or(TypeFact::Unknown, TypeFact::primitive)
            }
            Self::Range => TypeFact::Range,
            Self::Array { element } => TypeFact::array(element.to_type_fact()),
            Self::Map { key, value } => TypeFact::map(key.to_type_fact(), value.to_type_fact()),
            Self::Set { element } => TypeFact::set(element.to_type_fact()),
            Self::Iterator { item } => TypeFact::iterator(item.to_type_fact()),
            Self::Option { some } => TypeFact::option(some.to_type_fact()),
            Self::OptionSome { some } => TypeFact::option_some(some.to_type_fact()),
            Self::OptionNone => TypeFact::option_none(),
            Self::Result { ok, err } => TypeFact::result(ok.to_type_fact(), err.to_type_fact()),
            Self::ResultOk { ok } => TypeFact::result_ok(ok.to_type_fact()),
            Self::ResultErr { err } => TypeFact::result_err(err.to_type_fact()),
            Self::Function { params, returns } => TypeFact::function(
                params.iter().map(Self::to_type_fact).collect(),
                returns.to_type_fact(),
            ),
            Self::Record { name } => TypeFact::record(name),
            Self::Enum { name, variant } => TypeFact::enum_type(name, variant.clone()),
            Self::Host { name } => TypeFact::host(name),
            Self::Trait { name } => TypeFact::trait_type(name),
            Self::Module { name } => TypeFact::module(name),
            Self::Union { facts } => TypeFact::union(facts.iter().map(Self::to_type_fact)),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::LanguageServiceDatabases;

    use super::*;

    fn sample_facts() -> RegistryFacts {
        let mut facts = RegistryFacts::default();
        facts.insert_type("Player", TypeFact::host("Player"));
        facts.insert_trait("Rewardable", TypeFact::trait_type("Rewardable"));
        facts.insert_field("Player", "level", TypeFact::I64);
        facts.insert_field_access(RegistryFieldAccessFact {
            owner: "Player".to_owned(),
            name: "level".to_owned(),
            readable: true,
            writable: true,
            reflect_readable: true,
            reflect_writable: false,
            required_permissions: vec!["player.read".to_owned()],
        });
        facts.insert_method(
            "Player",
            "grant_exp",
            TypeFact::function(vec![TypeFact::I64], TypeFact::BOOL),
        );
        facts.insert_method_effect("Player", "grant_exp", RegistryEffectFact::host_write());
        facts.insert_method_access(RegistryMethodAccessFact {
            owner: "Player".to_owned(),
            name: "grant_exp".to_owned(),
            public: true,
            reflect_callable: true,
            required_permissions: vec!["player.reward".to_owned()],
        });
        facts.insert_function(
            "game::reward::grant",
            TypeFact::function(
                vec![TypeFact::host("Player"), TypeFact::I64],
                TypeFact::BOOL,
            ),
        );
        facts.insert_function_effect("game::reward::grant", RegistryEffectFact::host_write());
        facts.insert_index_capability(RegistryIndexCapabilityFact {
            owner: "Inventory".to_owned(),
            readable: true,
            writable: true,
            addable: false,
            removable: false,
            key: TypeFact::STRING,
            value: TypeFact::I64,
        });
        facts
    }

    #[test]
    fn schema_export_round_trips_registry_facts() {
        let facts = sample_facts();
        let artifact = SchemaArtifact::from_registry_facts(&facts);
        let json = artifact
            .to_json()
            .expect("schema artifact should encode as JSON");
        let parsed =
            SchemaArtifact::from_json(&json).expect("schema artifact should decode from JSON");
        let round_tripped = parsed.to_registry_facts();

        assert_eq!(round_tripped, facts);
    }

    #[test]
    fn invalid_schema_reports_diagnostic() {
        let error = SchemaArtifact::from_json(r#"{ "formatVersion": 999 }"#)
            .expect_err("unsupported format version should fail");

        assert!(
            error
                .message()
                .contains("unsupported schema artifact format version"),
            "{}",
            error.message()
        );
    }

    #[test]
    fn invalid_schema_artifact_records_schema_diagnostic() {
        let mut databases = LanguageServiceDatabases::new();
        databases.load_schema_artifact_json("/workspace/target/vela/schema.json", "{");

        let diagnostics = databases.schema_db().diagnostics();
        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0].message().contains("schema.json` is invalid"),
            "{}",
            diagnostics[0].message()
        );
        assert!(
            databases.schema_db().facts().types().next().is_none(),
            "invalid schema should not leave stale facts installed"
        );
    }
}
