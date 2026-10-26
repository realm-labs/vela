
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
