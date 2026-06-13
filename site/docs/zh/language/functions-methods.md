# 函数和方法

函数用 `fn` 声明并按名字调用。方法可以是脚本类型上的 script method，也可以是 Rust 注册的 host method。

## 函数

```vela
fn add(left, right) {
    return left + right;
}

fn main() {
    return add(20, 22);
}
```

## Script Method

```vela
struct DamageResult {
    applied: int,
}

impl DamageResult {
    fn score(self, bonus) -> int {
        return self.applied + bonus;
    }
}
```

当多个类型需要共享协议或默认方法体时，再使用 trait：

```vela
trait DamageSummary {
    fn score(self, bonus) -> int;
}

impl DamageSummary for DamageResult {}
```

## Host Method

Rust 可以在具体 host type 上注册方法。脚本语法保持一致：

```vela
player.inventory.grant("gold", 10);
```

VM 会解析 receiver 类型和 method ID，然后通过 `HostAccess` 路由调用。

## 动态 Receiver 调用

如果编译器知道 receiver 类型，已存在的方法会使用 linked stable ID 快路径，
可证明不存在的方法仍然可以是编译期错误。如果 receiver 类型未知，源码中静态
写出的 method call 仍然会编译并 link：

```vela
fn starts_with_q(value) {
    return value.starts_with("q");
}
```

运行时 VM 会根据实际 receiver 解析方法。字符串、脚本值和已注册的 host ref
都可以通过这个路径派发。不支持该方法的 receiver 会产生带源码 span 的运行时
错误。

动态 script method 会在目标解析后处理 named argument 和默认参数：

```vela
fn wrapped(value) {
    return value.wrap(suffix = "}", prefix = "{");
}
```
