
import os
iterations = int(os.environ.get("VELA_BENCH_ITERATIONS", "1"))
def run_once():
    scores = {"quest": 3, "raid": 8, "daily": 2}
    total = 0
    for tick in range(120):
        scores["quest"] = scores["quest"] + tick % 5
        scores["daily"] = scores["daily"] + 1
        total += scores["quest"] + scores["raid"]
    return total + scores["daily"]
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
