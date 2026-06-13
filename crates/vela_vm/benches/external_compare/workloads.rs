pub(crate) struct Workload {
    pub(crate) name: &'static str,
    pub(crate) vela: &'static str,
    pub(crate) lua: &'static str,
    pub(crate) rhai: &'static str,
    pub(crate) node: &'static str,
    pub(crate) python: &'static str,
}

pub(crate) const WORKLOADS: &[Workload] = &[
    Workload {
        name: "scalar_branch_loop",
        vela: r#"
fn run_once() {
    let total = 0;
    for value in 0..200 {
        if value % 3 == 0 {
            total += value * 2;
            continue;
        }
        if value > 180 {
            break;
        }
        total += (value * 5) % 17;
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
    for value = 0, 199 do
        if value % 3 == 0 then
            total = total + value * 2
        elseif value > 180 then
            break
        else
            total = total + (value * 5) % 17
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
    for value in 0..200 {
        if value % 3 == 0 {
            total += value * 2;
        } else if value > 180 {
            break;
        } else {
            total += (value * 5) % 17;
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
    for (let value = 0; value < 200; value += 1) {
        if (value % 3 === 0) {
            total += value * 2;
            continue;
        }
        if (value > 180) {
            break;
        }
        total += (value * 5) % 17;
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
    for value in range(200):
        if value % 3 == 0:
            total += value * 2
            continue
        if value > 180:
            break
        total += (value * 5) % 17
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
"#,
    },
    Workload {
        name: "range_iteration",
        vela: r#"
fn run_once() {
    let total = 0;
    for outer in 0..8 {
        for value in 0..128 {
            total += value + outer - outer;
        }
    }
    for value in 0..=63 {
        total += value;
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
    for outer = 0, 7 do
        for value = 0, 127 do
            total = total + value + outer - outer
        end
    end
    for value = 0, 63 do
        total = total + value
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
    for outer in 0..8 {
        for value in 0..128 {
            total += value + outer - outer;
        }
    }
    for value in 0..=63 {
        total += value;
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
    for (let outer = 0; outer < 8; outer += 1) {
        for (let value = 0; value < 128; value += 1) {
            total += value + outer - outer;
        }
    }
    for (let value = 0; value <= 63; value += 1) {
        total += value;
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
    for outer in range(8):
        for value in range(128):
            total += value + outer - outer
    for value in range(64):
        total += value
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
"#,
    },
    Workload {
        name: "function_calls",
        vela: r#"
fn add_one(value) {
    return value + 1;
}

fn mix_pair(left, right) {
    return left * 3 + right;
}

fn run_once() {
    let total = 0;
    for tick in 0..240 {
        total += add_one(tick);
        total += mix_pair(tick, total % 17);
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
function add_one(value)
    return value + 1
end

function mix_pair(left, right)
    return left * 3 + right
end

function run_once()
    local total = 0
    for tick = 0, 239 do
        total = total + add_one(tick)
        total = total + mix_pair(tick, total % 17)
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
fn add_one(value) {
    value + 1
}

fn mix_pair(left, right) {
    left * 3 + right
}

fn run_once() {
    let total = 0;
    for tick in 0..240 {
        total += add_one(tick);
        total += mix_pair(tick, total % 17);
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
function addOne(value) { return value + 1; }
function mixPair(left, right) { return left * 3 + right; }
function runOnce() {
    let total = 0;
    for (let tick = 0; tick < 240; tick += 1) {
        total += addOne(tick);
        total += mixPair(tick, total % 17);
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
def add_one(value):
    return value + 1
def mix_pair(left, right):
    return left * 3 + right
def run_once():
    total = 0
    for tick in range(240):
        total += add_one(tick)
        total += mix_pair(tick, total % 17)
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
"#,
    },
    Workload {
        name: "array_scan",
        vela: r#"
fn run_once() {
    let values = [3, 1, 4, 1, 5, 9, 2, 6];
    let total = 0;
    for tick in 0..200 {
        for value in values {
            if value % 2 == 0 {
                total += (value * tick) % 17;
            } else {
                total += value + tick % 5;
            }
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
local values = {3, 1, 4, 1, 5, 9, 2, 6}
function run_once()
    local total = 0
    for tick = 0, 199 do
        for _, value in ipairs(values) do
            if value % 2 == 0 then
                total = total + (value * tick) % 17
            else
                total = total + value + tick % 5
            end
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
    let values = [3, 1, 4, 1, 5, 9, 2, 6];
    let total = 0;
    for tick in 0..200 {
        for value in values {
            if value % 2 == 0 {
                total += (value * tick) % 17;
            } else {
                total += value + tick % 5;
            }
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
const values = [3, 1, 4, 1, 5, 9, 2, 6];
function runOnce() {
    let total = 0;
    for (let tick = 0; tick < 200; tick += 1) {
        for (const value of values) {
            if (value % 2 === 0) {
                total += (value * tick) % 17;
            } else {
                total += value + tick % 5;
            }
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
values = [3, 1, 4, 1, 5, 9, 2, 6]
def run_once():
    total = 0
    for tick in range(200):
        for value in values:
            if value % 2 == 0:
                total += (value * tick) % 17
            else:
                total += value + tick % 5
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
"#,
    },
    Workload {
        name: "map_lookup_update",
        vela: r#"
fn run_once() {
    let scores = {"quest": 3, "raid": 8, "daily": 2};
    let total = 0;
    for tick in 0..120 {
        scores["quest"] = scores["quest"] + tick % 5;
        scores["daily"] = scores.get_or("daily", 0) + 1;
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
        scores.quest = scores.quest + tick % 5
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
        scores.quest += tick % 5;
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
        scores["quest"] = scores["quest"] + tick % 5
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
        name: "set_lookup_mutation",
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
        add(active, "event");
        add(active, "quest");
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
        name: "string_methods",
        vela: r#"
fn run_once() {
    let total = 0;
    let labels = ["quest", "raid", "daily", "bonus"];
    for tick in 0..50 {
        for label in labels {
            if label.starts_with("q") || label.contains("i") {
                total += label.len() + tick % 7;
            } else {
                total += label.len();
            }
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
    local labels = {"quest", "raid", "daily", "bonus"}
    for tick = 0, 49 do
        for _, label in ipairs(labels) do
            if string.sub(label, 1, 1) == "q" or string.find(label, "i", 1, true) ~= nil then
                total = total + #label + tick % 7
            else
                total = total + #label
            end
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
    let labels = ["quest", "raid", "daily", "bonus"];
    for tick in 0..50 {
        for label in labels {
            if label.starts_with("q") || label.contains("i") {
                total += label.len() + tick % 7;
            } else {
                total += label.len();
            }
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
    const labels = ["quest", "raid", "daily", "bonus"];
    for (let tick = 0; tick < 50; tick += 1) {
        for (const label of labels) {
            if (label.startsWith("q") || label.includes("i")) {
                total += label.length + tick % 7;
            } else {
                total += label.length;
            }
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
    labels = ["quest", "raid", "daily", "bonus"]
    for tick in range(50):
        for label in labels:
            if label.startswith("q") or "i" in label:
                total += len(label) + tick % 7
            else:
                total += len(label)
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
"#,
    },
    Workload {
        name: "closure_callbacks",
        vela: r#"
fn run_once() {
    let total = 0;
    for tick in 0..120 {
        let add = |value| value + tick;
        let mix = |left, right| left * 3 + right + tick - tick;
        total += add(tick);
        total += add(total % 17);
        total += mix(tick, total % 23);
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
    for tick = 0, 119 do
        local function add(value)
            return value + tick
        end
        local function mix(left, right)
            return left * 3 + right + tick - tick
        end
        total = total + add(tick)
        total = total + add(total % 17)
        total = total + mix(tick, total % 23)
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
fn add(value, tick) {
    value + tick
}

fn mix(left, right, tick) {
    left * 3 + right + tick - tick
}

fn run_once() {
    let total = 0;
    for tick in 0..120 {
        total += add(tick, tick);
        total += add(total % 17, tick);
        total += mix(tick, total % 23, tick);
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
    for (let tick = 0; tick < 120; tick += 1) {
        const add = (value) => value + tick;
        const mix = (left, right) => left * 3 + right + tick - tick;
        total += add(tick);
        total += add(total % 17);
        total += mix(tick, total % 23);
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
    for tick in range(120):
        add = lambda value, tick=tick: value + tick
        mix = lambda left, right, tick=tick: left * 3 + right + tick - tick
        total += add(tick)
        total += add(total % 17)
        total += mix(tick, total % 23)
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
"#,
    },
];
