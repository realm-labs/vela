
import os
iterations = int(os.environ.get("VELA_BENCH_ITERATIONS", "1"))
def run_once():
    scores = {"quest": 3, "raid": 8, "daily": 2}
    total = 0
    for tick in range(120):
        scores["quest"] = scores["quest"] + tick % 5
        scores["daily"] = scores.get("daily", 0) + 1
        total += scores.get("quest", 0) + scores.get("raid", 0)
    return total + scores.get("daily", 0)
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
