use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use vela_analysis::registry::{RegistryEffectFact, RegistryFacts};
use vela_analysis::type_fact::TypeFact;

use super::SchemaArtifactError;

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaServiceSetFact {
    id: String,
    path: String,
    abi_fingerprint: String,
    type_binding_checksum: String,
    services: Vec<SchemaServiceFact>,
}

impl SchemaServiceSetFact {
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        path: impl Into<String>,
        abi_fingerprint: impl Into<String>,
        type_binding_checksum: impl Into<String>,
        services: impl IntoIterator<Item = SchemaServiceFact>,
    ) -> Self {
        Self {
            id: id.into(),
            path: path.into(),
            abi_fingerprint: abi_fingerprint.into(),
            type_binding_checksum: type_binding_checksum.into(),
            services: services.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub fn abi_fingerprint(&self) -> &str {
        &self.abi_fingerprint
    }

    #[must_use]
    pub fn type_binding_checksum(&self) -> &str {
        &self.type_binding_checksum
    }

    #[must_use]
    pub fn services(&self) -> &[SchemaServiceFact] {
        &self.services
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaServiceFact {
    id: String,
    member: String,
    path: String,
    abi_fingerprint: String,
    methods: Vec<SchemaServiceMethodFact>,
}

impl SchemaServiceFact {
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        member: impl Into<String>,
        path: impl Into<String>,
        abi_fingerprint: impl Into<String>,
        methods: impl IntoIterator<Item = SchemaServiceMethodFact>,
    ) -> Self {
        Self {
            id: id.into(),
            member: member.into(),
            path: path.into(),
            abi_fingerprint: abi_fingerprint.into(),
            methods: methods.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn member(&self) -> &str {
        &self.member
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub fn abi_fingerprint(&self) -> &str {
        &self.abi_fingerprint
    }

    #[must_use]
    pub fn methods(&self) -> &[SchemaServiceMethodFact] {
        &self.methods
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaServiceMethodFact {
    id: String,
    name: String,
    path: String,
    async_method: bool,
    effects: Vec<String>,
    parameters: Vec<SchemaServiceParameterFact>,
    return_type: String,
}

impl SchemaServiceMethodFact {
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        path: impl Into<String>,
        async_method: bool,
        effects: impl IntoIterator<Item = String>,
        parameters: impl IntoIterator<Item = SchemaServiceParameterFact>,
        return_type: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            path: path.into(),
            async_method,
            effects: effects.into_iter().collect(),
            parameters: parameters.into_iter().collect(),
            return_type: return_type.into(),
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub const fn is_async(&self) -> bool {
        self.async_method
    }

    #[must_use]
    pub fn effects(&self) -> &[String] {
        &self.effects
    }

    #[must_use]
    pub fn parameters(&self) -> &[SchemaServiceParameterFact] {
        &self.parameters
    }

    #[must_use]
    pub fn return_type(&self) -> &str {
        &self.return_type
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaServiceParameterFact {
    name: String,
    type_hint: String,
    mode: String,
    #[serde(default)]
    host_origins: Vec<String>,
}

impl SchemaServiceParameterFact {
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        type_hint: impl Into<String>,
        mode: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            type_hint: type_hint.into(),
            mode: mode.into(),
            host_origins: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_host_origins(
        mut self,
        origins: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.host_origins = origins.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn type_hint(&self) -> &str {
        &self.type_hint
    }

    #[must_use]
    pub fn mode(&self) -> &str {
        &self.mode
    }

    #[must_use]
    pub fn host_origins(&self) -> &[String] {
        &self.host_origins
    }
}

pub(super) fn validate_service_set(
    service_set: &SchemaServiceSetFact,
) -> Result<(), SchemaArtifactError> {
    require_nonempty("service-set ID", &service_set.id)?;
    require_nonempty("service-set path", &service_set.path)?;
    require_nonempty("service-set ABI fingerprint", &service_set.abi_fingerprint)?;
    require_nonempty(
        "service-set TypeBinding checksum",
        &service_set.type_binding_checksum,
    )?;
    let mut service_ids = BTreeSet::new();
    let mut service_paths = BTreeSet::new();
    let mut service_members = BTreeSet::new();
    for service in &service_set.services {
        require_nonempty("service ID", &service.id)?;
        require_nonempty("service member", &service.member)?;
        require_nonempty("service path", &service.path)?;
        require_nonempty("service ABI fingerprint", &service.abi_fingerprint)?;
        if !service_ids.insert(&service.id)
            || !service_paths.insert(&service.path)
            || !service_members.insert(&service.member)
        {
            return Err(SchemaArtifactError::new(
                "service metadata contains a duplicate ID, path, or set member",
            ));
        }
        let mut method_ids = BTreeSet::new();
        let mut method_names = BTreeSet::new();
        for method in &service.methods {
            require_nonempty("service method ID", &method.id)?;
            require_nonempty("service method name", &method.name)?;
            require_nonempty("service method path", &method.path)?;
            require_nonempty("service method return type", &method.return_type)?;
            if !method_ids.insert(&method.id) || !method_names.insert(&method.name) {
                return Err(SchemaArtifactError::new(
                    "service metadata contains a duplicate method ID or name",
                ));
            }
            for parameter in &method.parameters {
                require_nonempty("service parameter name", &parameter.name)?;
                require_nonempty("service parameter type", &parameter.type_hint)?;
                require_nonempty("service parameter mode", &parameter.mode)?;
                let mut origins = BTreeSet::new();
                for origin in &parameter.host_origins {
                    if !matches!(
                        origin.as_str(),
                        "injected" | "constructible" | "produced_borrow"
                    ) || !origins.insert(origin)
                    {
                        return Err(SchemaArtifactError::new(
                            "service parameter contains an unknown or duplicate Host origin",
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

pub(super) fn project_service_set(service_set: &SchemaServiceSetFact, facts: &mut RegistryFacts) {
    for service in &service_set.services {
        facts.insert_trait(&service.path, TypeFact::trait_type(&service.path));
        for method in &service.methods {
            facts.insert_trait_method(
                &service.path,
                &method.name,
                TypeFact::function(
                    method
                        .parameters
                        .iter()
                        .map(|_| TypeFact::Unknown)
                        .collect(),
                    TypeFact::Unknown,
                ),
            );
            facts.insert_trait_method_effect(
                &service.path,
                &method.name,
                registry_effect(&method.effects),
            );
        }
    }
}

fn registry_effect(effects: &[String]) -> RegistryEffectFact {
    let has = |name| effects.iter().any(|effect| effect == name);
    RegistryEffectFact {
        reads_host: has("host_read") || has("host_write"),
        writes_host: has("host_write"),
        emits_events: has("event_emit"),
        reads_time: has("time"),
        uses_random: has("random"),
        reads_io: has("io_read"),
        writes_io: has("io_write"),
        reads_reflection: has("reflection_read"),
        writes_reflection: has("reflection_write"),
        calls_reflection: has("reflection_call"),
        spawns_tasks: has("task_spawn"),
    }
}

fn require_nonempty(kind: &str, value: &str) -> Result<(), SchemaArtifactError> {
    if value.trim().is_empty() {
        return Err(SchemaArtifactError::new(format!(
            "{kind} must be non-empty"
        )));
    }
    Ok(())
}
