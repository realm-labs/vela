
import os
iterations = int(os.environ.get("VELA_BENCH_ITERATIONS", "1"))
class Player:
    def __init__(self, id, level):
        self.id = id
        self.level = level
def run_once():
    total = 0
    for tick in range(96):
        alice = Player(1, 10)
        bob = Player(2, 20)
        alice_copy = Player(1, 10)
        active = {alice, bob}
        active.add(alice)
        active.discard(bob)
        alice.level += 1
        if alice in active and bob not in active and alice_copy not in active:
            total += len(active) + alice.level % 7 + tick % 5
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
