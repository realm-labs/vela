
const iterations = Number(process.env.VELA_BENCH_ITERATIONS || "1");
const values = [3, 1, 4, 1, 5, 9, 2, 6];
function runOnce() {
    let total = 0;
    for (let tick = 0; tick < 200; tick += 1) {
        for (const value of values) {
            if (value % 2 === 0) {
                total += (value * tick) % 17;
            } else {
                total += value + tick % 5;
            }
        }
    }
    return total;
}
let checksum = 0;
for (let iteration = 0; iteration < iterations; iteration += 1) {
    checksum += runOnce();
}
console.log(String(checksum));
