
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
