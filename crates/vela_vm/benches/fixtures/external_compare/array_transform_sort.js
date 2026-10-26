
const iterations = Number(process.env.VELA_BENCH_ITERATIONS || "1");
function runOnce() {
    let total = 0;
    for (let tick = 0; tick < 64; tick += 1) {
        const values = [7, 3, 11, 5, 2, 13, 17, 19];
        const sorted = values
            .map((value) => value * 2 + tick % 5)
            .filter((value) => value % 3 !== 0)
            .sort((left, right) => left - right);
        let sum = 0;
        for (const value of sorted) {
            sum += value;
        }
        total += sum + sorted[0] + sorted[sorted.length - 1];
    }
    return total;
}
let checksum = 0;
for (let iteration = 0; iteration < iterations; iteration += 1) {
    checksum += runOnce();
}
console.log(String(checksum));
