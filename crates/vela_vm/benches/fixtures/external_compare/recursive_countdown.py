
import os
iterations = int(os.environ.get("VELA_BENCH_ITERATIONS", "1"))
def descend(value):
    if value <= 0:
        return 0
    return value + descend(value - 1)
def run_once():
    total = 0
    for value in range(16, 24):
        total += descend(value)
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
