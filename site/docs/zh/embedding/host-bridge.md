# Host 边界

Host bridge 是 Rust 和 Vela 之间的核心边界。Rust 持有持久状态，脚本只拿到可读写范围明确的 handle 和 path。

## 即时写入

脚本中的 host 写入是即时生效的。例如：

```vela
player.level += 1;
```

会被解释为 host read、本地计算、host write-through。后续脚本如果发生错误，已经写入的 Rust 状态不会自动回滚。

## Access 模型

- `HostRef` 标识外部 host 对象。
- `HostPath` 标识 host root 下的字段或索引子路径。
- `PathProxy` 在脚本表达式中携带 host path 意图。
- `HostAccess` 负责权限检查，并路由读、写、复合写和 host method call。

脚本永远不会拿到真实 Rust 引用。调用边界传入的 Rust `&mut T` 会变成调用期可写 host handle，而不是可以被脚本保存的 Rust borrow。

## 类型方法

注册的 host type 可以包含字段、方法和可选 index capability。`HashMap<i32, i32>`、`Vec<Item>`、`HashSet<String>` 和 trait object surface 在脚本侧都使用同一个“具体 host type”模型。

如果 receiver 类型没有注册对应方法或 index 能力，编译器或 runtime 会给出明确错误，而不是猜测行为。
