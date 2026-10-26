
import os
iterations = int(os.environ.get("VELA_BENCH_ITERATIONS", "1"))
class Counter:
    def __init__(self, value, step):
        self.value = value
        self.step = step
    def bump(self, extra):
        return self.value + self.step + extra
def run_once():
    total = 0
    for tick in range(96):
        counter = Counter(tick, tick % 5 + 1)
        total += counter.bump(tick % 3)
        total += counter.value + counter.step
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
