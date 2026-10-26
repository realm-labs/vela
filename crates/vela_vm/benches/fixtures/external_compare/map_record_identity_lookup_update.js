
const iterations = Number(process.env.VELA_BENCH_ITERATIONS || "1");
function runOnce() {
    const alice = { id: 1, level: 10 };
    const bob = { id: 2, level: 20 };
    const aliceCopy = { id: 1, level: 10 };
    const scores = new Map([[alice, 3], [bob, 8]]);
    let total = 0;
    for (let tick = 0; tick < 96; tick += 1) {
        alice.level += 1;
        scores.set(alice, (scores.get(alice) || 0) + tick % 5);
        if (scores.has(alice) && scores.has(bob) && !scores.has(aliceCopy)) {
            total += (scores.get(alice) || 0) + (scores.get(bob) || 0) + alice.level % 7;
        }
    }
    return total;
}
let checksum = 0;
for (let iteration = 0; iteration < iterations; iteration += 1) {
    checksum += runOnce();
}
console.log(String(checksum));
