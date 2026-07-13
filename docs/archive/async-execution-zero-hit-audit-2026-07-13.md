# Async Execution Zero-Hit Audit — 2026-07-13

This audit records the Batch D architecture-leftover scan from
`docs/async-execution-model-plan.md` Section 17.6. It covers active source and
active architecture/decision/goal documents; historical archive text is not an
implementation contract.

## Zero-Hit Patterns

The following searches returned no matches:

```text
does not support async (functions|methods)
DIRECT_HOST_OBJECT_ID_BASE | CallArgsAdapter | GlobalStoreAdapter
may_yield
BoxFuture | LocalBoxFuture | SendRuntime | LocalRuntime
Portable | ThreadBound | thread_bound
tokio::spawn in vela_engine, vela_vm, or vela_host
public call_with_adapter/call_method/call_provider/call_provider_handle/
  call_provider_with_adapter/call_raw/call_args_raw Runtime methods
```

The macro rejection scan therefore found no legacy blanket rejection for async
functions or methods. The runtime surface scan found only `call` and
`call_async` as public execution methods.

## Reviewed Hits

`execute_linked_call(` has 24 matches across `vela_vm` and `vela_engine`. There
is exactly one match in `linked_execution.rs`: the definition of the
non-recursive root-driver shim. All other matches are outer VM entry adapters or
tests. The session body never calls the shim, so script calls, callbacks,
providers, guards, and methods cannot recurse through it.

`ActiveNativeReentry::drive_sync` and `drive_async` are the expected sync and
async front ends. Both push onto the same `ExecutionSession` and call
`drive_linked_execution`; they are not separate interpreters or runtime setup
paths. The only `call_provider_i64` match is a package test helper.

`Runtime::call_raw` and `call_args_raw` remain `#[cfg(test)] pub(crate)` fixture
adapters for legacy host/cache/reload coverage. They are absent from production
builds and from the public Runtime API. Their calls do not represent a second
supported embedding surface.

Direct host argument IDs are assigned by the execution-owned
`ExecutionHost::next_direct_object_id` and passed through nested reentry scopes.
No `CallArgs`-local base or allocator remains. Runtime global host IDs use their
separate high range and are not script-call argument IDs.

## Result

No active architectural leftover requires implementation changes. The reviewed
compatibility names are narrow root/test adapters over the single frame driver,
not alternate sync, async, provider, host-ID, or executor designs.
