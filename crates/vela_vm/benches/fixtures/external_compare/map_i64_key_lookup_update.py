
import os
iterations = int(os.environ.get("VELA_BENCH_ITERATIONS", "1"))
def run_once():
    scores = {1: 3, 2: 8, 3: 2}
    total = 0
    for tick in range(120):
        scores[1] = scores.get(1, 0) + tick % 5
        scores[3] = scores.get(3, 0) + 1
        total += scores.get(1, 0) + scores.get(2, 0)
    return total + scores.get(3, 0)
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
