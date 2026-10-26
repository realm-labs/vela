
import os
iterations = int(os.environ.get("VELA_BENCH_ITERATIONS", "1"))
class Player:
    def __init__(self, id, level):
        self.id = id
        self.level = level
def run_once():
    alice = Player(1, 10)
    bob = Player(2, 20)
    alice_copy = Player(1, 10)
    scores = {alice: 3, bob: 8}
    total = 0
    for tick in range(96):
        alice.level += 1
        scores[alice] = scores.get(alice, 0) + tick % 5
        if alice in scores and bob in scores and alice_copy not in scores:
            total += scores.get(alice, 0) + scores.get(bob, 0) + alice.level % 7
    return total
checksum = 0
for _ in range(iterations):
    checksum += run_once()
print(checksum)
