use std::collections::BTreeMap;

use vela_common::{ServiceCallMode, service_dispatch_stable_id};
use vela_def::FunctionId;
use vela_host::lease::HostLeaseKind;

use super::{ServiceDispatchLease, ServiceDispatchNative};
use crate::interop::BoundaryMode;

pub(super) fn service_dispatch_natives(
    schema: &crate::service::ServiceSetSchema,
) -> BTreeMap<FunctionId, ServiceDispatchNative> {
    schema
        .services()
        .iter()
        .flat_map(|service| {
            service.methods().iter().flat_map(move |method| {
                [ServiceCallMode::Base, ServiceCallMode::Pinned].map(move |mode| {
                    let target =
                        crate::service::ServiceCallTarget::new(mode, service.id(), method.id);
                    let id =
                        FunctionId::new(service_dispatch_stable_id(mode, service.id(), method.id));
                    let method_name = method
                        .path
                        .rsplit("::")
                        .next()
                        .unwrap_or(method.path.as_str());
                    let name = format!(
                        "__vela_service.{}.{}.{}",
                        mode.abi_name(),
                        service.path().replace("::", "."),
                        method_name,
                    );
                    (
                        id,
                        ServiceDispatchNative {
                            target,
                            name,
                            effects: method.callable.effects,
                            asyncness: method.callable.asyncness,
                            parameter_leases: method
                                .callable
                                .parameters
                                .iter()
                                .enumerate()
                                .filter_map(|(index, parameter)| {
                                    let (kind, host_only) = match parameter.mode {
                                        BoundaryMode::StorageDirectedShared => {
                                            (HostLeaseKind::Shared, true)
                                        }
                                        BoundaryMode::SharedHost => (HostLeaseKind::Shared, false),
                                        BoundaryMode::ExclusiveHost => {
                                            (HostLeaseKind::Exclusive, false)
                                        }
                                        _ => return None,
                                    };
                                    Some(ServiceDispatchLease {
                                        index,
                                        kind,
                                        host_only,
                                    })
                                })
                                .collect(),
                        },
                    )
                })
            })
        })
        .collect()
}
