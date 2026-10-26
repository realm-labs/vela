
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
