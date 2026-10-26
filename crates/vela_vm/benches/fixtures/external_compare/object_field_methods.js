
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
