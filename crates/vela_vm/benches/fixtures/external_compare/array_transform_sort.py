
import os
iterations = int(os.environ.get("VELA_BENCH_ITERATIONS", "1"))
def run_once():
    total = 0
    for tick in range(64):
        values = [7, 3, 11, 5, 2, 13, 17, 19]
        sorted_values = sorted(
            value * 2 + tick % 5
            for value in values
            if (value * 2 + tick % 5) % 3 != 0
        )
        total += sum(sorted_values) + sorted_values[0] + sorted_values[-1]
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
