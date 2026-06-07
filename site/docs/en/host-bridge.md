# Host Bridge

The host bridge is the core boundary between Rust and Vela. Rust owns durable state. Scripts receive handles and paths that describe what they can read or mutate.

## Write-Through Mutation

Host writes are immediate. A script expression such as:

```vela
player.level += 1;
```

lowers to a host read, local computation, and host write-through mutation. If a later script operation fails, already-written Rust state is not automatically rolled back.

## Access Model

- `HostRef` identifies an external host object.
- `HostPath` identifies a field or indexed child under a host root.
- `PathProxy` carries host path intent through script expressions.
- `HostAccess` validates capabilities and routes reads, writes, compound writes, and host method calls.

Scripts never receive real Rust references. A Rust `&mut T` passed at the call boundary becomes a call-scoped writable host handle, not a storable Rust borrow.

## Type Methods

Registered host types have fields, methods, and optional index capabilities. `HashMap<i32, i32>`, `Vec<Item>`, `HashSet<String>`, and trait-object surfaces all use the same concrete host type model from the script point of view.

If a method or index capability is not registered for the receiver type, the compiler or runtime reports a targeted error instead of guessing.
