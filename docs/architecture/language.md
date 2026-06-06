## Language Semantics

The first grammar draft lives in [grammar.ebnf](grammar.ebnf). It is the syntax
target for the parser milestones before semantic validation and lowering.

Parser implementations should preserve source spans for every token and AST
node. A future LSP needs this for diagnostics, completion replacement ranges,
go-to-definition, rename, hover, and incremental reparsing. The compiler may
lower into a simpler AST/HIR, but the syntax layer should keep a lossless CST or
equivalent token tree with comments and newlines.

Example script:

```rust
use game::player::Player
use game::reward::Reward

struct KillReward {
    item_id
    count
}

enum QuestProgress {
    None
    Active { quest_id, count }
    Finished { quest_id }
}

trait Damageable {
    fn damage(self, amount)
}

#[event("monster.kill")]
pub fn on_kill(ctx, player, monster) {
    player.exp += monster.exp

    if player.exp >= ctx.config.exp_to_next_level(player.level) {
        player.level += 1
        player.exp = 0
        ctx.emit("player.level_up", player.id, player.level)
    }

    let rewards = ctx.config.kill_rewards
        .filter(|r| r.monster_id == monster.id)
        .map(|r| KillReward {
            item_id: r.item_id,
            count: r.count,
        })

    for reward in rewards {
        player.inventory.add(reward.item_id, reward.count)
    }

    match player.quest_progress {
        QuestProgress::Active { quest_id, count } => {
            player.quest_progress = QuestProgress::Active {
                quest_id,
                count: count + 1,
            }
        }
        _ => {}
    }
}
```

### Dynamic Type Boundary

The language is dynamically typed, with lightweight hints and metadata.

Function overloading is not part of the language. A module may contain only one
function for a given name, and a type or trait may contain only one method for a
given receiver/name pair. Parameter count, type hints, default values, and
native Rust signatures do not create overload sets.

Different value categories may independently define the same method name, such
as string and array helpers, because dispatch starts from the receiver category
or reflected receiver type. That is receiver-based method dispatch, not
same-scope overload resolution.

Supported value categories:

```text
null
bool
int
float
string
array
map
set
range
function
closure
record / struct-like value
enum / tagged union
host ref
path proxy
trait/protocol object
```

Script generics are not supported:

```text
Array<T>      not supported
Map<K, V>     not supported
Option<T>     not supported
Result<T, E>  not supported
```

Dynamic enum definitions can still model Option and Result:

```rust
enum Option {
    Some(value)
    None
}

enum Result {
    Ok(value)
    Err(error)
}
```

`null`, `Option`, `Result`, and runtime errors have separate responsibilities:

```text
null        no meaningful value, void-like results, host nullable boundaries, or missing metadata
Option::None expected absence in gameplay or lookup logic
Result::Err  recoverable failure with a script-visible reason
VM error    unrecoverable trap, script bug, contract violation, budget failure, or sandbox denial
```

Script and standard-library APIs should prefer `Option` for expected missing
data and `Result` for expected recoverable failure. They should not use `null`
as the normal "not found" or "failed" result:: `null` remains the value for
statement-only blocks, no-result native calls, reflection metadata gaps, and
host/Rust nullable interop.

Control-flow expressions produce values. Empty or statement-only blocks
evaluate to `null`, and expression-valued `if` without an `else` evaluates to
`null` on the untaken branch.

### Dynamic Traits / Protocols

Traits are runtime capabilities or protocols, not Rust traits.

Supported:

```text
trait method declarations
trait default methods
host type implementations
script type implementations
runtime implements checks
dynamic trait method dispatch
```

Not supported:

```text
generic traits
associated types
complex where clauses
traits participating in object memory layout
static monomorphization
```

