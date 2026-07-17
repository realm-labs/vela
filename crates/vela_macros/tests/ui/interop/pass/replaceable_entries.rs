use vela_engine::binding::VmResult;
use vela_engine::dispatch::{DispatchAuthority, DispatchRoot};
use vela_macros::{ScriptHost, ScriptReflect, methods, replaceable};

#[derive(ScriptHost, ScriptReflect)]
#[script(path = "host::Context")]
pub struct Context {
    #[script(skip)]
    root: DispatchRoot,
}

impl DispatchAuthority for Context {
    fn vela_dispatch_root(&self) -> &DispatchRoot {
        &self.root
    }
}

#[methods(path = "host::Context")]
impl Context {
    pub fn available(&self) -> bool {
        true
    }
}

#[derive(ScriptHost, ScriptReflect)]
#[script(path = "host::Service")]
pub struct Service {
    #[script(get)]
    value: i64,
}

#[replaceable(path = "host::free", authority = "context", index = 0)]
pub fn free_entry(context: &mut Context, value: i64) -> VmResult<i64> {
    let _ = context;
    Ok(value)
}

#[replaceable(path = "host::plain", authority = "context", index = 2)]
pub fn plain_entry(context: &mut Context, value: i64) -> i64 {
    let _ = context;
    value
}

#[methods(path = "host::Service")]
impl Service {
    #[replaceable(
        path = "host::Service::method_entry",
        authority = "context",
        index = 1
    )]
    pub fn method_entry(&self, context: &mut Context, value: i64) -> VmResult<i64> {
        let _ = context;
        Ok(value)
    }

    pub fn adjacent(&self, value: i64) -> i64 {
        value + 1
    }
}

fn main() {
    let _ = vela_replaceable_slot_free_entry();
    let _ = Service::vela_replaceable_slot_method_entry();
    let _ = vela_replaceable_slot_plain_entry();
}
