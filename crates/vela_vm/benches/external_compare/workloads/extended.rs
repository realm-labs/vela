use super::Workload;

pub(crate) const EXTENDED_WORKLOADS: &[Workload] = &[
    Workload {
        name: "recursive_countdown",
        vela: r#"
fn descend(value) {
    if value <= 0 {
        return 0;
    }
    return value + descend(value - 1);
}

fn run_once() {
    let total = 0;
    for value in 16..24 {
        total += descend(value);
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
function descend(value)
    if value <= 0 then
        return 0
    end
    return value + descend(value - 1)
end

function run_once()
    local total = 0
    for value = 16, 23 do
        total = total + descend(value)
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
fn descend(value) {
    if value <= 0 {
        return 0;
    }
    return value + descend(value - 1);
}

fn run_once() {
    let total = 0;
    for value in 16..24 {
        total += descend(value);
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
function descend(value) {
    if (value <= 0) {
        return 0;
    }
    return value + descend(value - 1);
}
function runOnce() {
    let total = 0;
    for (let value = 16; value < 24; value += 1) {
        total += descend(value);
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
def descend(value):
    if value <= 0:
        return 0
    return value + descend(value - 1)
def run_once():
    total = 0
    for value in range(16, 24):
        total += descend(value)
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
"#,
    },
    Workload {
        name: "nested_collection_allocation",
        vela: r#"
fn run_once() {
    let total = 0;
    for tick in 0..64 {
        let rows = [
            [tick, tick + 1, tick + 2],
            [tick + 3, tick + 4, tick + 5],
            [tick + 6, tick + 7, tick + 8],
        ];
        let labels = {
            "alpha": tick,
            "beta": tick + 2,
            "gamma": tick + 4,
        };
        total += rows[0][1] + rows[1][2] + rows[2][0];
        total += labels["alpha"] + labels["gamma"];
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
    for tick = 0, 63 do
        local rows = {
            {tick, tick + 1, tick + 2},
            {tick + 3, tick + 4, tick + 5},
            {tick + 6, tick + 7, tick + 8},
        }
        local labels = {
            alpha = tick,
            beta = tick + 2,
            gamma = tick + 4,
        }
        total = total + rows[1][2] + rows[2][3] + rows[3][1]
        total = total + labels["alpha"] + labels["gamma"]
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
    for tick in 0..64 {
        let rows = [
            [tick, tick + 1, tick + 2],
            [tick + 3, tick + 4, tick + 5],
            [tick + 6, tick + 7, tick + 8],
        ];
        let labels = #{
            "alpha": tick,
            "beta": tick + 2,
            "gamma": tick + 4,
        };
        total += rows[0][1] + rows[1][2] + rows[2][0];
        total += labels["alpha"] + labels["gamma"];
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
    for (let tick = 0; tick < 64; tick += 1) {
        const rows = [
            [tick, tick + 1, tick + 2],
            [tick + 3, tick + 4, tick + 5],
            [tick + 6, tick + 7, tick + 8],
        ];
        const labels = {
            alpha: tick,
            beta: tick + 2,
            gamma: tick + 4,
        };
        total += rows[0][1] + rows[1][2] + rows[2][0];
        total += labels["alpha"] + labels["gamma"];
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
    for tick in range(64):
        rows = [
            [tick, tick + 1, tick + 2],
            [tick + 3, tick + 4, tick + 5],
            [tick + 6, tick + 7, tick + 8],
        ]
        labels = {
            "alpha": tick,
            "beta": tick + 2,
            "gamma": tick + 4,
        }
        total += rows[0][1] + rows[1][2] + rows[2][0]
        total += labels["alpha"] + labels["gamma"]
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
"#,
    },
    Workload {
        name: "object_field_methods",
        vela: r#"
struct Counter {
    value,
    step,
}

impl Counter {
    fn bump(self, extra) {
        return self.value + self.step + extra;
    }
}

fn run_once() {
    let total = 0;
    for tick in 0..96 {
        let counter = Counter { value: tick, step: tick % 5 + 1 };
        total += counter.bump(tick % 3);
        total += counter.value + counter.step;
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
function bump(counter, extra)
    return counter.value + counter.step + extra
end

function run_once()
    local total = 0
    for tick = 0, 95 do
        local counter = { value = tick, step = tick % 5 + 1 }
        total = total + bump(counter, tick % 3)
        total = total + counter.value + counter.step
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
fn bump(counter, extra) {
    counter.value + counter.step + extra
}

fn run_once() {
    let total = 0;
    for tick in 0..96 {
        let counter = #{ value: tick, step: tick % 5 + 1 };
        total += bump(counter, tick % 3);
        total += counter.value + counter.step;
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
class Counter {
    constructor(value, step) {
        this.value = value;
        this.step = step;
    }
    bump(extra) {
        return this.value + this.step + extra;
    }
}
function runOnce() {
    let total = 0;
    for (let tick = 0; tick < 96; tick += 1) {
        const counter = new Counter(tick, tick % 5 + 1);
        total += counter.bump(tick % 3);
        total += counter.value + counter.step;
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
class Counter:
    def __init__(self, value, step):
        self.value = value
        self.step = step
    def bump(self, extra):
        return self.value + self.step + extra
def run_once():
    total = 0
    for tick in range(96):
        counter = Counter(tick, tick % 5 + 1)
        total += counter.bump(tick % 3)
        total += counter.value + counter.step
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
"#,
    },
    Workload {
        name: "string_build_split_join",
        vela: r#"
fn run_once() {
    let total = 0;
    for tick in 0..72 {
        let text = "alpha,beta,gamma,delta";
        let parts = text.split(",");
        let joined = parts.join(":");
        total += joined.len() + 10 + tick % 11;
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
function split_commas(text)
    local parts = {}
    for part in string.gmatch(text, "([^,]+)") do
        parts[#parts + 1] = part
    end
    return parts
end

function run_once()
    local total = 0
    for tick = 0, 71 do
        local text = "alpha,beta,gamma,delta"
        local parts = split_commas(text)
        local joined = table.concat(parts, ":")
        total = total + #joined + 10 + tick % 11
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
    for tick in 0..72 {
        let text = "alpha,beta,gamma,delta";
        let parts = text.split(",");
        let joined = parts[0] + ":" + parts[1] + ":" + parts[2] + ":" + parts[3];
        total += joined.len() + 10 + tick % 11;
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
    for (let tick = 0; tick < 72; tick += 1) {
        const text = "alpha,beta,gamma,delta";
        const parts = text.split(",");
        const joined = parts.join(":");
        total += joined.length + 10 + tick % 11;
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
    for tick in range(72):
        text = "alpha,beta,gamma,delta"
        parts = text.split(",")
        joined = ":".join(parts)
        total += len(joined) + 10 + tick % 11
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
"#,
    },
    Workload {
        name: "float_math_loop",
        vela: r#"
fn run_once() {
    let total = 0.0;
    let value = 1.25;
    for tick in 0..256 {
        total += value * 1.5 - value / 3.0 + 0.75;
        value += 0.5;
    }
    return math::round(total);
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
    local total = 0.0
    local value = 1.25
    for tick = 0, 255 do
        total = total + value * 1.5 - value / 3.0 + 0.75
        value = value + 0.5
    end
    return math.floor(total + 0.5)
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
    let total = 0.0;
    let value = 1.25;
    for tick in 0..256 {
        total += value * 1.5 - value / 3.0 + 0.75;
        value += 0.5;
    }
    total.round().to_int()
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
    let total = 0.0;
    let value = 1.25;
    for (let tick = 0; tick < 256; tick += 1) {
        total += value * 1.5 - value / 3.0 + 0.75;
        value += 0.5;
    }
    return Math.round(total);
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
    total = 0.0
    value = 1.25
    for tick in range(256):
        total += value * 1.5 - value / 3.0 + 0.75
        value += 0.5
    return int(total + 0.5)
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
"#,
    },
    Workload {
        name: "array_transform_sort",
        vela: r#"
fn run_once() {
    let total = 0;
    for tick in 0..64 {
        let values = [7, 3, 11, 5, 2, 13, 17, 19];
        let mapped = values.map(|value| value * 2 + tick % 5);
        let filtered = mapped.filter(|value| value % 3 != 0);
        let sorted = filtered.sort();
        total += sorted.sum() + sorted[0] + sorted[sorted.len() - 1];
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
    for tick = 0, 63 do
        local values = {7, 3, 11, 5, 2, 13, 17, 19}
        local filtered = {}
        for _, value in ipairs(values) do
            local mapped = value * 2 + tick % 5
            if mapped % 3 ~= 0 then
                filtered[#filtered + 1] = mapped
            end
        end
        table.sort(filtered)
        local sum = 0
        for _, value in ipairs(filtered) do
            sum = sum + value
        end
        total = total + sum + filtered[1] + filtered[#filtered]
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
    for tick in 0..64 {
        let values = [7, 3, 11, 5, 2, 13, 17, 19];
        let mapped = values.map(|value| value * 2 + tick % 5);
        let filtered = mapped.filter(|value| value % 3 != 0);
        filtered.sort();
        let sum = 0;
        for index in 0..filtered.len() {
            sum += filtered[index];
        }
        total += sum + filtered[0] + filtered[filtered.len() - 1];
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
    for (let tick = 0; tick < 64; tick += 1) {
        const values = [7, 3, 11, 5, 2, 13, 17, 19];
        const sorted = values
            .map((value) => value * 2 + tick % 5)
            .filter((value) => value % 3 !== 0)
            .sort((left, right) => left - right);
        let sum = 0;
        for (const value of sorted) {
            sum += value;
        }
        total += sum + sorted[0] + sorted[sorted.length - 1];
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
    for tick in range(64):
        values = [7, 3, 11, 5, 2, 13, 17, 19]
        sorted_values = sorted(
            value * 2 + tick % 5
            for value in values
            if (value * 2 + tick % 5) % 3 != 0
        )
        total += sum(sorted_values) + sorted_values[0] + sorted_values[-1]
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
"#,
    },
];
