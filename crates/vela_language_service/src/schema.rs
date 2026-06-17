use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use vela_analysis::registry::{
    RegistryEffectFact, RegistryFacts, RegistryFieldAccessFact, RegistryFunctionFact,
    RegistryIndexCapabilityFact, RegistryMemberFact, RegistryMethodAccessFact,
};
use vela_analysis::type_fact::TypeFact;
use vela_common::{PrimitiveTag, SourceId, Span};

pub const SCHEMA_ARTIFACT_FORMAT_VERSION: u32 = 1;
const SCHEMA_HASH_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const SCHEMA_HASH_PRIME: u64 = 0x0000_0100_0000_01b3;

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

    #[must_use]
    pub fn schema_version(&self) -> Option<&str> {
        self.schema_version.as_deref()
    }

    #[must_use]
    pub fn schema_hash(&self) -> Option<&str> {
        self.schema_hash.as_deref()
    }

    pub fn computed_schema_hash(&self) -> Result<u64, SchemaArtifactError> {
        self.facts.compatibility_hash()
    }

    #[must_use]
    pub fn source_locations(&self) -> SchemaSourceLocations {
        self.facts.source_locations()
    }

    fn validate(&self) -> Result<(), SchemaArtifactError> {
        if self.format_version != SCHEMA_ARTIFACT_FORMAT_VERSION {
            return Err(SchemaArtifactError::new(format!(
                "unsupported schema artifact format version {}; expected {}",
                self.format_version, SCHEMA_ARTIFACT_FORMAT_VERSION
            )));
        }
        if self
            .schema_version
            .as_deref()
            .is_some_and(|version| version.trim().is_empty())
        {
            return Err(SchemaArtifactError::new(
                "schemaVersion must be non-empty when present",
            ));
        }
        if let Some(schema_hash) = self.schema_hash.as_deref() {
            let declared = parse_schema_hash(schema_hash)?;
            let computed = self.computed_schema_hash()?;
            if declared != computed {
                return Err(SchemaArtifactError::new(format!(
                    "schema hash mismatch: artifact declares {}, computed 0x{computed:016x}",
                    schema_hash.trim()
                )));
            }
        }
        Ok(())
    }
}

fn parse_schema_hash(value: &str) -> Result<u64, SchemaArtifactError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(SchemaArtifactError::new(
            "schemaHash must be non-empty when present",
        ));
    }
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        return u64::from_str_radix(hex, 16).map_err(|_| {
            SchemaArtifactError::new(format!(
                "schemaHash `{trimmed}` must be a decimal u64 or 0x-prefixed hexadecimal u64"
            ))
        });
    }
    trimmed.parse::<u64>().map_err(|_| {
        SchemaArtifactError::new(format!(
            "schemaHash `{trimmed}` must be a decimal u64 or 0x-prefixed hexadecimal u64"
        ))
    })
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct SchemaSourceLocations {
    types: BTreeMap<String, Span>,
    traits: BTreeMap<String, Span>,
    fields: BTreeMap<(String, String), Span>,
    variants: BTreeMap<(String, String), Span>,
    methods: BTreeMap<(String, String), Span>,
    trait_methods: BTreeMap<(String, String), Span>,
    functions: BTreeMap<String, Span>,
}

impl SchemaSourceLocations {
    #[must_use]
    pub fn type_span(&self, name: &str) -> Option<Span> {
        self.types.get(name).copied()
    }

    #[must_use]
    pub fn trait_span(&self, name: &str) -> Option<Span> {
        self.traits.get(name).copied()
    }

    #[must_use]
    pub fn field_span(&self, owner: &str, name: &str) -> Option<Span> {
        self.fields
            .get(&(owner.to_owned(), name.to_owned()))
            .copied()
    }

    #[must_use]
    pub fn variant_span(&self, owner: &str, name: &str) -> Option<Span> {
        self.variants
            .get(&(owner.to_owned(), name.to_owned()))
            .copied()
    }

    #[must_use]
    pub fn method_span(&self, owner: &str, name: &str) -> Option<Span> {
        self.methods
            .get(&(owner.to_owned(), name.to_owned()))
            .copied()
    }

    #[must_use]
    pub fn trait_method_span(&self, owner: &str, name: &str) -> Option<Span> {
        self.trait_methods
            .get(&(owner.to_owned(), name.to_owned()))
            .copied()
    }

