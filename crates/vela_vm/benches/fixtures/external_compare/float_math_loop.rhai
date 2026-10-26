
fn run_once() {
    let total = 0.0;
    let value = 1.25;
    for tick in 0..256 {
        total += value * 1.5 - value / 3.0 + 0.75;
        value += 0.5;
    }
    total.round().to_int()
}

fn run(iterations) {
    let checksum = 0;
    for iteration in 0..iterations {
        checksum += run_once();
    }
    checksum
}
