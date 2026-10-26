
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
