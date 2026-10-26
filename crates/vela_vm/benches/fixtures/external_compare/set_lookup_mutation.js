
const iterations = Number(process.env.VELA_BENCH_ITERATIONS || "1");
function runOnce() {
    let total = 0;
    for (let tick = 0; tick < 96; tick += 1) {
        const active = new Set(["quest", "raid"]);
        active.add("event");
        active.add("quest");
        active.delete("raid");
        if (active.has("quest") && active.has("event") && !active.has("raid")) {
            total += active.size + tick % 7;
        }
    }
    return total;
}
let checksum = 0;
for (let iteration = 0; iteration < iterations; iteration += 1) {
    checksum += runOnce();
}
console.log(String(checksum));
