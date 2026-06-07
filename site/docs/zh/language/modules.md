# 模块

Vela 模块由 compiler 解析源码文件得到。文件路径和模块名尽量保持一致，减少歧义。

## Import

```vela
use game::reward::grant_reward;

fn main(player) {
    return grant_reward(player);
}
```

## 声明

模块可以定义 public function、private helper、struct、enum、trait、impl、const 和 global。模块元数据属于热更新 ABI surface。

## 为什么需要模块图

模块图为 compiler 提供稳定 declaration ID、import resolution、依赖追踪和热更新影响分析。它也为未来 tooling 提供干净的语义模型，而不引入 runtime monkey patching。
