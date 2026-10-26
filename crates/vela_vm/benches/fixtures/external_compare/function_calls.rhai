
fn add_one(value) {
    value + 1
}

fn mix_pair(left, right) {
    left * 3 + right
}

fn run_once() {
    let total = 0;
    for tick in 0..240 {
        total += add_one(tick);
        total += mix_pair(tick, total % 17);
    }
    total
}

fn run(iterations) {
    let checksum = 0;
    for iteration in 0..iterations {
        checksum += run_once();
    }
    checksum
}
