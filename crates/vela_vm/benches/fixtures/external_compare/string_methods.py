
import os
iterations = int(os.environ.get("VELA_BENCH_ITERATIONS", "1"))
def run_once():
    total = 0
    labels = ["quest", "raid", "daily", "bonus"]
    for tick in range(50):
        for label in labels:
            if label.startswith("q") or "i" in label:
                total += len(label) + tick % 7
            else:
                total += len(label)
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
