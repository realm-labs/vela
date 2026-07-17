use std::sync::Arc;

use parking_lot::Mutex;
use vela_macros::{ScriptHost, ScriptReflect, methods, replaceable};
use vela_vm::error::{VmError, VmResult};

use super::{DispatchAuthority, DispatchController, DispatchRoot, SharedDispatchRuntime};
use crate::args::{FromScriptArg, IntoScriptArg};
use crate::interop::VelaValueBoundary;
use crate::native::TypeHint;
use crate::runtime::{RuntimeImage, SharedRuntime};

#[derive(Debug, Eq, PartialEq)]
pub struct ReturnError(String);

impl From<VmError> for ReturnError {
    fn from(error: VmError) -> Self {
        Self(error.to_string())
    }
}

impl IntoScriptArg for ReturnError {
    fn into_script_arg(self) -> vela_vm::owned_value::OwnedValue {
        self.0.into_script_arg()
    }
}

impl FromScriptArg for ReturnError {
    const TYPE_NAME: &'static str = "return error";

    fn from_script_arg(value: &vela_vm::owned_value::OwnedValue) -> VmResult<Self> {
        String::from_script_arg(value).map(Self)
    }
}

impl VelaValueBoundary for ReturnError {
    fn vela_type_hint() -> TypeHint {
        TypeHint::string()
    }
}

#[derive(ScriptHost, ScriptReflect)]
#[script(path = "host::ReturnContext")]
pub struct ReturnContext {
    #[script(get)]
    marker: i64,
    #[script(skip)]
    root: DispatchRoot,
}

#[methods]
impl ReturnContext {
    pub fn marker(&self) -> i64 {
        self.marker
    }
}

impl DispatchAuthority for ReturnContext {
    fn vela_dispatch_root(&self) -> &DispatchRoot {
        &self.root
    }
}

#[replaceable(
    path = "host::returns::optional_context",
    authority = "context",
    index = 0
)]
pub fn optional_context(context: &ReturnContext, present: bool) -> Option<&ReturnContext> {
    present.then_some(context)
}

#[replaceable(
    path = "host::returns::result_context",
    authority = "context",
    index = 1
)]
pub fn result_context(
    context: &ReturnContext,
    succeed: bool,
) -> Result<&ReturnContext, ReturnError> {
    succeed
        .then_some(context)
        .ok_or_else(|| ReturnError("fallback".to_owned()))
}

#[replaceable(path = "host::returns::context_pair", authority = "context", index = 2)]
pub fn context_pair(context: &ReturnContext) -> (&ReturnContext, &ReturnContext) {
    (context, context)
}

#[test]
fn replaceable_borrowed_containers_reuse_only_the_tracked_origin() {
    let slots = vec![
        vela_replaceable_slot_optional_context(),
        vela_replaceable_slot_result_context(),
        vela_replaceable_slot_context_pair(),
    ];
    let engine = crate::engine::Engine::builder()
        .register_host_type::<ReturnContext>()
        .register_replaceable_slots(slots.clone())
        .capability(vela_common::Capability::HostRead)
        .build()
        .expect("engine");
    let program = engine
        .compile_source(
            r#"
#[override(host::returns::optional_context)]
fn optional_context(context: ReturnContext, present: bool) -> Option<ReturnContext> {
let aliases = [context];
return Option::Some(aliases[0]);
}

#[override(host::returns::result_context)]
fn result_context(context: ReturnContext, succeed: bool) -> Result<ReturnContext, String> {
let aliases = [context];
if succeed { return Result::Ok(aliases[0]); }
return Result::Err("denied");
}

#[override(host::returns::context_pair)]
fn context_pair(context: ReturnContext) {
let aliases = [context, context];
return (aliases[0], aliases[1]);
}
"#,
        )
        .expect("borrowed container override program");
    let runtime = shared_runtime(engine, program);
    let controller = DispatchController::new(slots).expect("controller");
    let candidate = controller.stage_current(&runtime).expect("override stage");
    controller.activate(candidate).expect("activate candidate");
    let context = ReturnContext {
        marker: 7,
        root: DispatchRoot::pin(&controller, Arc::clone(&runtime)).expect("root"),
    };
    assert_eq!(context.marker(), 7);

    let optional = optional_context(&context, true).expect("present origin");
    assert!(std::ptr::eq(optional, &context));

    let returned = result_context(&context, true).expect("successful origin");
    assert!(std::ptr::eq(returned, &context));
    let Err(error) = result_context(&context, false) else {
        panic!("expected business error");
    };
    assert_eq!(error, ReturnError("denied".to_owned()));

    let (first, second) = context_pair(&context);
    assert!(std::ptr::eq(first, &context));
    assert!(std::ptr::eq(second, &context));
}

fn shared_runtime(
    engine: crate::engine::Engine,
    program: vela_bytecode::compiler::CompiledProgram,
) -> SharedDispatchRuntime {
    let image = RuntimeImage::new_compiled(engine, program).into_shared();
    Arc::new(Mutex::new(
        SharedRuntime::from_shared_image(image).expect("shared runtime"),
    ))
}
