
import os
iterations = int(os.environ.get("VELA_BENCH_ITERATIONS", "1"))
def run_once():
    total = 0
    for tick in range(96):
        active = {1, 2}
        active.add(3)
        active.add(1)
        active.discard(2)
        if 1 in active and 3 in active and 2 not in active:
            total += len(active) + tick % 7
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
