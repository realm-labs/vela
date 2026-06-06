# Host Type Methods Example

Run:

```bash
cargo run -p vela_engine --example host_type_methods --quiet
```

This example covers the host type method and argument model:

- concrete host type specs for `Player`, `IntIntMap`, `TagSet`, and `RewardSink`
- same method name on different concrete receiver types: `contains`
- direct `&mut` Rust object injection through `CallArgs::with_host_mut`
- `player.inventory.items["gold"].count` as a keyed host path without cloning a Rust collection
- root and child host method calls through `receiver_path + HostMethodId`
