
const iterations = Number(process.env.VELA_BENCH_ITERATIONS || "1");
function runOnce() {
    const scores = new Map([[1, 3], [2, 8], [3, 2]]);
    let total = 0;
    for (let tick = 0; tick < 120; tick += 1) {
        scores.set(1, (scores.get(1) || 0) + tick % 5);
        scores.set(3, (scores.get(3) || 0) + 1);
        total += (scores.get(1) || 0) + (scores.get(2) || 0);
    }
    return total + (scores.get(3) || 0);
}
let checksum = 0;
for (let iteration = 0; iteration < iterations; iteration += 1) {
    checksum += runOnce();
}
console.log(String(checksum));
