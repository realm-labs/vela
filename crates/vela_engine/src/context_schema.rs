use vela_common::{FieldId, HostMethodId, HostTypeId, TypeId};
use vela_reflect::access::{MethodAccess, MethodEffectSet};
use vela_reflect::registry::{
    FieldDesc, MethodDesc, MethodParamDesc, SchemaHash, TypeDesc, TypeKey,
};

pub const CONTEXT_TYPE_ID: TypeId = TypeId::new(0xff00_1000);
pub const CONTEXT_HOST_TYPE_ID: HostTypeId = HostTypeId::new(0xff00_1001);
pub const CONTEXT_NOW_FIELD_ID: FieldId = FieldId::new(0xff00_1002);
pub const CONTEXT_TICK_FIELD_ID: FieldId = FieldId::new(0xff00_1003);
pub const CONTEXT_EMIT_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_1004);
pub const CONTEXT_LOG_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_1005);

#[must_use]
pub fn context_host_type_desc() -> TypeDesc {
    TypeDesc::new(TypeKey::new(CONTEXT_TYPE_ID, "Context"))
        .schema_hash(SchemaHash::new(0xff00_1000_0000_0001))
        .host_type(CONTEXT_HOST_TYPE_ID)
        .docs("Standard host context object for deterministic time, events, and logging.")
        .attr("stdlib", "context")
        .attr("domain", "gameplay")
        .field(
            FieldDesc::new(CONTEXT_NOW_FIELD_ID, "now")
                .type_hint("int")
                .docs("Current deterministic context timestamp.")
                .attr("stdlib", "context")
                .attr("domain", "gameplay"),
        )
        .field(
            FieldDesc::new(CONTEXT_TICK_FIELD_ID, "tick")
                .type_hint("int")
                .docs("Current deterministic context tick.")
                .attr("stdlib", "context")
                .attr("domain", "gameplay"),
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
                .attr("domain", "gameplay"),
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
                .attr("domain", "gameplay"),
        )
}
