# Playground

Playground 通过一个很薄的 WASM wrapper 在浏览器中运行 `vela_engine`。

## 支持能力

- 编译诊断。
- 运行时诊断。
- Record、enum、method、array、map、set、string、math helper、Option/Result helper、受控 time 和受控 random。
- 预置示例和可编辑源码。

## 沙箱边界

浏览器 playground 不暴露 Rust host object、文件系统 I/O 或真实服务器状态。这是有意设计。Host bridge 是 Rust embedding 能力，所以修改 Rust-owned 状态的例子放在 `examples/src/bin` 中。

## 返回值

`run_script` 返回脚本值的 JSON 表示：

```vela
fn main() {
    return { "gold": 10, "xp": 25 };
}
```

页面会在 Output 面板显示 JSON 结果。诊断单独展示，避免错误信息和返回值混在一起。
