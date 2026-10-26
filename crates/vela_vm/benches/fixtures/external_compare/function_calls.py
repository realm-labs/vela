
import os
iterations = int(os.environ.get("VELA_BENCH_ITERATIONS", "1"))
def add_one(value):
    return value + 1
def mix_pair(left, right):
    return left * 3 + right
def run_once():
    total = 0
    for tick in range(240):
        total += add_one(tick)
        total += mix_pair(tick, total % 17)
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
