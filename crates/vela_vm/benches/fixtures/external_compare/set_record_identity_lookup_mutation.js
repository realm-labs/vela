
const iterations = Number(process.env.VELA_BENCH_ITERATIONS || "1");
function runOnce() {
    let total = 0;
    for (let tick = 0; tick < 96; tick += 1) {
        const alice = { id: 1, level: 10 };
        const bob = { id: 2, level: 20 };
        const aliceCopy = { id: 1, level: 10 };
        const active = new Set([alice, bob]);
        active.add(alice);
        active.delete(bob);
        alice.level += 1;
        if (active.has(alice) && !active.has(bob) && !active.has(aliceCopy)) {
            total += active.size + alice.level % 7 + tick % 5;
        }
    }
    return total;
}
let checksum = 0;
for (let iteration = 0; iteration < iterations; iteration += 1) {
    checksum += runOnce();
}
console.log(String(checksum));
