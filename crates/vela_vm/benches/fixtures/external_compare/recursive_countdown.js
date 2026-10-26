
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
