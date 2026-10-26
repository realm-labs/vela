
fn add(value, tick) {
    value + tick
}

fn mix(left, right, tick) {
    left * 3 + right + tick - tick
}

fn run_once() {
    let total = 0;
    for tick in 0..120 {
        total += add(tick, tick);
        total += add(total % 17, tick);
        total += mix(tick, total % 23, tick);
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