    #[must_use]
    pub fn function_span(&self, name: &str) -> Option<Span> {
        self.functions
            .get(name)
            .copied()
            .or_else(|| self.unique_function_segment_span(name))
    }

    fn unique_function_segment_span(&self, name: &str) -> Option<Span> {
        let mut matches = self.functions.iter().filter_map(|(function, span)| {
            function
                .rsplit("::")
                .next()
                .is_some_and(|segment| segment == name)
                .then_some(*span)
        });
        let first = matches.next()?;
        matches.next().is_none().then_some(first)
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
                .map(|(name, fact)| SchemaNamedFact::new(name, fact, facts.type_docs(name)))
                .collect(),
            traits: facts
                .traits()
                .map(|(name, fact)| SchemaNamedFact::new(name, fact, facts.trait_docs(name)))
                .collect(),
            fields: facts
                .fields()
                .map(|member| {
                    let docs = facts.field_docs(&member.owner, &member.name);
                    SchemaMemberFact::from_registry_member(member, docs)
                })
                .collect(),
            field_access: facts
                .field_accesses()
                .map(SchemaFieldAccessFact::from)
                .collect(),
            variants: facts
                .variants()
                .map(|member| {
                    let docs = facts.variant_docs(&member.owner, &member.name);
                    SchemaMemberFact::from_registry_member(member, docs)
                })
                .collect(),
            methods: facts
                .methods()
                .map(|member| {
                    let docs = facts.method_docs(&member.owner, &member.name);
                    SchemaMemberFact::from_registry_member(member, docs)
                })
                .collect(),
            method_effects: facts
                .method_effects()
                .map(SchemaMemberEffectFact::from)
                .collect(),
            method_access: facts
                .method_accesses()
                .map(SchemaMethodAccessFact::from)
                .collect(),
            trait_methods: facts
                .trait_methods()
                .map(|member| {
                    let docs = facts.trait_method_docs(&member.owner, &member.name);
                    SchemaMemberFact::from_registry_member(member, docs)
                })
                .collect(),
            trait_method_effects: facts
                .trait_method_effects()
                .map(SchemaMemberEffectFact::from)
                .collect(),
            functions: facts
                .functions()
                .map(|function| {
                    let docs = facts.function_docs(&function.name);
                    SchemaFunctionFact::from_registry_function(function, docs)
                })
                .collect(),
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
            if let Some(docs) = &entry.docs {
                facts.insert_type_docs(entry.name.clone(), docs.clone());
            }
        }
        for entry in &self.traits {
            facts.insert_trait(entry.name.clone(), entry.fact.to_type_fact());
            if let Some(docs) = &entry.docs {
                facts.insert_trait_docs(entry.name.clone(), docs.clone());
            }
        }
        for entry in &self.fields {
            facts.insert_field(
                entry.owner.clone(),
                entry.name.clone(),
                entry.fact.to_type_fact(),
            );
            if let Some(docs) = &entry.docs {
                facts.insert_field_docs(entry.owner.clone(), entry.name.clone(), docs.clone());
            }
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
            if let Some(docs) = &entry.docs {
                facts.insert_variant_docs(entry.owner.clone(), entry.name.clone(), docs.clone());
            }
        }
        for entry in &self.methods {
            facts.insert_method(
                entry.owner.clone(),
                entry.name.clone(),
                entry.fact.to_type_fact(),
            );
            if let Some(docs) = &entry.docs {
                facts.insert_method_docs(entry.owner.clone(), entry.name.clone(), docs.clone());
            }
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
            if let Some(docs) = &entry.docs {
                facts.insert_trait_method_docs(
                    entry.owner.clone(),
                    entry.name.clone(),
                    docs.clone(),
                );
            }
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
            if let Some(docs) = &entry.docs {
                facts.insert_function_docs(entry.name.clone(), docs.clone());
            }
        }
        for effect in &self.function_effects {
            facts.insert_function_effect(effect.name.clone(), effect.effect.to_registry_fact());
        }
        for capability in &self.index_capabilities {
            facts.insert_index_capability(capability.to_registry_fact());
        }
        facts
    }

    fn compatibility_hash(&self) -> Result<u64, SchemaArtifactError> {
        let canonical_facts = Self::from_registry_facts(&self.to_registry_facts());
        let payload = serde_json::to_vec(&canonical_facts).map_err(|error| {
            SchemaArtifactError::new(format!("failed to encode canonical schema facts: {error}"))
        })?;
        Ok(fnv1a64(&payload))
    }

    fn source_locations(&self) -> SchemaSourceLocations {
        let mut locations = SchemaSourceLocations::default();
        for entry in &self.types {
            if let Some(span) = entry.source_span.and_then(SchemaSourceSpan::to_span) {
                locations.types.insert(entry.name.clone(), span);
            }
        }
        for entry in &self.traits {
            if let Some(span) = entry.source_span.and_then(SchemaSourceSpan::to_span) {
                locations.traits.insert(entry.name.clone(), span);
            }
        }
        for entry in &self.fields {
            if let Some(span) = entry.source_span.and_then(SchemaSourceSpan::to_span) {
                locations
                    .fields
                    .insert((entry.owner.clone(), entry.name.clone()), span);
            }
        }
        for entry in &self.variants {
            if let Some(span) = entry.source_span.and_then(SchemaSourceSpan::to_span) {
                locations
                    .variants
                    .insert((entry.owner.clone(), entry.name.clone()), span);
            }
        }
        for entry in &self.methods {
            if let Some(span) = entry.source_span.and_then(SchemaSourceSpan::to_span) {
                locations
                    .methods
                    .insert((entry.owner.clone(), entry.name.clone()), span);
            }
        }
        for entry in &self.trait_methods {
            if let Some(span) = entry.source_span.and_then(SchemaSourceSpan::to_span) {
                locations
                    .trait_methods
                    .insert((entry.owner.clone(), entry.name.clone()), span);
            }
        }
        for entry in &self.functions {
            if let Some(span) = entry.source_span.and_then(SchemaSourceSpan::to_span) {
                locations.functions.insert(entry.name.clone(), span);
            }
        }
        locations
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = SCHEMA_HASH_OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(SCHEMA_HASH_PRIME);
    }
    if hash == 0 { 1 } else { hash }
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SchemaSourceSpan {
    source: u32,
    start: u32,
    end: u32,
}

