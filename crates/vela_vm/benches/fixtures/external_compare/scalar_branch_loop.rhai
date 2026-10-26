
fn run_once() {
    let total = 0;
    for value in 0..200 {
        if value % 3 == 0 {
            total += value * 2;
        } else if value > 180 {
            break;
        } else {
            total += (value * 5) % 17;
        }
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
