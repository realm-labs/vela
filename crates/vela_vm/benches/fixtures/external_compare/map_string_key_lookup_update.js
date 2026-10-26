
const iterations = Number(process.env.VELA_BENCH_ITERATIONS || "1");
function runOnce() {
    const scores = { quest: 3, raid: 8, daily: 2 };
    let total = 0;
    for (let tick = 0; tick < 120; tick += 1) {
        scores.quest = (scores.quest || 0) + tick % 5;
        scores.daily = (scores.daily || 0) + 1;
        total += (scores.quest || 0) + (scores.raid || 0);
    }
    return total + (scores.daily || 0);
}
let checksum = 0;
for (let iteration = 0; iteration < iterations; iteration += 1) {
    checksum += runOnce();
}
console.log(String(checksum));
