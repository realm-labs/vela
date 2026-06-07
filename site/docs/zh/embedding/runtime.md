# Runtime API

Rust embedding 的核心流程很小：构建 engine，编译源码，创建 runtime，调用脚本 entry。

```rust
let engine = EngineBuilder::new()
    .with_standard_natives()
    .capability(Capability::Random)
    .with_controlled_random(7)
    .build()?;

let program = engine.compile_source(SourceId::new(1), source)?;
let mut runtime = Runtime::new(engine, program);
let result = runtime.call(
    "main",
    CallArgs::new(),
    CallOptions::new(250_000, 8 * 1024 * 1024, 128),
)?;
```

## 调用句柄

高频 embedding 可以缓存 `VelaFunction`：

```rust
let handle_tick = runtime.entry("handle_tick")?;
runtime.call(&handle_tick, args, options)?;
```

句柄绑定到 runtime，并参与热更新版本检查。

## 返回值

`Runtime::call` 返回 `VelaValue`，它可以原样传回后续脚本调用，不需要先转成 detached owned snapshot。Rust 需要时可以转成 `OwnedValue`，或者通过 serde 反序列化。
