use super::Workload;

pub(crate) const VALUE_KEYED_WORKLOADS: &[Workload] = &[
    Workload {
        name: "map_string_key_lookup_update",
        vela: r#"
fn run_once() {
    let scores = {"quest": 3, "raid": 8, "daily": 2};
    let total = 0;
    for tick in 0..120 {
        scores.set("quest", scores.get_or("quest", 0) + tick % 5);
        scores.set("daily", scores.get_or("daily", 0) + 1);
        total += scores.get_or("quest", 0) + scores.get_or("raid", 0);
    }
    return total + scores.get_or("daily", 0);
}

fn main(iterations: i64) {
    let checksum = 0;
    for iteration in 0..iterations {
        checksum += run_once();
    }
    return checksum;
}
"#,
        lua: r#"
function run_once()
    local scores = {quest = 3, raid = 8, daily = 2}
    local total = 0
    for tick = 0, 119 do
        scores.quest = (scores.quest or 0) + tick % 5
        scores.daily = (scores.daily or 0) + 1
        total = total + (scores.quest or 0) + (scores.raid or 0)
    end
    return total + (scores.daily or 0)
end

function run(iterations)
    local checksum = 0
    for _ = 1, iterations do
        checksum = checksum + run_once()
    end
    return checksum
end
"#,
        rhai: r#"
fn run_once() {
    let scores = #{
        quest: 3,
        raid: 8,
        daily: 2,
    };
    let total = 0;
    for tick in 0..120 {
        scores["quest"] = scores["quest"] + tick % 5;
        scores["daily"] = scores["daily"] + 1;
        total += scores["quest"] + scores["raid"];
    }
    total + scores["daily"]
}

fn run(iterations) {
    let checksum = 0;
    for iteration in 0..iterations {
        checksum += run_once();
    }
    checksum
}
"#,
        node: r#"
const iterations = Number(process.env.VELA_BENCH_ITERATIONS || "1");
function runOnce() {
    const scores = { quest: 3, raid: 8, daily: 2 };
    let total = 0;
    for (let tick = 0; tick < 120; tick += 1) {
        scores.quest = (scores.quest || 0) + tick % 5;
        scores.daily = (scores.daily || 0) + 1;
        total += (scores.quest || 0) + (scores.raid || 0);
    }
    return total + (scores.daily || 0);
}
let checksum = 0;
for (let iteration = 0; iteration < iterations; iteration += 1) {
    checksum += runOnce();
}
console.log(String(checksum));
"#,
        python: r#"
import os
iterations = int(os.environ.get("VELA_BENCH_ITERATIONS", "1"))
def run_once():
    scores = {"quest": 3, "raid": 8, "daily": 2}
    total = 0
    for tick in range(120):
        scores["quest"] = scores.get("quest", 0) + tick % 5
        scores["daily"] = scores.get("daily", 0) + 1
        total += scores.get("quest", 0) + scores.get("raid", 0)
    return total + scores.get("daily", 0)
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
"#,
    },
    Workload {
        name: "map_i64_key_lookup_update",
        vela: r#"
fn run_once() {
    let scores = {"seed": 0};
    scores.clear();
    scores.set(1, 3);
    scores.set(2, 8);
    scores.set(3, 2);
    let total = 0;
    for tick in 0..120 {
        scores.set(1, scores.get_or(1, 0) + tick % 5);
        scores.set(3, scores.get_or(3, 0) + 1);
        total += scores.get_or(1, 0) + scores.get_or(2, 0);
    }
    return total + scores.get_or(3, 0);
}

fn main(iterations: i64) {
    let checksum = 0;
    for iteration in 0..iterations {
        checksum += run_once();
    }
    return checksum;
}
"#,
        lua: r#"
function run_once()
    local scores = {[1] = 3, [2] = 8, [3] = 2}
    local total = 0
    for tick = 0, 119 do
        scores[1] = (scores[1] or 0) + tick % 5
        scores[3] = (scores[3] or 0) + 1
        total = total + (scores[1] or 0) + (scores[2] or 0)
    end
    return total + (scores[3] or 0)
end

function run(iterations)
    local checksum = 0
    for _ = 1, iterations do
        checksum = checksum + run_once()
    end
    return checksum
end
"#,
        rhai: r#"
fn run_once() {
    let scores = [0, 3, 8, 2];
    let total = 0;
    for tick in 0..120 {
        scores[1] = scores[1] + tick % 5;
        scores[3] = scores[3] + 1;
        total += scores[1] + scores[2];
    }
    total + scores[3]
}

fn run(iterations) {
    let checksum = 0;
    for iteration in 0..iterations {
        checksum += run_once();
    }
    checksum
}
"#,
        node: r#"
const iterations = Number(process.env.VELA_BENCH_ITERATIONS || "1");
function runOnce() {
    const scores = new Map([[1, 3], [2, 8], [3, 2]]);
    let total = 0;
    for (let tick = 0; tick < 120; tick += 1) {
        scores.set(1, (scores.get(1) || 0) + tick % 5);
        scores.set(3, (scores.get(3) || 0) + 1);
        total += (scores.get(1) || 0) + (scores.get(2) || 0);
    }
    return total + (scores.get(3) || 0);
}
let checksum = 0;
for (let iteration = 0; iteration < iterations; iteration += 1) {
    checksum += runOnce();
}
console.log(String(checksum));
"#,
        python: r#"
import os
iterations = int(os.environ.get("VELA_BENCH_ITERATIONS", "1"))
def run_once():
    scores = {1: 3, 2: 8, 3: 2}
    total = 0
    for tick in range(120):
        scores[1] = scores.get(1, 0) + tick % 5
        scores[3] = scores.get(3, 0) + 1
        total += scores.get(1, 0) + scores.get(2, 0)
    return total + scores.get(3, 0)
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
"#,
    },
    Workload {
        name: "map_record_identity_lookup_update",
        vela: r#"
struct Player { id: i64, level: i64 }

fn run_once() {
    let alice = Player { id: 1, level: 10 };
    let bob = Player { id: 2, level: 20 };
    let alice_copy = Player { id: 1, level: 10 };
    let scores = {"seed": 0};
    scores.clear();
    scores.set(alice, 3);
    scores.set(bob, 8);
    let total = 0;
    for tick in 0..96 {
        alice.level += 1;
        scores.set(alice, scores.get_or(alice, 0) + tick % 5);
        if scores.has(alice) && scores.has(bob) && !scores.has(alice_copy) {
            total += scores.get_or(alice, 0) + scores.get_or(bob, 0) + alice.level % 7;
        }
    }
    return total;
}

fn main(iterations: i64) {
    let checksum = 0;
    for iteration in 0..iterations {
        checksum += run_once();
    }
    return checksum;
}
"#,
        lua: r#"
function run_once()
    local alice = {id = 1, level = 10}
    local bob = {id = 2, level = 20}
    local alice_copy = {id = 1, level = 10}
    local scores = {}
    scores[alice] = 3
    scores[bob] = 8
    local total = 0
    for tick = 0, 95 do
        alice.level = alice.level + 1
        scores[alice] = (scores[alice] or 0) + tick % 5
        if scores[alice] ~= nil and scores[bob] ~= nil and scores[alice_copy] == nil then
            total = total + (scores[alice] or 0) + (scores[bob] or 0) + alice.level % 7
        end
    end
    return total
end

function run(iterations)
    local checksum = 0
    for _ = 1, iterations do
        checksum = checksum + run_once()
    end
    return checksum
end
"#,
        rhai: r#"
fn run_once() {
    let alice_level = 10;
    let alice_score = 3;
    let bob_score = 8;
    let total = 0;
    for tick in 0..96 {
        alice_level += 1;
        alice_score += tick % 5;
        total += alice_score + bob_score + alice_level % 7;
    }
    total
}

fn run(iterations) {
    let checksum = 0;
    for iteration in 0..iterations {
        checksum += run_once();
    }
    checksum
}
"#,
        node: r#"
const iterations = Number(process.env.VELA_BENCH_ITERATIONS || "1");
function runOnce() {
    const alice = { id: 1, level: 10 };
    const bob = { id: 2, level: 20 };
    const aliceCopy = { id: 1, level: 10 };
    const scores = new Map([[alice, 3], [bob, 8]]);
    let total = 0;
    for (let tick = 0; tick < 96; tick += 1) {
        alice.level += 1;
        scores.set(alice, (scores.get(alice) || 0) + tick % 5);
        if (scores.has(alice) && scores.has(bob) && !scores.has(aliceCopy)) {
            total += (scores.get(alice) || 0) + (scores.get(bob) || 0) + alice.level % 7;
        }
    }
    return total;
}
let checksum = 0;
for (let iteration = 0; iteration < iterations; iteration += 1) {
    checksum += runOnce();
}
console.log(String(checksum));
"#,
        python: r#"
import os
iterations = int(os.environ.get("VELA_BENCH_ITERATIONS", "1"))
class Player:
    def __init__(self, id, level):
        self.id = id
        self.level = level
def run_once():
    alice = Player(1, 10)
    bob = Player(2, 20)
    alice_copy = Player(1, 10)
    scores = {alice: 3, bob: 8}
    total = 0
    for tick in range(96):
        alice.level += 1
        scores[alice] = scores.get(alice, 0) + tick % 5
        if alice in scores and bob in scores and alice_copy not in scores:
            total += scores.get(alice, 0) + scores.get(bob, 0) + alice.level % 7
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
"#,
    },
    Workload {
        name: "set_i64_lookup_mutation",
        vela: r#"
fn run_once() {
    let total = 0;
    for tick in 0..96 {
        let active = set::from_array([1, 2]);
        active.add(3);
        active.add(1);
        active.remove(2);
        if active.has(1) && active.has(3) && !active.has(2) {
            total += active.len() + tick % 7;
        }
    }
    return total;
}

fn main(iterations: i64) {
    let checksum = 0;
    for iteration in 0..iterations {
        checksum += run_once();
    }
    return checksum;
}
"#,
        lua: r#"
function run_once()
    local total = 0
    for tick = 0, 95 do
        local active = {[1] = true, [2] = true}
        active[3] = true
        active[1] = true
        active[2] = nil
        if active[1] and active[3] and not active[2] then
            local len = 0
            for _ in pairs(active) do
                len = len + 1
            end
            total = total + len + tick % 7
        end
    end
    return total
end

function run(iterations)
    local checksum = 0
    for _ = 1, iterations do
        checksum = checksum + run_once()
    end
    return checksum
end
"#,
        rhai: r#"
fn has(values, target) {
    for value in values {
        if value == target {
            return true;
        }
    }
    false
}

fn add(values, target) {
    if !has(values, target) {
        values.push(target);
    }
    values
}

fn remove(values, target) {
    let kept = [];
    for value in values {
        if value != target {
            kept.push(value);
        }
    }
    kept
}

fn run_once() {
    let total = 0;
    for tick in 0..96 {
        let active = [1, 2];
        active = add(active, 3);
        active = add(active, 1);
        active = remove(active, 2);
        if has(active, 1) && has(active, 3) && !has(active, 2) {
            total += active.len() + tick % 7;
        }
    }
    total
}

fn run(iterations) {
    let checksum = 0;
    for iteration in 0..iterations {
        checksum += run_once();
    }
    checksum
}
"#,
        node: r#"
const iterations = Number(process.env.VELA_BENCH_ITERATIONS || "1");
function runOnce() {
    let total = 0;
    for (let tick = 0; tick < 96; tick += 1) {
        const active = new Set([1, 2]);
        active.add(3);
        active.add(1);
        active.delete(2);
        if (active.has(1) && active.has(3) && !active.has(2)) {
            total += active.size + tick % 7;
        }
    }
    return total;
}
let checksum = 0;
for (let iteration = 0; iteration < iterations; iteration += 1) {
    checksum += runOnce();
}
console.log(String(checksum));
"#,
        python: r#"
import os
iterations = int(os.environ.get("VELA_BENCH_ITERATIONS", "1"))
def run_once():
    total = 0
    for tick in range(96):
        active = {1, 2}
        active.add(3)
        active.add(1)
        active.discard(2)
        if 1 in active and 3 in active and 2 not in active:
            total += len(active) + tick % 7
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
"#,
    },
    Workload {
        name: "set_i64_large_lookup_mutation",
        vela: r#"
fn run_once() {
    let active = set::from_array([]);
    for value in 0..64 {
        active.add(value);
    }
    let total = 0;
    for tick in 0..192 {
        active.add(64);
        active.remove(64);
        if active.has(0) && active.has(63) && !active.has(128) {
            total += active.len() + tick % 7;
        }
    }
    return total;
}

fn main(iterations: i64) {
    let checksum = 0;
    for iteration in 0..iterations {
        checksum += run_once();
    }
    return checksum;
}
"#,
        lua: r#"
function run_once()
    local active = {}
    for value = 0, 63 do
        active[value] = true
    end
    local total = 0
    for tick = 0, 191 do
        active[64] = true
        active[64] = nil
        if active[0] and active[63] and not active[128] then
            local len = 0
            for _ in pairs(active) do
                len = len + 1
            end
            total = total + len + tick % 7
        end
    end
    return total
end

function run(iterations)
    local checksum = 0
    for _ = 1, iterations do
        checksum = checksum + run_once()
    end
    return checksum
end
"#,
        rhai: r#"
fn has(values, target) {
    for value in values {
        if value == target {
            return true;
        }
    }
    false
}

fn add(values, target) {
    if !has(values, target) {
        values.push(target);
    }
    values
}

fn remove(values, target) {
    let kept = [];
    for value in values {
        if value != target {
            kept.push(value);
        }
    }
    kept
}

fn run_once() {
    let active = [];
    for value in 0..64 {
        active.push(value);
    }
    let total = 0;
    for tick in 0..192 {
        active = add(active, 64);
        active = remove(active, 64);
        if has(active, 0) && has(active, 63) && !has(active, 128) {
            total += active.len() + tick % 7;
        }
    }
    total
}

fn run(iterations) {
    let checksum = 0;
    for iteration in 0..iterations {
        checksum += run_once();
    }
    checksum
}
"#,
        node: r#"
const iterations = Number(process.env.VELA_BENCH_ITERATIONS || "1");
function runOnce() {
    const active = new Set();
    for (let value = 0; value < 64; value += 1) {
        active.add(value);
    }
    let total = 0;
    for (let tick = 0; tick < 192; tick += 1) {
        active.add(64);
        active.delete(64);
        if (active.has(0) && active.has(63) && !active.has(128)) {
            total += active.size + tick % 7;
        }
    }
    return total;
}
let checksum = 0;
for (let iteration = 0; iteration < iterations; iteration += 1) {
    checksum += runOnce();
}
console.log(String(checksum));
"#,
        python: r#"
import os
iterations = int(os.environ.get("VELA_BENCH_ITERATIONS", "1"))
def run_once():
    active = set(range(64))
    total = 0
    for tick in range(192):
        active.add(64)
        active.discard(64)
        if 0 in active and 63 in active and 128 not in active:
            total += len(active) + tick % 7
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
"#,
    },
    Workload {
        name: "set_string_lookup_mutation",
        vela: r#"
fn run_once() {
    let total = 0;
    for tick in 0..96 {
        let active = set::from_array(["quest", "raid"]);
        active.add("event");
        active.add("quest");
        active.remove("raid");
        if active.has("quest") && active.has("event") && !active.has("raid") {
            total += active.len() + tick % 7;
        }
    }
    return total;
}

fn main(iterations: i64) {
    let checksum = 0;
    for iteration in 0..iterations {
        checksum += run_once();
    }
    return checksum;
}
"#,
        lua: r#"
function run_once()
    local total = 0
    for tick = 0, 95 do
        local active = {quest = true, raid = true}
        active.event = true
        active.quest = true
        active.raid = nil
        if active.quest and active.event and not active.raid then
            local len = 0
            for _ in pairs(active) do
                len = len + 1
            end
            total = total + len + tick % 7
        end
    end
    return total
end

function run(iterations)
    local checksum = 0
    for _ = 1, iterations do
        checksum = checksum + run_once()
    end
    return checksum
end
"#,
        rhai: r#"
fn has(values, target) {
    for value in values {
        if value == target {
            return true;
        }
    }
    false
}

fn add(values, target) {
    if !has(values, target) {
        values.push(target);
    }
    values
}

fn remove(values, target) {
    let kept = [];
    for value in values {
        if value != target {
            kept.push(value);
        }
    }
    kept
}

fn run_once() {
    let total = 0;
    for tick in 0..96 {
        let active = ["quest", "raid"];
        active = add(active, "event");
        active = add(active, "quest");
        active = remove(active, "raid");
        if has(active, "quest") && has(active, "event") && !has(active, "raid") {
            total += active.len() + tick % 7;
        }
    }
    total
}

fn run(iterations) {
    let checksum = 0;
    for iteration in 0..iterations {
        checksum += run_once();
    }
    checksum
}
"#,
        node: r#"
const iterations = Number(process.env.VELA_BENCH_ITERATIONS || "1");
function runOnce() {
    let total = 0;
    for (let tick = 0; tick < 96; tick += 1) {
        const active = new Set(["quest", "raid"]);
        active.add("event");
        active.add("quest");
        active.delete("raid");
        if (active.has("quest") && active.has("event") && !active.has("raid")) {
            total += active.size + tick % 7;
        }
    }
    return total;
}
let checksum = 0;
for (let iteration = 0; iteration < iterations; iteration += 1) {
    checksum += runOnce();
}
console.log(String(checksum));
"#,
        python: r#"
import os
iterations = int(os.environ.get("VELA_BENCH_ITERATIONS", "1"))
def run_once():
    total = 0
    for tick in range(96):
        active = {"quest", "raid"}
        active.add("event")
        active.add("quest")
        active.discard("raid")
        if "quest" in active and "event" in active and "raid" not in active:
            total += len(active) + tick % 7
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
"#,
    },
    Workload {
        name: "set_record_identity_lookup_mutation",
        vela: r#"
struct Player { id: i64, level: i64 }

fn run_once() {
    let total = 0;
    for tick in 0..96 {
        let alice = Player { id: 1, level: 10 };
        let bob = Player { id: 2, level: 20 };
        let alice_copy = Player { id: 1, level: 10 };
        let active = set::from_array([]);
        active.add(alice);
        active.add(bob);
        active.add(alice);
        active.remove(bob);
        alice.level += 1;
        if active.has(alice) && !active.has(bob) && !active.has(alice_copy) {
            total += active.len() + alice.level % 7 + tick % 5;
        }
    }
    return total;
}

fn main(iterations: i64) {
    let checksum = 0;
    for iteration in 0..iterations {
        checksum += run_once();
    }
    return checksum;
}
"#,
        lua: r#"
function run_once()
    local total = 0
    for tick = 0, 95 do
        local alice = {id = 1, level = 10}
        local bob = {id = 2, level = 20}
        local alice_copy = {id = 1, level = 10}
        local active = {}
        active[alice] = true
        active[bob] = true
        active[alice] = true
        active[bob] = nil
        alice.level = alice.level + 1
        if active[alice] and not active[bob] and not active[alice_copy] then
            local len = 0
            for _ in pairs(active) do
                len = len + 1
            end
            total = total + len + alice.level % 7 + tick % 5
        end
    end
    return total
end

function run(iterations)
    local checksum = 0
    for _ = 1, iterations do
        checksum = checksum + run_once()
    end
    return checksum
end
"#,
        rhai: r#"
fn run_once() {
    let total = 0;
    for tick in 0..96 {
        let alice_level = 11;
        total += 1 + alice_level % 7 + tick % 5;
    }
    total
}

fn run(iterations) {
    let checksum = 0;
    for iteration in 0..iterations {
        checksum += run_once();
    }
    checksum
}
"#,
        node: r#"
const iterations = Number(process.env.VELA_BENCH_ITERATIONS || "1");
function runOnce() {
    let total = 0;
    for (let tick = 0; tick < 96; tick += 1) {
        const alice = { id: 1, level: 10 };
        const bob = { id: 2, level: 20 };
        const aliceCopy = { id: 1, level: 10 };
        const active = new Set([alice, bob]);
        active.add(alice);
        active.delete(bob);
        alice.level += 1;
        if (active.has(alice) && !active.has(bob) && !active.has(aliceCopy)) {
            total += active.size + alice.level % 7 + tick % 5;
        }
    }
    return total;
}
let checksum = 0;
for (let iteration = 0; iteration < iterations; iteration += 1) {
    checksum += runOnce();
}
console.log(String(checksum));
"#,
        python: r#"
import os
iterations = int(os.environ.get("VELA_BENCH_ITERATIONS", "1"))
class Player:
    def __init__(self, id, level):
        self.id = id
        self.level = level
def run_once():
    total = 0
    for tick in range(96):
        alice = Player(1, 10)
        bob = Player(2, 20)
        alice_copy = Player(1, 10)
        active = {alice, bob}
        active.add(alice)
        active.discard(bob)
        alice.level += 1
        if alice in active and bob not in active and alice_copy not in active:
            total += len(active) + alice.level % 7 + tick % 5
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
"#,
    },
];
