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
    applied: Int,
}

impl DamageResult {
    fn score(self, bonus) -> Int {
        return self.applied + bonus;
    }
}
```

当多个类型需要共享协议或默认方法体时，再使用 trait：

```vela
trait DamageSummary {
    fn score(self, bonus) -> Int;
}

impl DamageSummary for DamageResult {}
```

## Host Method

Rust 可以在具体 host type 上注册方法。脚本语法保持一致：

```vela
player.inventory.grant("gold", 10);
```

VM 会解析 receiver 类型和 method ID，然后通过 `HostAccess` 路由调用。
