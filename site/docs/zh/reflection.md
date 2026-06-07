# 反射

反射向脚本暴露元数据和受控值操作。

## 元数据

脚本可以查询：

- Type、field、variant、trait、method。
- Module 和 function。
- Required permission 和 declared effect。
- Unknown name 的候选提示。

## 受控修改

反射可以在 policy 允许时执行受控 read、write 和 call。它不能在运行时修改类型结构。

这意味着反射适合诊断、工具、debug view 和动态业务流程，但它不是 monkey patching 系统。

## 权限

反射有独立的 read、write 和 call 权限。Host field access 仍然走 `HostAccess`，所以反射不会绕过 host 边界。
