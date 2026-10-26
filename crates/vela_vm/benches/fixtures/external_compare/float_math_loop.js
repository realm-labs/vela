
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
