
fn run_once() {
    let total = 0;
    for tick in 0..96 {
        let alice_level = 11;
        total += 1 + alice_level % 7 + tick % 5;
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
