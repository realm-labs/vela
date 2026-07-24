use vela_common::{CallableAsyncness, ServiceId, ServiceMethodId, ServiceSetId};
use vela_mir::MirEffect;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceCompilationSchema {
    service_set: ServiceSetId,
    services: Box<[ServiceCompilationService]>,
}

impl ServiceCompilationSchema {
    #[must_use]
    pub fn new(
        service_set: ServiceSetId,
        services: impl IntoIterator<Item = ServiceCompilationService>,
    ) -> Self {
        Self {
            service_set,
            services: services.into_iter().collect(),
        }
    }

    #[must_use]
    pub const fn service_set(&self) -> ServiceSetId {
        self.service_set
    }

    pub fn services(&self) -> impl ExactSizeIterator<Item = &ServiceCompilationService> {
        self.services.iter()
    }

    #[must_use]
    pub fn service_by_path(&self, path: &str) -> Option<&ServiceCompilationService> {
        self.services.iter().find(|service| service.path == path)
    }

    #[must_use]
    pub fn service_by_member(&self, member: &str) -> Option<&ServiceCompilationService> {
        self.services
            .iter()
            .find(|service| service.member == member)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceCompilationService {
    pub id: ServiceId,
    pub member: String,
    pub path: String,
    methods: Box<[ServiceCompilationMethod]>,
}

impl ServiceCompilationService {
    #[must_use]
    pub fn new(
        id: ServiceId,
        member: impl Into<String>,
        path: impl Into<String>,
        methods: impl IntoIterator<Item = ServiceCompilationMethod>,
    ) -> Self {
        Self {
            id,
            member: member.into(),
            path: path.into(),
            methods: methods.into_iter().collect(),
        }
    }

    pub fn methods(&self) -> impl ExactSizeIterator<Item = &ServiceCompilationMethod> {
        self.methods.iter()
    }

    #[must_use]
    pub fn method(&self, name: &str) -> Option<&ServiceCompilationMethod> {
        self.methods.iter().find(|method| method.name == name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceCompilationMethod {
    pub id: ServiceMethodId,
    pub name: String,
    pub parameter_count: u32,
    pub asyncness: CallableAsyncness,
    pub effect: MirEffect,
}

impl ServiceCompilationMethod {
    #[must_use]
    pub fn new(
        id: ServiceMethodId,
        name: impl Into<String>,
        parameter_count: u32,
        asyncness: CallableAsyncness,
        effect: MirEffect,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            parameter_count,
            asyncness,
            effect,
        }
    }
}
