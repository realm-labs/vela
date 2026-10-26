
import os
iterations = int(os.environ.get("VELA_BENCH_ITERATIONS", "1"))
def run_once():
    total = 0
    for tick in range(72):
        text = "alpha,beta,gamma,delta"
        parts = text.split(",")
        joined = ":".join(parts)
        total += len(joined) + 10 + tick % 11
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
