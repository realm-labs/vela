use vela_common::{HostMethodId, HostTypeId, stable_id};
use vela_def::{FieldId, TypeId};
use vela_reflect::access::{MethodAccess, MethodEffectSet};
use vela_reflect::registry::{
    FieldDesc, MethodDesc, MethodParamDesc, SchemaHash, TypeDesc, TypeKey,
};

pub const CONTEXT_TYPE_ID: TypeId = TypeId::new(stable_id("std_type", "", "Context") as u128);
pub const CONTEXT_HOST_TYPE_ID: HostTypeId =
    HostTypeId::new(stable_id("std_host_type", "", "Context"));
pub const CONTEXT_NOW_FIELD_ID: FieldId =
    FieldId::new(stable_id("std_field", "Context", "now") as u128);
pub const CONTEXT_TICK_FIELD_ID: FieldId =
    FieldId::new(stable_id("std_field", "Context", "tick") as u128);
pub const CONTEXT_EMIT_METHOD_ID: HostMethodId =
    HostMethodId::new(stable_id("std_method", "Context", "emit"));
pub const CONTEXT_LOG_METHOD_ID: HostMethodId =
    HostMethodId::new(stable_id("std_method", "Context", "log"));

#[must_use]
pub fn context_host_type_desc() -> TypeDesc {
    TypeDesc::new(TypeKey::new(CONTEXT_TYPE_ID, "Context"))
        .schema_hash(SchemaHash::new(stable_id("std_schema", "", "Context")))
        .host_type(CONTEXT_HOST_TYPE_ID)
        .docs("Standard host context object for deterministic time, events, and logging.")
        .attr("stdlib", "context")
        .attr("domain", "context")
        .field(
            FieldDesc::new(CONTEXT_NOW_FIELD_ID, "now")
                .type_hint("int")
                .docs("Current deterministic context timestamp.")
                .attr("stdlib", "context")
                .attr("domain", "context"),
        )
        .field(
            FieldDesc::new(CONTEXT_TICK_FIELD_ID, "tick")
                .type_hint("int")
                .docs("Current deterministic context tick.")
                .attr("stdlib", "context")
                .attr("domain", "context"),
        )
        .method(
            MethodDesc::new(CONTEXT_EMIT_METHOD_ID, "emit")
                .param(MethodParamDesc::new("event").type_hint("string"))
                .param(
                    MethodParamDesc::new("payload")
                        .type_hint("any")
                        .defaulted(true),
                )
                .return_type("null")
                .effects(MethodEffectSet::event_emit())
                .access(MethodAccess::new().reflect_callable(true))
                .docs("Records an event emission patch for the host safe point.")
                .attr("stdlib", "context")
                .attr("domain", "context"),
        )
        .method(
            MethodDesc::new(CONTEXT_LOG_METHOD_ID, "log")
                .param(MethodParamDesc::new("level").type_hint("string"))
                .param(MethodParamDesc::new("message").type_hint("string"))
                .param(
                    MethodParamDesc::new("payload")
                        .type_hint("any")
                        .defaulted(true),
                )
                .return_type("null")
                .effects(MethodEffectSet::event_emit())
                .access(MethodAccess::new().reflect_callable(true))
                .docs("Records a log patch for the host safe point.")
                .attr("stdlib", "context")
                .attr("domain", "context"),
        )
}
