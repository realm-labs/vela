---
title: "动态方法分发"
description: "Vela 动态方法分发文档。"
---

动态方法分发允许普通动态值在编译期不知道 receiver 类型时调用源码中静态写出的 Method Name。它是受控的、registry-backed 分发，不是 monkey patching。

## 编译期拆分

如果 receiver 类型已知且方法存在，编译器会生成 resolved method dispatch。如果 receiver 未知，会生成带源码方法名和原始参数信息的 dynamic dispatch。

```vela
fn length(value) -> i64 {
    return value.len()
}
```

## 解析顺序

运行时会先分类 receiver，然后按固定顺序解析：标准 value method、脚本 impl method、宿主 method，最后才是带源码位置的 missing-method 错误。

```vela
fn starts_with_q(value) -> bool {
    return value.starts_with("q")
}
```

## 参数和 Guard

Dynamic bytecode 会保留位置参数和命名参数，直到目标确定。解析目标后，运行时根据目标签名组装参数，在支持的位置补默认值，并运行类型或宿主转换 guard。

## Cache 边界

动态方法 cache 由方法名、receiver 分类，以及相关 program 或宿主 schema epoch 保护。Cache miss 会回退到解析；它本身不是语言错误。

## 宿主安全

动态宿主方法分发仍然经过 `HostRef`、`HostPath`、`PathProxy`、HostAccess、注册元数据、能力检查和 generation 检查，不能绕过宿主修改模型。
