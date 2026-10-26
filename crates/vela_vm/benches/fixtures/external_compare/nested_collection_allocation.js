
const iterations = Number(process.env.VELA_BENCH_ITERATIONS || "1");
function runOnce() {
    let total = 0;
    for (let tick = 0; tick < 64; tick += 1) {
        const rows = [
            [tick, tick + 1, tick + 2],
            [tick + 3, tick + 4, tick + 5],
            [tick + 6, tick + 7, tick + 8],
        ];
        const labels = {
            alpha: tick,
            beta: tick + 2,
            gamma: tick + 4,
        };
        total += rows[0][1] + rows[1][2] + rows[2][0];
        total += labels["alpha"] + labels["gamma"];
    }
    return total;
}
let checksum = 0;
for (let iteration = 0; iteration < iterations; iteration += 1) {
    checksum += runOnce();
}
console.log(String(checksum));
