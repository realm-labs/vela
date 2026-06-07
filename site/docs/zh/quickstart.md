# 快速开始

最快的体验方式是打开浏览器 playground。选择一个示例，编辑脚本，然后运行 `main` 函数。

## Playground 循环

1. 打开 playground。
2. 选择一个示例。
3. 修改源码。
4. 点击 Compile 查看诊断。
5. 点击 Run 运行选中的 entry 函数。

## 最小脚本

```vela
fn main() {
    let rewards = { "gold": 10, "xp": 25 };
    return rewards["gold"] + rewards["xp"];
}
```

## Record 和 Method

```vela
struct DamageResult {
    actor: string,
    applied: int,
}

impl DamageResult {
    fn score(self, bonus) -> int {
        return self.applied + bonus;
    }
}

fn main() {
    let result = DamageResult {
        actor: "knight",
        applied: 42,
    };
    return result.score(8);
}
```

## CLI 形态

CLI 是最终的脚本执行入口，类似 Lua 用户直接执行 `.lua` 文件。

```bash
cargo run -p vela_cli -- examples/src/bin/level_up/level_up.vela
```

## 嵌入形态

Rust host 编译源码得到 program，创建 runtime，然后用明确的参数和执行预算调用脚本入口。需要让脚本修改 Rust 持久状态时，使用 host handle 或注册 global。

```rust
let engine = EngineBuilder::new()
    .with_standard_natives()
    .build()?;

let program = engine.compile_source(SourceId::new(1), source)?;
let mut runtime = Runtime::new(engine, program);
let value = runtime.call("main", CallArgs::new(), CallOptions::unbounded())?;
```
