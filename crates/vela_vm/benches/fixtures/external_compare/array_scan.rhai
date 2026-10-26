
fn run_once() {
    let values = [3, 1, 4, 1, 5, 9, 2, 6];
    let total = 0;
    for tick in 0..200 {
        for value in values {
            if value % 2 == 0 {
                total += (value * tick) % 17;
            } else {
                total += value + tick % 5;
            }
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
