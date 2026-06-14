#[path = "workloads/core.rs"]
mod core;
#[path = "workloads/extended.rs"]
mod extended;
#[path = "workloads/value_keyed.rs"]
mod value_keyed;

pub(crate) struct Workload {
    pub(crate) name: &'static str,
    pub(crate) vela: &'static str,
    pub(crate) lua: &'static str,
    pub(crate) rhai: &'static str,
    pub(crate) node: &'static str,
    pub(crate) python: &'static str,
}

pub(crate) fn all_workloads() -> impl Iterator<Item = &'static Workload> {
    core::CORE_WORKLOADS
        .iter()
        .chain(value_keyed::VALUE_KEYED_WORKLOADS.iter())
        .chain(extended::EXTENDED_WORKLOADS.iter())
}
