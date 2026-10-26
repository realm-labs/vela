
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
