
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
