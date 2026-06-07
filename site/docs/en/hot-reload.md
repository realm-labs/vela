# Hot Reload

Vela treats hot reload as a runtime contract, not a development add-on.

## Function-Level Replacement

New source compiles into a new program version. New calls enter the new code. Existing call frames continue on the old `CodeObject` until they naturally return.

## Compatibility

Reload compatibility is bounded by ABI and schema checks. Compatible additions and function body changes can be accepted. Incompatible function signatures, schema changes, effect changes, or access changes can be rejected without advancing the active runtime version.

## Safe Points

Hosts decide when to check or apply pending reloads. Game servers usually do this at event or tick boundaries, so state changes remain predictable.

## Playground Scope

The browser playground focuses on compile and run feedback. Full hot-reload staging is demonstrated in the standalone Rust examples because it depends on host runtime policy and version management.

## Typical Host Loop

```rust
runtime.stage_hot_reload_update_file("scripts/logic.vela")?;

if let Some(report) = runtime.check_reload_at_tick_boundary()? {
    println!("{report:?}");
}
```

Rejected updates keep the current version active. Accepted updates rebuild runtime metadata so future calls enter the new code.
