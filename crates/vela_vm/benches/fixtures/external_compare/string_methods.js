
const iterations = Number(process.env.VELA_BENCH_ITERATIONS || "1");
function runOnce() {
    let total = 0;
    const labels = ["quest", "raid", "daily", "bonus"];
    for (let tick = 0; tick < 50; tick += 1) {
        for (const label of labels) {
            if (label.startsWith("q") || label.includes("i")) {
                total += label.length + tick % 7;
            } else {
                total += label.length;
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
