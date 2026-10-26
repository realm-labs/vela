
import os
iterations = int(os.environ.get("VELA_BENCH_ITERATIONS", "1"))
def run_once():
    total = 0
    for outer in range(8):
        for value in range(128):
            total += value + outer - outer
    for value in range(64):
        total += value
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
