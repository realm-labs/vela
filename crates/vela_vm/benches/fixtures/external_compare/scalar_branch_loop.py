
import os
iterations = int(os.environ.get("VELA_BENCH_ITERATIONS", "1"))
def run_once():
    total = 0
    for value in range(200):
        if value % 3 == 0:
            total += value * 2
            continue
        if value > 180:
            break
        total += (value * 5) % 17
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
