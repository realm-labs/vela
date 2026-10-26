
import os
iterations = int(os.environ.get("VELA_BENCH_ITERATIONS", "1"))
def run_once():
    total = 0
    for tick in range(96):
        active = {"quest", "raid"}
        active.add("event")
        active.add("quest")
        active.discard("raid")
        if "quest" in active and "event" in active and "raid" not in active:
            total += len(active) + tick % 7
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
