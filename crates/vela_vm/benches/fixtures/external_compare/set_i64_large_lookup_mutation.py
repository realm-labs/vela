
import os
iterations = int(os.environ.get("VELA_BENCH_ITERATIONS", "1"))
def run_once():
    active = set(range(64))
    total = 0
    for tick in range(192):
        active.add(64)
        active.discard(64)
        if 0 in active and 63 in active and 128 not in active:
            total += len(active) + tick % 7
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
