
fn descend(value) {
    if value <= 0 {
        return 0;
    }
    return value + descend(value - 1);
}

fn run_once() {
    let total = 0;
    for value in 16..24 {
        total += descend(value);
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
