
import os
iterations = int(os.environ.get("VELA_BENCH_ITERATIONS", "1"))
def run_once():
    total = 0
    for tick in range(120):
        add = lambda value, tick=tick: value + tick
        mix = lambda left, right, tick=tick: left * 3 + right + tick - tick
        total += add(tick)
        total += add(total % 17)
        total += mix(tick, total % 23)
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
