
import os
iterations = int(os.environ.get("VELA_BENCH_ITERATIONS", "1"))
values = [3, 1, 4, 1, 5, 9, 2, 6]
def run_once():
    total = 0
    for tick in range(200):
        for value in values:
            if value % 2 == 0:
                total += (value * tick) % 17
            else:
                total += value + tick % 5
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
