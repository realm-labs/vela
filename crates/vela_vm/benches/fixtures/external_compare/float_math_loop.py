
import os
iterations = int(os.environ.get("VELA_BENCH_ITERATIONS", "1"))
def run_once():
    total = 0.0
    value = 1.25
    for tick in range(256):
        total += value * 1.5 - value / 3.0 + 0.75
        value += 0.5
    return int(total + 0.5)
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