impl SchemaSourceSpan {
    fn to_span(self) -> Option<Span> {
        (self.start <= self.end)
            .then(|| Span::new(SourceId::new(self.source), self.start, self.end))
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
struct SchemaNamedFact {
    name: String,
    fact: SchemaTypeFact,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    docs: Option<String>,
    #[serde(
        default,
        rename = "sourceSpan",
        alias = "source_span",
        skip_serializing_if = "Option::is_none"
    )]
    source_span: Option<SchemaSourceSpan>,
}

impl SchemaNamedFact {
    fn new(name: impl Into<String>, fact: &TypeFact, docs: Option<&str>) -> Self {
        Self {
            name: name.into(),
            fact: SchemaTypeFact::from_type_fact(fact),
            docs: docs.map(str::to_owned),
            source_span: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
struct SchemaMemberFact {
    owner: String,
    name: String,
    fact: SchemaTypeFact,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    docs: Option<String>,
    #[serde(
        default,
        rename = "sourceSpan",
        alias = "source_span",
        skip_serializing_if = "Option::is_none"
    )]
    source_span: Option<SchemaSourceSpan>,
}

impl SchemaMemberFact {
    fn from_registry_member(value: RegistryMemberFact, docs: Option<&str>) -> Self {
        Self {
            owner: value.owner,
            name: value.name,
            fact: SchemaTypeFact::from_type_fact(&value.fact),
            docs: docs.map(str::to_owned),
            source_span: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
struct SchemaFunctionFact {
    name: String,
    fact: SchemaTypeFact,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    docs: Option<String>,
    #[serde(
        default,
        rename = "sourceSpan",
        alias = "source_span",
        skip_serializing_if = "Option::is_none"
    )]
    source_span: Option<SchemaSourceSpan>,
}

impl SchemaFunctionFact {
    fn from_registry_function(value: RegistryFunctionFact, docs: Option<&str>) -> Self {
        Self {
            name: value.name,
            fact: SchemaTypeFact::from_type_fact(&value.fact),
            docs: docs.map(str::to_owned),
            source_span: None,
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
        facts.insert_type_docs("Player", "Player host object.");
        facts.insert_trait("Rewardable", TypeFact::trait_type("Rewardable"));
        facts.insert_trait_docs("Rewardable", "Rewardable host trait.");
        facts.insert_field("Player", "level", TypeFact::I64);
        facts.insert_field_docs("Player", "level", "Current player level.");
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
        facts.insert_method_docs("Player", "grant_exp", "Grant player experience.");
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
        facts.insert_function_docs("game::reward::grant", "Grant reward.");
        facts.insert_function_effect("game::reward::grant", RegistryEffectFact::host_write());
        facts.insert_variant(
            "QuestState",
            "Active",
            TypeFact::enum_type("QuestState", Some("Active")),
        );
        facts.insert_variant_docs("QuestState", "Active", "Active quest state.");
        facts.insert_trait_method(
            "Rewardable",
            "preview",
            TypeFact::function(vec![TypeFact::I64], TypeFact::BOOL),
        );
        facts.insert_trait_method_docs("Rewardable", "preview", "Preview reward.");
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
    fn schema_artifact_accepts_docs_metadata() {
        let artifact = SchemaArtifact::from_json(
            r#"{
                "formatVersion": 1,
                "facts": {
                    "types": [
                        {
                            "name": "Player",
                            "fact": { "kind": "host", "name": "Player" },
                            "docs": "Player host object."
                        }
                    ],
                    "fields": [
                        {
                            "owner": "Player",
                            "name": "level",
                            "fact": { "kind": "primitive", "name": "i64" },
                            "docs": "Current player level."
                        }
                    ],
                    "functions": [
                        {
                            "name": "game::reward::grant",
                            "fact": {
                                "kind": "function",
                                "params": [{ "kind": "host", "name": "Player" }],
                                "returns": { "kind": "primitive", "name": "bool" }
                            },
                            "docs": "Grant reward."
                        }
                    ]
                }
            }"#,
        )
        .expect("schema docs metadata should decode");

        let facts = artifact.to_registry_facts();

        assert_eq!(facts.type_docs("Player"), Some("Player host object."));
        assert_eq!(
            facts.field_docs("Player", "level"),
            Some("Current player level.")
        );
        assert_eq!(
            facts.function_docs("game::reward::grant"),
            Some("Grant reward.")
        );
    }

    #[test]
    fn schema_hash_compatibility_accepts_matching_facts() {
        let facts = sample_facts();
        let mut artifact = SchemaArtifact::from_registry_facts(&facts);
        let computed = artifact
            .computed_schema_hash()
            .expect("schema hash should be computable");
        let expected_hash = format!("0x{computed:016x}");
        artifact.schema_version = Some("2026-06-16T00:00:00Z".to_owned());
        artifact.schema_hash = Some(expected_hash.clone());
        let json = artifact
            .to_json()
            .expect("schema artifact should encode as JSON");

        let parsed =
            SchemaArtifact::from_json(&json).expect("matching schema hash should validate");

        assert_eq!(parsed.schema_version(), Some("2026-06-16T00:00:00Z"));
        assert_eq!(parsed.schema_hash(), Some(expected_hash.as_str()));
        assert_eq!(parsed.to_registry_facts(), facts);
    }

    #[test]
    fn schema_hash_compatibility_rejects_stale_facts() {
        let facts = sample_facts();
        let mut artifact = SchemaArtifact::from_registry_facts(&facts);
        artifact.schema_hash = Some("0x0000000000000001".to_owned());
        let json = artifact
            .to_json()
            .expect("schema artifact should encode as JSON");

        let error = SchemaArtifact::from_json(&json)
            .expect_err("stale schema hash should fail compatibility validation");

        assert!(
            error.message().contains("schema hash mismatch"),
            "{}",
            error.message()
        );
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
    fn invalid_schema_metadata_reports_diagnostic() {
        let mut databases = LanguageServiceDatabases::new();
        databases.load_schema_artifact_json(
            "/workspace/target/vela/schema.json",
            r#"{ "formatVersion": 1, "schemaVersion": " ", "facts": {} }"#,
        );

        let diagnostics = databases.schema_db().diagnostics();
        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .message()
                .contains("schemaVersion must be non-empty"),
            "{}",
            diagnostics[0].message()
        );
        assert!(
            databases.schema_db().facts().types().next().is_none(),
            "invalid schema metadata should not install facts"
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
