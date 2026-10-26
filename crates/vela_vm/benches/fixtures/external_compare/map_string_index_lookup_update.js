
const iterations = Number(process.env.VELA_BENCH_ITERATIONS || "1");
function runOnce() {
    const scores = { quest: 3, raid: 8, daily: 2 };
    let total = 0;
    for (let tick = 0; tick < 120; tick += 1) {
        scores.quest = scores.quest + (tick % 5);
        scores.daily = scores.daily + 1;
        total += scores.quest + scores.raid;
    }
    return total + scores.daily;
}
let checksum = 0;
for (let i = 0; i < iterations; i += 1) {
    checksum += runOnce();
}
console.log(checksum);
