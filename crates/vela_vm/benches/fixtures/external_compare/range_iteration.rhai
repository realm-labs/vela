
fn run_once() {
    let total = 0;
    for outer in 0..8 {
        for value in 0..128 {
            total += value + outer - outer;
        }
    }
    for value in 0..=63 {
        total += value;
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
