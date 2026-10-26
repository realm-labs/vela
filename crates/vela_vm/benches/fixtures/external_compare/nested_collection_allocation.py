
import os
iterations = int(os.environ.get("VELA_BENCH_ITERATIONS", "1"))
def run_once():
    total = 0
    for tick in range(64):
        rows = [
            [tick, tick + 1, tick + 2],
            [tick + 3, tick + 4, tick + 5],
            [tick + 6, tick + 7, tick + 8],
        ]
        labels = {
            "alpha": tick,
            "beta": tick + 2,
            "gamma": tick + 4,
        }
        total += rows[0][1] + rows[1][2] + rows[2][0]
        total += labels["alpha"] + labels["gamma"]
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
